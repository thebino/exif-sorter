use assert_cmd::Command;
use imagemeta::exif;
use img_parts::ImageEXIF;
use predicates::prelude::*;
use std::io::Cursor;
use std::{
    fs::{self, File},
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
fn should_die_with_non_existing_source_directory() {
    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.arg("--source-dir=non-existing")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid source directory:"))
        .stderr(predicate::str::contains("non-existing"));
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

#[allow(dead_code)]
// #[test]
fn should_rename_file_at_target_instead_of_overwriting() {
    // create a test directory
    let testdir = temp_dir::TempDir::new().unwrap();
    let testpath = testdir.path();

    let testfile1 = testdir.child("testfile1");
    std::fs::write(&testfile1, b"abc").unwrap();
    let _ = write_exif_to_file(&testfile1);

    let testfile2 = testdir.child("testfile2");
    std::fs::write(&testfile2, b"def").unwrap();
    let _ = write_exif_to_file(&testfile2);

    let mut cmd = Command::cargo_bin("exif-sorter").unwrap();
    cmd.arg("-s")
        .arg(testpath)
        .assert()
        .failure()
        .stdout(predicate::str::contains("testfile1"))
        .stdout(predicate::str::contains("No exif information!"));
}
// TODO: add test for dry-run
// TODO: add test for file moved
// TODO: add test for duplicate file (same filename only)

#[allow(dead_code)]
fn write_exif_to_file(path: &Path) -> Result<(), anyhow::Error> {
    let input = fs::read(path)?;
    let mut jpeg = img_parts::jpeg::Jpeg::from_bytes(input.into())?;

    let exif = exif::Exif {
        ifds: vec![exif::Ifd {
            id: 0,
            entries: vec![exif::Entry {
                tag: 0x9003,
                data: exif::EntryData::Ascii("1991:01:01 00:13:37".to_string()),
            }],
            children: Vec::new(),
        }],
    };

    let mut out_exif = Cursor::new(Vec::new());
    // exif_metadata.encode(&mut out_exif)?;
    exif.encode(&mut out_exif)?;

    jpeg.set_exif(Some(out_exif.into_inner().into()));
    let output = File::create(path)?;
    let _ = jpeg.encoder().write_to(output)?;

    Ok(())
}
