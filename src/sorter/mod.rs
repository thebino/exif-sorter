#![allow(unused)]
pub mod image;

use anyhow::{bail, Error, Result};
use chrono::{NaiveDate, NaiveDateTime, ParseError};
use clap::Parser;
use colored::*;
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
use walkdir::WalkDir;

use self::image::{Dates, Image};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Could not parse {0} from DateTimeOriginal!")]
    DateTimeParsingEror(ParseError),
    #[error("Intermittent IO error during iteration")]
    IntermittentIO(),
    #[error("Invalid source directory: {expected:?} could not be found!")]
    InvalidSource { expected: String },
    #[error("No DateTimeOriginal found!")]
    NoDateTimeOriginalFound(),
    #[error("No exif information!")]
    NoExifInformation(),
    #[error("TODO")]
    TODO(),
}

pub struct SorterConfig {
    dry_run: bool,
    sub_dir: Option<String>,
    target_dir: Option<String>,
}

/// walk through given directory, read all images files and move the files regarding its exif date afterwards.
pub fn parse_and_move(
    dry_run: bool,
    dir: &Path,
    cb: &dyn Fn(DirEntry) -> Result<String>,
) -> Result<()> {
    let mut images = scan_dir(dir)?;
    read_exif_data(&mut images);
    set_target(&mut images);
    search_for_duplicates(&mut images);
    move_to_target(images);

    Ok(())
}

/// Scan given directory including its subdirectories and returns a list of findings including their source path
pub fn scan_dir(dir: &Path) -> Result<Vec<Image>> {
    let mut files: Vec<Image> = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|entry| entry.ok()) {
        println!(
            "{:<100}",
            entry.clone().into_path().into_os_string().to_str().unwrap()
        );

        files.push(Image::new(entry.into_path()))
    }

    Ok(files)
}

/// Read exif data for all given files and update its dates
/// * `files` - List of previous found files in the source directory.
/// * `use_file_source` - Use the `File Source` from within the Exif metadata as filename prefix. [Default: true]
/// * `use_image_number` - Use the `Image Number` from within the Exif metadata as filename. [Default: true]
pub fn read_exif_data(
    files: &mut Vec<Image>,
    use_file_source: bool,
    use_image_number: bool,
) -> Result<()> {
    for file in files.into_iter() {
        let f = std::fs::File::open(file.source_path.clone())?;

        let file_creation_date = extract_file_creation_date(&f)?;
        let file_modified_date = extract_file_modified_date(&f)?;

        let mut bufreader = std::io::BufReader::new(&f);
        let exifreader = exif::Reader::new();
        let exif = exifreader.read_from_container(&mut bufreader);

        let date_time_original = match exif {
            Ok(exif) => extract_datetimeoriginal_from_exif(exif),
            Err(_) => bail!(AppError::NoExifInformation()),
        }
        .unwrap()
        .date();

        if use_file_source {
            // TODO: add prefix
        }

        if use_image_number {
            // TODO: replace filename
        }

        file.dates = Dates {
            exif_date_time_original: Some(date_time_original),
            file_creation_date,
            file_modified_date,
        }
    }
    Ok(())
}

/// Set target fields based on configuration
pub fn set_target(files: &mut Vec<Image>, sub_dir: Option<String>) -> Result<()> {
    //
    Ok(())
}

/// Search for file duplicates and change the target filename to a unique one to prevent overwriting
pub fn search_for_duplicates(files: &mut Vec<Image>) -> Result<()> {
    let mut checked_files: HashMap<DuplicateKey, Image> = HashMap::new();

    #[derive(PartialEq, Eq, Hash)]
    struct DuplicateKey {
        pub target_path: PathBuf,
        pub target_filename: Option<String>,
        pub target_filetype: Option<String>,
    }

    for image in files.into_iter() {
        let key = DuplicateKey {
            target_path: image.target_path.clone(),
            target_filename: image.target_filename.clone(),
            target_filetype: image.target_filetype.clone(),
        };

        // add a random number as postfix for duplicates
        if checked_files.contains_key(&key) {
            let filename = image.target_filename.clone().unwrap();
            let mut rng = rand::thread_rng();

            let random = rng.gen_range(1..999999);
            image.target_filename = Some(format!("{filename}_{random}"));
        }
    }

    Ok(())
}

