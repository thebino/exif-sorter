use chrono::NaiveDate;

use super::AppError;

pub struct Image {
    source_path: String,
    source_filename: String,
    source_filetype: String,
    dates: ImageDates,
    target_path: String,
    target_filename: String,
    target_filetype: String,
    error: Option<AppError>,
}

pub struct ImageDates {
    exif_date_time_original: Option<NaiveDate>,
    file_creation_date: NaiveDate,
    file_modified_date: NaiveDate,
}
