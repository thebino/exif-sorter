use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use exif_sorter::sorter::image::{DateSource, Image};

/// Write a minimal TIFF stream containing only the given EXIF fields.
/// kamadak-exif reads TIFF containers directly, so the output is a valid
/// input for `read_exif_date` without needing a full JPEG wrapper.
fn write_exif_fixture(path: &Path, fields: &[exif::Field]) {
    use exif::experimental::Writer;
    let mut writer = Writer::new();
    for field in fields {
        writer.push_field(field);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    writer.write(&mut buf, false).expect("failed to write EXIF fixture");
    std::fs::write(path, buf.into_inner()).expect("failed to write fixture file");
}

fn ascii_field(tag: exif::Tag, value: &str) -> exif::Field {
    exif::Field {
        tag,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Ascii(vec![value.as_bytes().to_vec()]),
    }
}

#[test]
fn test_extract_creation_date() {
    // given
    let path = Path::new("tests/data/exif/NIKON CORPORATION NIKON D50 3040x2014_019917.nef");
    let file = Image::new(path.to_path_buf(), path.to_path_buf());

    let expected = NaiveDate::from_ymd_opt(2024, 10, 19).unwrap();

    // when
    let date = file.extract_file_creation_date();

    // then
    assert!(date.is_ok());
    assert_eq!(date.unwrap(), expected);
}

#[test]
fn test_extract_modification_date() {
    // given
    let path = Path::new("tests/data/exif/NIKON CORPORATION NIKON D50 3040x2014_019917.nef");
    let file = Image::new(path.to_path_buf(), path.to_path_buf());
    let expected = NaiveDate::from_ymd_opt(2025, 11, 3).unwrap();

    // when
    let date = file.extract_file_modified_date();

    // then
    assert!(date.is_ok());
    assert_eq!(date.unwrap(), expected);
}

test_datetime_extraction_cases! {
    canon_5dm2_cr2: "Canon EOS 5D Mark II 5616x3744_000014.cr2" => "2021:07:03",
    canon_5dm3_cr2: "Canon EOS 5D Mark III 5760x3840_000915.cr2" => "2023:07:29",
    canon_7d: "Canon EOS 7D 5184x3456_000673.cr2" => "2017:05:14",
    google_pixel_5_dng: "Google Pixel 5 2016x1512_000253.dng" => "2023:02:06",
    hasselblad_x1d_fff: "Hasselblad-x1d-II.fff" => "2019:05:31",
    nikon_d50_nef: "NIKON CORPORATION NIKON D50 3040x2014_019917.nef" => "2009:01:09",
    nikon_d300_nef: "NIKON CORPORATION NIKON D300 4352x2868_055006.nef" => "2011:05:21",
    nikon_d800_dng: "NIKON CORPORATION NIKON D800 4912x7360_000161.dng" => "2018:07:22",
    nikon_d5100_nef: "NIKON CORPORATION NIKON D5100 4992x3280_058129.nef" => "2014:05:20",
    nikon_z7_nef: "NIKON CORPORATION NIKON Z 7 8288x5520_002000.nef" => "2020:03:07",
    ricoh_theta_s: "R0010002.JPG" => "2015:01:01",
    ricoh_gr2_dng: "RICOH_GR2.DNG" => "2007:11:25",
    sony_nex_6: "DSC09903.ARW" => "2015:01:17",
    sony_a6000_arw: "SONY ILCE-6000 6048x4024_012003.arw" => "2014:02:18",
}

// ---- Date fallback chain: DateTimeOriginal → Digitized → DateTime → GPS → file dates ----

#[test]
fn read_exif_date_falls_back_to_datetime_digitized() {
    // given: EXIF with DateTimeDigitized but no DateTimeOriginal
    let tmp = testdir::testdir!();
    let path = tmp.join("digitized_only.tif");
    write_exif_fixture(
        &path,
        &[ascii_field(exif::Tag::DateTimeDigitized, "2016:03:04 10:11:12")],
    );
    let image = Image::new(path.clone(), path);

    // when
    let result = image.read_exif_date();

    // then
    let (date, source) = result.expect("expected fallback to DateTimeDigitized");
    assert_eq!(date, NaiveDate::from_ymd_opt(2016, 3, 4).unwrap());
    assert_eq!(source, DateSource::ExifDateTimeDigitized);
}

#[test]
fn read_exif_date_falls_back_to_datetime_tag() {
    // given: EXIF with only the plain DateTime tag (0x0132) — the only date
    // tag many old cameras write
    let tmp = testdir::testdir!();
    let path = tmp.join("datetime_only.tif");
    write_exif_fixture(
        &path,
        &[ascii_field(exif::Tag::DateTime, "2015:06:07 08:09:10")],
    );
    let image = Image::new(path.clone(), path);

    // when
    let result = image.read_exif_date();

    // then
    let (date, source) = result.expect("expected fallback to DateTime");
    assert_eq!(date, NaiveDate::from_ymd_opt(2015, 6, 7).unwrap());
    assert_eq!(source, DateSource::ExifDateTime);
}

#[test]
fn read_exif_date_skips_implausible_epoch_date() {
    // A camera with a dead clock battery writes 1970-01-01 into
    // DateTimeOriginal. Trusting it files every affected photo into a
    // confidently-wrong 1970/ folder; the chain must skip it and use the
    // next plausible source instead.
    let tmp = testdir::testdir!();
    let path = tmp.join("epoch_original.tif");
    write_exif_fixture(
        &path,
        &[
            ascii_field(exif::Tag::DateTimeOriginal, "1970:01:01 00:00:00"),
            ascii_field(exif::Tag::DateTime, "2018:09:10 11:12:13"),
        ],
    );
    let image = Image::new(path.clone(), path);

    // when
    let result = image.read_exif_date();

    // then
    let (date, source) = result.expect("expected implausible date to be skipped");
    assert_eq!(date, NaiveDate::from_ymd_opt(2018, 9, 10).unwrap());
    assert_eq!(source, DateSource::ExifDateTime);
}

#[test]
fn read_exif_date_falls_back_to_gps_date() {
    // given: EXIF with only GPSDateStamp — GPS time comes from the satellite
    // fix and is correct even when the camera clock was never set
    let tmp = testdir::testdir!();
    let path = tmp.join("gps_only.tif");
    write_exif_fixture(
        &path,
        &[ascii_field(exif::Tag::GPSDateStamp, "2019:08:15")],
    );
    let image = Image::new(path.clone(), path);

    // when
    let result = image.read_exif_date();

    // then
    let (date, source) = result.expect("expected fallback to GPSDateStamp");
    assert_eq!(date, NaiveDate::from_ymd_opt(2019, 8, 15).unwrap());
    assert_eq!(source, DateSource::ExifGpsDate);
}

#[test]
fn extract_date_falls_back_to_file_date_without_exif() {
    // Recovered files often have no EXIF at all. extract_date must fall back
    // to filesystem timestamps and flag the result as low confidence so the
    // caller can warn — file dates on recovered media reflect the recovery
    // run, not the capture.
    let tmp = testdir::testdir!();
    let path = tmp.join("no_exif.jpg");
    std::fs::write(&path, b"not a real image").unwrap();
    let image = Image::new(path.clone(), path);

    // when
    let result = image.extract_date();

    // then
    let (date, source) = result.expect("expected fallback to file dates");
    assert!(source.is_low_confidence());
    assert_eq!(date, chrono::Utc::now().date_naive());
}

#[test]
fn is_plausible_date_rejects_epoch_and_future() {
    assert!(!Image::is_plausible_date(
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
    ));
    assert!(!Image::is_plausible_date(
        chrono::Utc::now().date_naive() + chrono::Days::new(365)
    ));
    assert!(Image::is_plausible_date(
        NaiveDate::from_ymd_opt(1985, 6, 1).unwrap()
    ));
    assert!(Image::is_plausible_date(chrono::Utc::now().date_naive()));
}

// ---- Panic safety: Image::new must not panic on edge-case paths ----

#[test]
fn image_new_does_not_panic_on_root_path() {
    // Paths like "/" have no file stem and no parent. The unwrap() calls on
    // file_stem() and parent() panic and abort the entire sort run for all
    // remaining files — not just the one bad path. Image::new must handle
    // these paths without panicking (skip or propagate as error).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Image::new(PathBuf::from("/"), PathBuf::from("/tmp"))
    }));
    assert!(
        result.is_ok(),
        "Image::new panicked on root path; must return a default or Err instead"
    );
}