/// Move given files based on its target configuration
pub fn move_to_target(files: Vec<Image>, dry_run: bool) -> Result<()> {
    // TODO: move files
    for file in files.into_iter() {
        println!(
            "{}/{} => {}/{}",
            file.source_path.to_string_lossy(),
            file.source_filename.to_string(),
            file.target_path.to_string_lossy(),
            file.target_filename.unwrap().to_string()
        );
    }
    Ok(())
}

/// read entries from given directory recursively
// pub fn parse_dir_old(
//     dry_run: bool,
//     dir: &Path,
//     cb: &dyn Fn(DirEntry) -> Result<String>,
// ) -> Result<()> {
//     if dir.is_dir() {
//         match fs::read_dir(dir) {
//             Ok(read_dir) => {
//                 for entry in read_dir {
//                     let entry = entry?;
//                     let target_name = &entry.file_name().into_string().unwrap();
//                     let source_name = format!("{}/{}", dir.to_str().unwrap_or("."), target_name);
//                     let path = entry.path();
//                     if path.is_dir() {
//                         parse_dir(dry_run, &path, cb)?;
//                     } else {
//                         let result = cb(entry);
//                         match result {
//                             Ok(result) => {
//                                 if dry_run.not() {
//                                     move_file_to_dir(
//                                         dir.to_str().unwrap_or("."),
//                                         result.as_str(),
//                                         target_name,
//                                         path.as_path(),
//                                     );
//                                 } else {
//                                     println!(
//                                         "{:<100} {}/{}",
//                                         source_name,
//                                         result.green(),
//                                         target_name
//                                     )
//                                 }
//                             }
//                             Err(e) => {
//                                 println!(
//                                     "{:<100} {}",
//                                     source_name,
//                                     e.to_string().truecolor(128, 128, 128)
//                                 )
//                             }
//                         }
//                     }
//                 }
//             }
//             Err(e) => {
//                 println!(
//                     "{:<100} {}",
//                     dir.to_str().unwrap().to_string().red(),
//                     e.to_string().red()
//                 )
//             }
//         }
//     }
//     Ok(())
// }

/// check for exif metadata and move file
// pub fn handle_file(entry: DirEntry) -> Result<String> {
//     let file = std::fs::File::open(entry.path())?;

//     let exif = read_exif(file);
//     match exif {
//         Ok(exif) => parse_date_from_exif(exif),
//         Err(_) => {
//             bail!(AppError::NoExifInformation())
//         }
//     }
// }

/// read file metadata and check for exif information
fn read_exif(file: File) -> Result<Exif, exif::Error> {
    let mut bufreader = std::io::BufReader::new(&file);
    let exifreader = exif::Reader::new();
    exifreader.read_from_container(&mut bufreader)
}

// parse the `DateTimeOriginal` field from Exif as date string
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

fn extract_file_creation_date(file: &File) -> Result<NaiveDate> {
    let system_time = file.metadata()?.created()?;
    let duration_since_epoch = system_time
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let secs = duration_since_epoch.as_secs() as i64;
    let nanos = duration_since_epoch.subsec_nanos() as u32;

    Ok(NaiveDateTime::from_timestamp_opt(secs, nanos)
        .unwrap_or_else(|| NaiveDateTime::from_timestamp(secs, nanos))
        .date())
}

fn extract_file_modified_date(file: &File) -> Result<NaiveDate> {
    let system_time = file.metadata()?.modified()?;
    let duration_since_epoch = system_time
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let secs = duration_since_epoch.as_secs() as i64;
    let nanos = duration_since_epoch.subsec_nanos() as u32;

    Ok(NaiveDateTime::from_timestamp_opt(secs, nanos)
        .unwrap_or_else(|| NaiveDateTime::from_timestamp(secs, nanos))
        .date())
}
/// Move the given file into the given directory
///
/// creating the directory if it does not exist yet.
fn move_file_to_dir(source_dir: &str, target_dir: &str, filename: &str, _path: &Path) {
    // TODO: move file if no duplicate
    // TODO: check for target_dir from args
    println!("{:<100} {}/{}", source_dir, target_dir.red(), filename)
}
