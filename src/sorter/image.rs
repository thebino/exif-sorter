use std::{
    ffi::OsStr,
    fs::{self, FileType},
    path::{Path, PathBuf},
};

use chrono::NaiveDate;

use super::AppError;

pub struct Image {
    pub source_path: PathBuf,
    pub source_filename: String,
    pub source_filetype: String,
    pub dates: Dates,
    pub target_path: PathBuf,
    pub target_filename: Option<String>,
    pub target_filetype: Option<String>,
    pub error: Option<AppError>,
}

#[derive(Default)]
pub struct Dates {
    pub exif_date_time_original: Option<NaiveDate>,
    pub file_creation_date: NaiveDate,
    pub file_modified_date: NaiveDate,
}

impl Image {
    pub fn new(path: PathBuf) -> Self {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let filetype = path
            .extension()
            .unwrap_or(OsStr::new(""))
            .to_string_lossy()
            .into_owned();

        Self {
            source_path: path.clone(),
            source_filename: filename.to_string(),
            source_filetype: filetype,
            dates: Dates::default(),
            target_path: path.clone(),
            target_filename: None,
            target_filetype: None,
            error: None,
        }
    }
}