// ---- Error propagation: File::open failures must not panic ----

#[test]
fn extract_creation_date_returns_error_for_missing_file() {
    // File::open inside extract_file_creation_date uses .unwrap(), which
    // panics when the file cannot be opened. Missing files are a normal
    // runtime condition (deleted between directory scan and processing).
    // The error must propagate as Err so the caller can skip the file
    // rather than terminating the entire sort run with a panic.
    let path = Path::new("this_file_does_not_exist.jpg");
    let image = Image::new(path.to_path_buf(), PathBuf::from("/tmp"));
    let result = image.extract_file_creation_date();
    assert!(result.is_err(), "expected Err for missing file, got Ok");
}

#[test]
fn extract_modified_date_returns_error_for_missing_file() {
    // Same as extract_creation_date: File::open must use ? instead of
    // unwrap() so that a missing or inaccessible file returns Err rather
    // than panicking and aborting the process.
    let path = Path::new("this_file_does_not_exist.jpg");
    let image = Image::new(path.to_path_buf(), PathBuf::from("/tmp"));
    let result = image.extract_file_modified_date();
    assert!(result.is_err(), "expected Err for missing file, got Ok");
}

// ---- Discarded errors: move_to_target must propagate failures as Err ----

#[test]
fn move_to_target_returns_error_when_source_does_not_exist() {
    // move_to_target currently logs copy/remove failures and returns Ok(()).
    // Returning Ok on failure masks data loss: the caller has no way to
    // detect that the file was not moved and cannot retry or warn the user.
    let tmp = testdir::testdir!();
    let target = tmp.join("sorted");

    let source = tmp.join("ghost.jpg"); // intentionally never created
    let mut image = Image::new(source, target);
    image.target_filename = "ghost".to_string();
    image.target_filetype = "jpg".to_string();

    let result = image.move_to_target(false);
    assert!(
        result.is_err(),
        "move_to_target returned Ok even though source did not exist"
    );
}

