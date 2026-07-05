use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use exif_sorter::sorter::image::{DateSource, Image};
use exif_sorter::sorter::TransferMode;

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
    // Git checkouts do not preserve file timestamps, so this test must not
    // assert against a fixed date of a tracked file (breaks on every fresh
    // clone / CI runner). A freshly created file has today's creation date.
    // given
    let tmp = testdir::testdir!();
    let path = tmp.join("fresh.jpg");
    std::fs::write(&path, b"x").unwrap();
    let file = Image::new(path.clone(), path);

    // when
    let date = file.extract_file_creation_date();

    // then
    assert!(date.is_ok());
    assert_eq!(date.unwrap(), chrono::Utc::now().date_naive());
}

#[test]
fn test_extract_modification_date() {
    // Deterministic in every environment: the mtime is set explicitly
    // instead of relying on checkout timestamps.
    // given
    let tmp = testdir::testdir!();
    let path = tmp.join("dated.jpg");
    std::fs::write(&path, b"x").unwrap();
    let mtime = filetime::FileTime::from_unix_time(1_588_636_800, 0); // 2020-05-05
    filetime::set_file_mtime(&path, mtime).unwrap();
    let file = Image::new(path.clone(), path);
    let expected = NaiveDate::from_ymd_opt(2020, 5, 5).unwrap();

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
fn filename_dates_are_extracted_for_common_camera_and_messenger_patterns() {
    use exif_sorter::sorter::filename_date::date_from_filename;

    let cases = [
        ("IMG_20190412_183000", Some((2019, 4, 12))),   // Android camera
        ("PXL_20210704_123456789", Some((2021, 7, 4))), // Pixel
        ("IMG-20200105-WA0001", Some((2020, 1, 5))),    // WhatsApp
        ("signal-2020-01-05-120000", Some((2020, 1, 5))),
        ("2019-04-12 vacation", Some((2019, 4, 12))),
        ("2019_04_12_hike", Some((2019, 4, 12))),
        ("DSC_0042", None),          // frame counter, no date
        ("19700101", None),          // implausible (epoch reset)
        ("1234567890", None),        // unix timestamp digits, not a date
        ("photo", None),             // no digits at all
        ("20991231", None),          // future date
    ];
    for (stem, expected) in cases {
        let expected = expected.map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap());
        assert_eq!(
            date_from_filename(stem),
            expected,
            "unexpected result for filename stem '{stem}'"
        );
    }
}

#[test]
fn extract_date_uses_filename_before_file_timestamps() {
    // A file without EXIF but with a dated name (EXIF-stripped messenger
    // export) must be dated from the name — a deliberate stamp — instead of
    // the unreliable filesystem timestamps.
    let tmp = testdir::testdir!();
    let path = tmp.join("IMG-20200105-WA0001.jpg");
    std::fs::write(&path, b"not a real image").unwrap();
    let image = Image::new(path.clone(), path);

    let (date, source) = image.extract_date().expect("expected filename date");
    assert_eq!(date, NaiveDate::from_ymd_opt(2020, 1, 5).unwrap());
    assert_eq!(source, DateSource::Filename);
    assert!(!source.is_low_confidence());
}

/// Build a minimal MP4: an `ftyp` box and a `moov` box containing an `mvhd`
/// (version 0) with the given creation time in seconds since 1904-01-01.
fn minimal_mp4(creation_time_1904: u32) -> Vec<u8> {
    let mut mvhd_content = vec![0u8; 12];
    // version 0 + flags already zero; creation_time at offset 4
    mvhd_content[4..8].copy_from_slice(&creation_time_1904.to_be_bytes());

    let mut mvhd = Vec::new();
    mvhd.extend_from_slice(&(8 + mvhd_content.len() as u32).to_be_bytes());
    mvhd.extend_from_slice(b"mvhd");
    mvhd.extend_from_slice(&mvhd_content);

    let mut moov = Vec::new();
    moov.extend_from_slice(&(8 + mvhd.len() as u32).to_be_bytes());
    moov.extend_from_slice(b"moov");
    moov.extend_from_slice(&mvhd);

    let mut data = Vec::new();
    data.extend_from_slice(&16u32.to_be_bytes());
    data.extend_from_slice(b"ftypisom");
    data.extend_from_slice(&[0, 0, 0, 0]);
    data.extend_from_slice(&moov);
    data
}

#[test]
fn extract_date_reads_mp4_creation_time() {
    // given: 2020-05-05 00:00:00 UTC = 1588636800 unix + 2082844800 offset
    let tmp = testdir::testdir!();
    let path = tmp.join("clip.mp4");
    std::fs::write(&path, minimal_mp4(1_588_636_800 + 2_082_844_800)).unwrap();
    let image = Image::new(path.clone(), path);

    // when
    let result = image.extract_date();

    // then
    let (date, source) = result.expect("expected video creation date");
    assert_eq!(date, NaiveDate::from_ymd_opt(2020, 5, 5).unwrap());
    assert_eq!(source, DateSource::VideoCreationTime);
}

