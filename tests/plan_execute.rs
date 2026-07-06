use std::fs;
use std::path::PathBuf;

use exif_sorter::sorter::{
    execute, plan, process, ItemOutcome, PlannedAction, ProcessOptions,
};

/// Source dir with one file per routing category: an EXIF-dated PNG
/// (Sorted, high confidence), a PNG-signature file without any metadata
/// (recognized content, low-confidence Sorted via file dates) and garbage
/// bytes behind a .jpg extension (Corrupt).
fn build_mixed_source(root: &PathBuf) {
    // EXIF-dated (1991-01-01, from the repo fixture)
    fs::copy(
        "tests/data/dateTimeOriginal.png",
        root.join("exif_dated.png"),
    )
    .unwrap();
    // Valid PNG signature but no metadata and no filename date → file dates
    // (plausible → low-confidence Sorted)
    let png_sig = [0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
    fs::write(root.join("recognized_no_date.png"), png_sig).unwrap();
    // Garbage with an image extension → Corrupt
    fs::write(root.join("garbage.jpg"), b"definitely not an image").unwrap();
}

#[test]
fn plan_is_read_only_and_routes_correctly() {
    // Planning must never touch the filesystem: the review flow promises the
    // user that nothing happens before confirmation — on recovered media a
    // stray write into the target could destroy data that is still being
    // triaged. The target directory must not even be created.
    // given
    let tmp = testdir::testdir!();
    let source = tmp.join("source");
    let target = tmp.join("sorted");
    fs::create_dir_all(&source).unwrap();
    build_mixed_source(&source);

    // when
    let plan = plan(&source, &target, &ProcessOptions::default(), |_, _| {}).unwrap();

    // then: routing per file
    assert_eq!(plan.items.len(), 3);
    let action_of = |name: &str| {
        plan.items
            .iter()
            .find(|i| i.image.source_full().ends_with(name))
            .unwrap_or_else(|| panic!("no planned item for {name}"))
    };
    let exif = action_of("exif_dated.png");
    assert!(
        matches!(&exif.action, PlannedAction::Sorted { date, .. } if date.to_string() == "1991-01-01"),
        "exif_dated.png should be Sorted by its EXIF date"
    );
    assert!(
        exif.planned_target.to_string_lossy().contains("1991/1991-01-01"),
        "planned target must follow the pattern"
    );
    assert!(matches!(
        action_of("recognized_no_date.png").action,
        PlannedAction::Sorted { .. }
    ));
    assert!(matches!(
        action_of("garbage.jpg").action,
        PlannedAction::Corrupt { .. }
    ));
    assert!(plan.items.iter().all(|i| i.selected), "everything selected by default");

    // then: nothing was written
    assert!(!target.exists(), "plan() must not create the target directory");
}

#[test]
fn plan_succeeds_when_target_does_not_exist() {
    // Regression: the target-subtree filter used target.canonicalize()?,
    // which fails for a not-yet-existing target. Planning must tolerate it.
    // given
    let tmp = testdir::testdir!();
    let source = tmp.join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("a.jpg"), b"x").unwrap();

    // when
    let result = plan(
        &source,
        &tmp.join("does_not_exist_yet"),
        &ProcessOptions::default(),
        |_, _| {},
    );

    // then
    assert!(result.is_ok(), "plan must work with a missing target dir");
}

#[test]
fn execute_all_selected_matches_one_shot_process() {
    // given: two identical source trees
    let tmp = testdir::testdir!();
    let source_a = tmp.join("source_a");
    let source_b = tmp.join("source_b");
    fs::create_dir_all(&source_a).unwrap();
    fs::create_dir_all(&source_b).unwrap();
    build_mixed_source(&source_a);
    build_mixed_source(&source_b);
    let options = ProcessOptions::default();

    // when: one tree through plan+execute, the other through process()
    let the_plan = plan(&source_a, &tmp.join("sorted_a"), &options, |_, _| {}).unwrap();
    let summary_a = execute(the_plan, &options, |_, _| {}).unwrap();
    let summary_b = process(&source_b, &tmp.join("sorted_b"), &options, |_, _| {}).unwrap();

    // then: identical counts, identical file layout
    assert_eq!(summary_a.transferred, summary_b.transferred);
    assert_eq!(summary_a.corrupt, summary_b.corrupt);
    assert_eq!(summary_a.total(), summary_b.total());
    assert!(tmp.join("sorted_a/1991/1991-01-01/exif_dated.png").exists());
    assert!(tmp.join("sorted_a/corrupt/garbage.jpg").exists());
    assert!(tmp.join("sorted_a/exif-sorter-manifest.csv").exists());
}

#[test]
fn execute_skips_deselected_items_and_reports_them() {
    // The review step exists so the user can veto individual transfers; a
    // deselected file must stay untouched at the source, appear in no
    // summary count, and be reported as SkippedByUser at its plan index.
    // given
    let tmp = testdir::testdir!();
    let source = tmp.join("source");
    let target = tmp.join("sorted");
    fs::create_dir_all(&source).unwrap();
    build_mixed_source(&source);
    let options = ProcessOptions::default();
    let mut the_plan = plan(&source, &target, &options, |_, _| {}).unwrap();

    // deselect the EXIF-dated file
    let deselected_index = the_plan
        .items
        .iter()
        .position(|i| i.image.source_full().ends_with("exif_dated.png"))
        .unwrap();
    the_plan.items[deselected_index].selected = false;
    let deselected_source = the_plan.items[deselected_index].image.source_full();

    // when
    let mut outcomes = Vec::new();
    let summary = execute(the_plan, &options, |index, outcome| {
        outcomes.push((index, outcome.clone()));
    })
    .unwrap();

    // then
    assert!(matches!(
        outcomes[deselected_index].1,
        ItemOutcome::SkippedByUser
    ));
    assert_eq!(outcomes.len(), 3, "on_item fires for every planned item");
    assert!(
        !target.join("1991/1991-01-01/exif_dated.png").exists(),
        "deselected file must not be transferred"
    );
    assert!(
        PathBuf::from(&deselected_source).exists(),
        "deselected source must stay in place"
    );
    assert_eq!(
        summary.total(),
        2,
        "deselected items are excluded from all summary counts"
    );
}

#[test]
fn collision_appearing_between_plan_and_execute_is_still_handled() {
    // The review pause makes plans stale by design. If another file claims
    // the planned path in the meantime, the collision policy must still
    // apply at execute time — otherwise reviewing would weaken the
    // no-silent-overwrite guarantee.
    // given
    let tmp = testdir::testdir!();
    let source = tmp.join("source");
    let target = tmp.join("sorted");
    fs::create_dir_all(&source).unwrap();
    fs::copy("tests/data/dateTimeOriginal.png", source.join("photo.png")).unwrap();
    let options = ProcessOptions::default(); // Suffix policy
    let the_plan = plan(&source, &target, &options, |_, _| {}).unwrap();
    let planned_path = the_plan.items[0].planned_target.clone();

    // someone claims the planned path during review
    fs::create_dir_all(planned_path.parent().unwrap()).unwrap();
    fs::write(&planned_path, b"someone else's file").unwrap();

    // when
    let summary = execute(the_plan, &options, |_, _| {}).unwrap();

    // then: transferred under a suffixed name, existing file untouched
    assert_eq!(summary.transferred, 1);
    assert_eq!(
        fs::read(&planned_path).unwrap(),
        b"someone else's file",
        "existing file must never be overwritten"
    );
    let siblings = fs::read_dir(planned_path.parent().unwrap())
        .unwrap()
        .count();
    assert_eq!(siblings, 2, "expected original plus suffixed transfer");
}