// ---- TOCTOU: target path can be claimed between set_target and move_to_target ----

#[test]
fn move_to_target_does_not_overwrite_file_claimed_after_set_target() {
    // set_target checks exists() to pick a unique filename, but that check
    // becomes stale the moment the function returns. Another process (or a
    // parallel sort run) can create a file at the chosen path before
    // move_to_target executes. move_to_target must re-verify or use an
    // atomic create so it never silently overwrites an existing file.
    let tmp = testdir::testdir!();
    let source_dir = tmp.join("source");
    let target_dir = tmp.join("sorted");
    std::fs::create_dir_all(&source_dir).unwrap();

    let source_file = source_dir.join("photo.jpg");
    std::fs::write(&source_file, b"image data").unwrap();

    let mut image = Image::new(source_file, target_dir.clone());
    image.target_filename = "photo".to_string();
    image.target_filetype = "jpg".to_string();

    let date = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
    let (chosen_dir, chosen_filename) = image.set_target(date).unwrap();

    // Simulate another process claiming the path between set_target and move
    let claimed = chosen_dir.join(format!("{chosen_filename}.jpg"));
    std::fs::create_dir_all(&chosen_dir).unwrap();
    std::fs::write(&claimed, b"other content").unwrap();

    image.target_dir = chosen_dir;
    image.target_filename = chosen_filename;
    let _ = image.move_to_target(false);

    let content = std::fs::read(&claimed).unwrap();
    assert_eq!(
        content, b"other content",
        "move_to_target overwrote a file that was created after set_target (TOCTOU)"
    );
}

/// create a test with the given name and extract the date from the exif metadata of the given file.
/// ```rust
/// test_datetime_extraction_cases! {
///     test_name: "Filename.ext" => "1999:03:27"
/// }
/// ```
#[macro_export]
macro_rules! test_datetime_extraction_cases {
    ($($name:ident: $target:expr => $want:expr,)+) => {
        $(
        #[test]
        fn $name() {
        // given
        let path_str = format!("./tests/data/exif/{}", $target);
        let path = Path::new(&path_str);
        let file = exif_sorter::sorter::image::Image::new(path.to_path_buf(), path.to_path_buf());

        // when
        let result = file.read_exif();

        // then
        assert!(result.is_ok());
        let expected: String = format!("{}", $want).to_string();
        let expected_date: chrono::NaiveDate = chrono::NaiveDate::parse_from_str(&expected, "%Y:%m:%d").expect("failed");
        assert_eq!(result.expect("failed 2"), expected_date);
        }
        )+
    };
}
