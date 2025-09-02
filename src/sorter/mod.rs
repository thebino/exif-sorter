#![allow(unused)]

use anyhow::{bail, Error, Result};
use chrono::{NaiveDate, NaiveDateTime, ParseError};
use clap::Parser;
use colored::*;
use dates::Dates;
use dir::scan_dir;
use exif::{Exif, In, Tag};
use fs::metadata;
use rand::prelude::*;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, DirEntry, File};
use std::ops::Not;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::UNIX_EPOCH;
use tracing::error;
use walkdir::WalkDir;

use crate::error::AppError;

use self::image::Image;

pub mod config;
pub mod dates;
pub mod dir;
pub mod image;

/// Search for file duplicates and change the target filename to a unique one to prevent overwriting
pub fn _search_for_duplicates(files: &mut [Image]) -> Result<()> {
    let mut checked_files: HashMap<DuplicateKey, Image> = HashMap::new();

    #[derive(PartialEq, Eq, Hash)]
    struct DuplicateKey {
        pub target_path: PathBuf,
        pub target_filename: String,
        pub target_filetype: String,
    }

    for image in files.iter_mut() {
        let key = DuplicateKey {
            target_path: image.target_dir.clone(),
            target_filename: image.target_filename.clone(),
            target_filetype: image.target_filetype.clone(),
        };

        // add a random number as postfix for duplicates
        if checked_files.contains_key(&key) {
            let filename = image.target_filename.clone();
            let mut rng = rand::thread_rng();

            let random = rng.gen_range(1..999999);
            image.target_filename = format!("{filename}_{random}");
        }
    }

    Ok(())
}

/// read file metadata and check for exif information
fn _read_exif(file: File) -> Result<Exif, exif::Error> {
    let mut bufreader = std::io::BufReader::new(&file);
    let exifreader = exif::Reader::new();
    exifreader.read_from_container(&mut bufreader)
}

// parse the `DateTimeOriginal` field from Exif as date string
#[deprecated(since = "0.2.0", note = "please use `Image::extract_datetime` instead")]
fn extract_datetimeoriginal_from_exif(exif: Exif) -> Result<NaiveDateTime> {
    let date = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY);
    match date {
        Some(date) => {
            let date_str = date.display_value().to_string();

            let datetime = chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S");

            match datetime {
                Ok(datetime) => Ok(datetime),
                Err(e) => {
                    bail!(AppError::DateTimeParsingEror(e))
                }
            }
        }
        None => {
            bail!(AppError::NoDateTimeOriginalFound())
        }
    }
}

#[deprecated(since = "0.2.0", note = "please use `Image::extract_datetime` instead")]
fn extract_file_creation_date(file: &File) -> Result<NaiveDate> {
    let system_time = file.metadata()?.created()?;
    let duration_since_epoch = system_time
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let secs = duration_since_epoch.as_secs() as i64;
    let nanos = duration_since_epoch.subsec_nanos();

    Ok(NaiveDateTime::from_timestamp_opt(secs, nanos)
        .unwrap_or_else(|| NaiveDateTime::from_timestamp(secs, nanos))
        .date())
}

#[deprecated(since = "0.2.0", note = "please use `Image::extract_datetime` instead")]
fn extract_file_modified_date(file: &File) -> Result<NaiveDate> {
    let system_time = file.metadata()?.modified()?;
    let duration_since_epoch = system_time
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let secs = duration_since_epoch.as_secs() as i64;
    let nanos = duration_since_epoch.subsec_nanos();

    Ok(NaiveDateTime::from_timestamp_opt(secs, nanos)
        .unwrap_or_else(|| NaiveDateTime::from_timestamp(secs, nanos))
        .date())
}
/// Move the given file into the given directory
///
/// creating the directory if it does not exist yet.
fn _move_file_to_dir(source_dir: &str, target_dir: &str, filename: &str, _path: &Path) {
    // TODO: move file if no duplicate
    // TODO: check for target_dir from args
    println!("{:<100} {}/{}", source_dir, target_dir.red(), filename)
}
