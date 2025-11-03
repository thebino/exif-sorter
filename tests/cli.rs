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
        "--immediate",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Permission denied"));
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

