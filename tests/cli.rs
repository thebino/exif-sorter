#![allow(deprecated)]
use assert_cmd::Command;
use eframe::egui::TextBuffer;
use predicates::prelude::*;
use std::path::PathBuf;
use std::{
    fs::{self},
    os::unix::fs::PermissionsExt,
    path::Path,
};

#[test]
fn should_display_usage_when_executed_with_help_argument() {
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn should_display_help_for_cli_subcommands() {
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args(["cli", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--immediate"));
}

#[test]
fn should_die_with_non_existing_source_directory() {
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args(["--source-dir=non-existing", "cli"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid source directory:"))
        .stderr(predicate::str::contains("non-existing"));
}

#[test]
fn should_skip_directories_with_lack_of_permissions_for_source_directory() {
    // An unreadable file (mode 000) cannot be copied (copy needs read
    // permission). The run must not crash, the failure must be reported,
    // and the source must stay in place — no data lost.
    // given
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");

    let testfile = root.join("dateTimeOriginal.png");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        testfile.as_path(),
    );

    let mut permissions = fs::metadata(&testfile).unwrap().permissions();
    permissions.set_mode(0o000);
    assert_eq!(permissions.mode(), 0o000);
    fs::set_permissions(&testfile, permissions).unwrap();

    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args([
        "cli",
        "-s",
        root.as_path().to_string_lossy().as_str(),
        "-t",
        target.as_path().to_string_lossy().as_str(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Permission denied"));

    // then: source not lost
    assert!(testfile.exists(), "unreadable source file must stay in place");
}

#[test]
fn should_copy_by_default_and_print_summary() {
    // Default mode is copy: the tool runs against just-recovered data, so
    // the source must stay untouched unless --move is given explicitly.
    // given
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    let testfile = root.join("dateTimeOriginal.png");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        testfile.as_path(),
    );

    // when
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args([
        "cli",
        "-s",
        root.as_path().to_string_lossy().as_str(),
        "-t",
        target.as_path().to_string_lossy().as_str(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Copied 1 of 1"));

    // then: copied, source untouched, manifest written
    assert!(testfile.exists(), "copy mode must not remove the source");
    assert!(target.join("1991/1991-01-01/dateTimeOriginal.png").exists());
    assert!(target.join("exif-sorter-manifest.csv").exists());
}

#[test]
fn should_move_files_with_move_flag() {
    // given
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    let testfile = root.join("dateTimeOriginal.png");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        testfile.as_path(),
    );

    // when
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args([
        "cli",
        "-s",
        root.as_path().to_string_lossy().as_str(),
        "-t",
        target.as_path().to_string_lossy().as_str(),
        "--move",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Moved 1 of 1"));

    // then
    assert!(!testfile.exists(), "--move must remove the source");
    assert!(target.join("1991/1991-01-01/dateTimeOriginal.png").exists());
}

#[test]
fn should_dedupe_identical_files_with_on_collision_dedupe() {
    // Recovered sets are full of exact duplicates (PhotoRec finds the same
    // file via multiple signatures). dedupe must store the content once.
    // given: identical file in two source subdirectories
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    for sub in ["sub1", "sub2"] {
        let dir = root.join(sub);
        fs::create_dir_all(&dir).unwrap();
        let _ = fs::copy(
            Path::new("tests/data/dateTimeOriginal.png"),
            dir.join("dateTimeOriginal.png"),
        );
    }

    // when
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args([
        "cli",
        "-s",
        root.as_path().to_string_lossy().as_str(),
        "-t",
        target.as_path().to_string_lossy().as_str(),
        "--on-collision",
        "dedupe",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Copied 1 of 2"))
    .stdout(predicate::str::contains("1 exact duplicates"));

    // then: stored exactly once
    let count = fs::read_dir(target.join("1991/1991-01-01"))
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().is_file())
        .count();
    assert_eq!(count, 1);
}

#[test]
fn should_revert_a_copy_run_from_manifest() {
    // given: a completed copy run
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    let testfile = root.join("dateTimeOriginal.png");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        testfile.as_path(),
    );
    Command::cargo_bin("exif-sorter")
        .unwrap()
        .args([
            "cli",
            "-s",
            root.as_path().to_string_lossy().as_str(),
            "-t",
            target.as_path().to_string_lossy().as_str(),
        ])
        .assert()
        .success();
    let copied = target.join("1991/1991-01-01/dateTimeOriginal.png");
    assert!(copied.exists());

    // when
    let manifest = target.join("exif-sorter-manifest.csv");
    Command::cargo_bin("exif-sorter")
        .unwrap()
        .args(["revert", "-m", manifest.to_string_lossy().as_str()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Reverted 1"));

    // then: copy removed, source still present
    assert!(!copied.exists(), "revert must remove the copy");
    assert!(testfile.exists(), "revert must never touch the source");
}

#[test]
fn should_apply_custom_folder_pattern() {
    // given
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        root.join("dateTimeOriginal.png"),
    );

    // when: month-based layout instead of the default {year}/{date}
    Command::cargo_bin("exif-sorter")
        .unwrap()
        .args([
            "cli",
            "-s",
            root.as_path().to_string_lossy().as_str(),
            "-t",
            target.as_path().to_string_lossy().as_str(),
            "--pattern",
            "{year}/{month}",
        ])
        .assert()
        .success();

    // then
    assert!(target.join("1991/01/dateTimeOriginal.png").exists());
}

#[test]
fn should_read_settings_from_config_file() {
    // given: a config file selecting a custom pattern
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        root.join("dateTimeOriginal.png"),
    );
    let config = root.join("config.toml");
    fs::write(&config, "pattern = \"{year}-{month}-{day}\"\n").unwrap();

    // when
    Command::cargo_bin("exif-sorter")
        .unwrap()
        .args([
            "cli",
            "-s",
            root.as_path().to_string_lossy().as_str(),
            "-t",
            target.as_path().to_string_lossy().as_str(),
            "--config",
            config.to_string_lossy().as_str(),
        ])
        .assert()
        .success();

    // then
    assert!(target.join("1991-01-01/dateTimeOriginal.png").exists());
}

#[test]
fn should_route_unrecognizable_content_to_corrupt() {
    // Carved files (PhotoRec output) often have an image extension but
    // garbage content, and their file dates reflect the recovery run — not
    // the capture. Sorting them by file date produces confidently-wrong
    // folders; they must go to corrupt/ instead.
    // given: a .jpg whose content matches no known file signature
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    fs::write(root.join("garbage.jpg"), b"this is not an image at all").unwrap();

    // when
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args([
        "cli",
        "-s",
        root.as_path().to_string_lossy().as_str(),
        "-t",
        target.as_path().to_string_lossy().as_str(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("corrupt/"));

    // then: routed to corrupt/, source untouched (copy default)
    assert!(target.join("corrupt/garbage.jpg").exists());
    assert!(root.join("garbage.jpg").exists());
}

#[test]
fn should_not_resort_files_already_in_target() {
    // Default arguments place the target inside the source. A second run
    // must not pick up already-sorted files and shuffle them again.
    // given: a pre-sorted file inside the target tree
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");
    let sorted_dir = target.join("1991/1991-01-01");
    fs::create_dir_all(&sorted_dir).unwrap();
    let sorted_file = sorted_dir.join("dateTimeOriginal.png");
    let _ = fs::copy(Path::new("tests/data/dateTimeOriginal.png"), &sorted_file);

    // when
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args([
        "cli",
        "-s",
        root.as_path().to_string_lossy().as_str(),
        "-t",
        target.as_path().to_string_lossy().as_str(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Copied 0 of 0"));

    // then: file untouched
    assert!(sorted_file.exists(), "already-sorted file was moved again");
}

#[test]
fn should_rename_duplicates_instead_of_overwriting() {
    //given
    let root: PathBuf = testdir::testdir!();
    let target = root.join("sorted");

    // sub1/testfile
    let sub1 = root.join("sub1");
    let _ = std::fs::create_dir(&sub1);
    let testfile1 = sub1.join("dateTimeOriginal.png");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        testfile1.as_path(),
    );

    // sub2/testfile
    let sub2 = root.join("sub2");
    let _ = std::fs::create_dir(&sub2);
    let testfile2 = sub2.join("dateTimeOriginal.png");
    let _ = fs::copy(
        Path::new("tests/data/dateTimeOriginal.png"),
        testfile2.as_path(),
    );

    // when
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.args([
        "cli",
        "-s",
        root.as_path().to_string_lossy().as_str(),
        "-t",
        target.as_path().to_string_lossy().as_str(),
        "--immediate",
    ])
    .assert()
    .success();

    let sorted = root.join("sorted/1991/1991-01-01");

    // then
    let count = fs::read_dir(sorted)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.path().is_file() {
                Some(())
            } else {
                None
            }
        })
        .count();
    assert_eq!(count, 2);
}

