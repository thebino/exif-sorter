use std::path::Path;

use chrono::NaiveDate;
use exif_sorter::sorter::image::Image;

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
    let expected = NaiveDate::from_ymd_opt(2025, 9, 2).unwrap();

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
