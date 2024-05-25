#![allow(unused)]
pub mod image;

use anyhow::{bail, Result};
use chrono::ParseError;
use clap::Parser;
use colored::*;
use exif::{Exif, In, Tag};
use std::fs::{self, DirEntry, File};
use std::ops::Not;
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Invalid source directory: {expected:?} could not be found!")]
    InvalidSource { expected: String },
    #[error("No exif information!")]
    NoExifInformation(),
    #[error("No DateTimeOriginal found!")]
    NoDateTimeOriginalFound(),
    #[error("Could not parse {0} from DateTimeOriginal!")]
    DateTimeParsingEror(ParseError),
    #[error("Intermittent IO error during iteration")]
    IntermittentIO(),
}

/// read entries from given directory recursively
pub fn parse_dir(dry_run: bool, dir: &Path, cb: &dyn Fn(DirEntry) -> Result<String>) -> Result<()> {
    if dir.is_dir() {
        match fs::read_dir(dir) {
            Ok(read_dir) => {
                for entry in read_dir {
                    let entry = entry?;
                    let target_name = &entry.file_name().into_string().unwrap();
                    let source_name = format!("{}/{}", dir.to_str().unwrap_or("."), target_name);
                    let path = entry.path();
                    if path.is_dir() {
                        parse_dir(dry_run, &path, cb)?;
                    } else {
                        let result = cb(entry);
                        match result {
                            Ok(result) => {
                                if dry_run.not() {
                                    move_file_to_dir(
                                        dir.to_str().unwrap_or("."),
                                        result.as_str(),
                                        target_name,
                                        path.as_path(),
                                    );
                                } else {
                                    println!(
                                        "{:<100} {}/{}",
                                        source_name,
                                        result.green(),
                                        target_name
                                    )
                                }
                            }
                            Err(e) => {
                                println!(
                                    "{:<100} {}",
                                    source_name,
                                    e.to_string().truecolor(128, 128, 128)
                                )
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!(
                    "{:<100} {}",
                    dir.to_str().unwrap().to_string().red(),
                    e.to_string().red()
                )
            }
        }
    }
    Ok(())
}

/// check for exif metadata and move file
pub fn handle_file(entry: DirEntry) -> Result<String> {
    let file = std::fs::File::open(entry.path())?;

    let exif = read_exif(file);
    match exif {
        Ok(exif) => parse_date_from_exif(exif),
        Err(_) => {
            bail!(AppError::NoExifInformation())
        }
    }
}

/// read file metadata and check for exif information
fn read_exif(file: File) -> Result<Exif, exif::Error> {
    let mut bufreader = std::io::BufReader::new(&file);
    let exifreader = exif::Reader::new();
    exifreader.read_from_container(&mut bufreader)
}

// parse the `DateTimeOriginal` field from Exif as date string
pub fn parse_date_from_exif(exif: Exif) -> Result<String> {
    let date = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY);
    match date {
        Some(date) => {
            let date_str = date.display_value().to_string();

            let datetime = chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S");

            match datetime {
                Ok(datetime) => Ok(format!("{}", datetime.date())),
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

/// Move the given file into the given directory
///
/// creating the directory if it does not exist yet.
fn move_file_to_dir(source_dir: &str, target_dir: &str, filename: &str, _path: &Path) {
    // TODO: move file if no duplicate
    // TODO: check for target_dir from args
    println!("{:<100} {}/{}", source_dir, target_dir.red(), filename)
}
