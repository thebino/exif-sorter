use chrono::NaiveDate;

use super::AppError;

#[derive(Default)]
pub struct Image {
    pub source_path: String,
    pub source_filename: String,
    pub source_filetype: String,
    pub dates: Dates,
    pub target_path: String,
    pub target_filename: String,
    pub target_filetype: String,
    pub error: Option<AppError>,
}

#[derive(Default)]
pub struct Dates {
    pub exif_date_time_original: Option<NaiveDate>,
    pub file_creation_date: NaiveDate,
    pub file_modified_date: NaiveDate,
}