#[test]
fn mp4_with_unset_creation_time_falls_through() {
    // Cameras without a clock write creation_time 0 (= 1904-01-01), which
    // must not be trusted; the chain continues (here: filename date).
    let tmp = testdir::testdir!();
    let path = tmp.join("VID_20210704_120000.mp4");
    std::fs::write(&path, minimal_mp4(0)).unwrap();
    let image = Image::new(path.clone(), path);

    let (date, source) = image.extract_date().expect("expected filename fallback");
    assert_eq!(date, NaiveDate::from_ymd_opt(2021, 7, 4).unwrap());
    assert_eq!(source, DateSource::Filename);
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

// ---- Discarded errors: transfer_to_target must propagate failures as Err ----

#[test]
fn transfer_to_target_returns_error_when_source_does_not_exist() {
    // Returning Ok on failure masks data loss: the caller has no way to
    // detect that the file was not transferred and cannot retry or warn.
    let tmp = testdir::testdir!();
    let target = tmp.join("sorted");

    let source = tmp.join("ghost.jpg"); // intentionally never created
    let mut image = Image::new(source, target);
    image.target_filename = "ghost".to_string();
    image.target_filetype = "jpg".to_string();

    let result = image.transfer_to_target(TransferMode::Move, false);
    assert!(
        result.is_err(),
        "transfer_to_target returned Ok even though source did not exist"
    );
}

// ---- Copy mode: source must stay untouched ----

#[test]
fn transfer_to_target_copy_keeps_source() {
    // Copy is the default mode because the tool runs against just-recovered
    // data: the source must never be modified unless --move is given.
    let tmp = testdir::testdir!();
    let source_dir = tmp.join("source");
    let target_dir = tmp.join("sorted/2020/2020-05-05");
    std::fs::create_dir_all(&source_dir).unwrap();

    let source_file = source_dir.join("photo.jpg");
    std::fs::write(&source_file, b"image data").unwrap();

    let mut image = Image::new(source_file.clone(), target_dir.clone());
    image.target_filename = "photo".to_string();
    image.target_filetype = "jpg".to_string();

    image
        .transfer_to_target(TransferMode::Copy, false)
        .expect("copy failed");

    assert!(source_file.exists(), "copy mode must not remove the source");
    let copied = target_dir.join("photo.jpg");
    assert_eq!(std::fs::read(&copied).unwrap(), b"image data");
}

// ---- Timestamp preservation: sorting must not destroy the mtime signal ----

#[test]
fn transfer_to_target_preserves_modified_time() {
    // After a sort, the file's mtime is the last date signal outside of
    // EXIF (PhotoRec stamps the capture date into it). Both transfer modes
    // must keep the original modified time.
    for mode in [TransferMode::Move, TransferMode::Copy] {
        let tmp = testdir::testdir!();
        let source_dir = tmp.join(format!("source-{mode:?}"));
        let target_dir = tmp.join(format!("sorted-{mode:?}/2020/2020-05-05"));
        std::fs::create_dir_all(&source_dir).unwrap();

        let source_file = source_dir.join("photo.jpg");
        std::fs::write(&source_file, b"image data").unwrap();
        let old_mtime = filetime::FileTime::from_unix_time(1_588_636_800, 0); // 2020-05-05
        filetime::set_file_mtime(&source_file, old_mtime).unwrap();

        let mut image = Image::new(source_file, target_dir.clone());
        image.target_filename = "photo".to_string();
        image.target_filetype = "jpg".to_string();

        image.transfer_to_target(mode, false).expect("transfer failed");

        let meta = std::fs::metadata(target_dir.join("photo.jpg")).unwrap();
        let mtime = filetime::FileTime::from_last_modification_time(&meta);
        assert_eq!(
            mtime.unix_seconds(),
            old_mtime.unix_seconds(),
            "modified time was not preserved by transfer_to_target ({mode:?})"
        );
    }
}

// ---- TOCTOU: target path can be claimed between set_target and move_to_target ----

#[test]
fn transfer_to_target_does_not_overwrite_file_claimed_after_set_target() {
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
    let (chosen_dir, chosen_filename) = image
        .set_target(date, exif_sorter::sorter::config::DEFAULT_PATTERN)
        .unwrap();

    // Simulate another process claiming the path between set_target and move
    let claimed = chosen_dir.join(format!("{chosen_filename}.jpg"));
    std::fs::create_dir_all(&chosen_dir).unwrap();
    std::fs::write(&claimed, b"other content").unwrap();

    image.target_dir = chosen_dir;
    image.target_filename = chosen_filename;
    let _ = image.transfer_to_target(TransferMode::Move, false);

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
