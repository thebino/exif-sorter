pub struct Image {
    source_path: &str,
    source_filename: &str,
    source_filetype: &str,
    dates: ImageDates,
    target_path: &str,
    target_filename: &str,
    target_filetype: &str,
    error: Option<AppError>,
}

pub struct ImageDates {
    exif_date_time_original: Option<NaiveDate>,
    file_creation_date: NaiveDate,
    file_modified_date: NaiveDate,
}
