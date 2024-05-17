use std::{fs, os::unix::fs::PermissionsExt};

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn should_display_usage_when_executed_with_help_argument() {
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn should_die_with_non_existing_source_directory() {
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.arg("-s test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid source directory:"))
        .stderr(predicate::str::contains("test"));
}

#[test]
fn should_skip_directories_with_lack_of_permissions_for_source_directory() {
    // create a test directory
    let testdir = temp_dir::TempDir::new().unwrap();
    let testpath = testdir.path();

    // create a test file and restrict permission
    let testfile = testdir.child("testfile");
    std::fs::write(testfile, b"abc").unwrap();
    let mut perms = fs::metadata(testpath).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(testpath, perms).unwrap();

    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.arg("-s")
        .arg(testpath)
        .assert()
        .success()
        .stdout(predicate::str::contains("Permission denied"));
}

#[test]
fn should_skip_file_without_exif_information_available() {
    // create a test directory
    let testdir = temp_dir::TempDir::new().unwrap();
    let testpath = testdir.path();

    // create a test file and restrict permission
    let testfile = testdir.child("testfile");
    std::fs::write(testfile, b"abc").unwrap();

    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.arg("-s")
        .arg(testpath)
        .assert()
        .success()
        .stdout(predicate::str::contains("testfile"))
        .stdout(predicate::str::contains("No exif information!"));
}

// TODO: add test for dry-run
// TODO: add test for file moved
// TODO: add test for duplicate file (same filename only)
