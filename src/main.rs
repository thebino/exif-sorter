#![forbid(unsafe_code)]
use anyhow::{bail, Error, Result};
use clap::Parser;
use colored::*;
use exif::{In, Tag};
use std::fs::{self, File};
use std::ops::Not;
use std::path::Path;

const HELP_TEMPLATE: &str = "\
{name} - {version}
{about-section}
{author}
https://github.com/thebino/exif-tool

{usage-heading}
{tab}{usage}

{all-args}{after-help}
";

#[derive(Debug, Parser)]
#[clap(version, author, help_template = HELP_TEMPLATE, about, long_about)]
struct Args {
    /// Directory to walk through and search for exif data
    #[arg(short, long, default_value = ".")]
    source_dir: String,

    /// Base directory to move source files into
    #[arg(short, long, default_value = "sorted")]
    target_dir: String,

    /// Create a file with results instead of moving
    #[arg(long)]
    dry_run: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Invalid source directory: {expected:?} could not be found!")]
    InvalidSource { expected: String },
    #[error("Could not read path: {0}")]
    CouldNotReadPath(Error),
    #[error("Intermittent IO error during iteration")]
    IntermittentIO(),
}

#[tokio::main]
async fn main() {
    if let Err(e) = run(Args::parse()).await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<()> {
    //    args.dry_run

    match Path::new(&args.source_dir).exists() {
        true => {
            let entries = fs::read_dir(args.source_dir);

            match entries {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                let name: String = entry.file_name().into_string().unwrap();
                                let file = std::fs::File::open(entry.path())?;
                                let mut bufreader = std::io::BufReader::new(&file);
                                let exifreader = exif::Reader::new();
                                let exif = exifreader.read_from_container(&mut bufreader);
                                match exif {
                                    Ok(exif) => {
                                        let date =
                                            exif.get_field(Tag::DateTimeOriginal, In::PRIMARY);
                                        match date {
                                            Some(date) => {
                                                let date_str = date.display_value().to_string();

                                                let datetime =
                                                    chrono::NaiveDateTime::parse_from_str(
                                                        &date_str,
                                                        "%Y-%m-%d %H:%M:%S",
                                                    )
                                                    .unwrap();

                                                let target_dir = format!(
                                                    "{}/{}",
                                                    &args.target_dir,
                                                    datetime.date()
                                                );
                                                if args.dry_run.not() {
                                                    move_file_to_dir(&name, file, &target_dir);
                                                } else {
                                                    // TODO: print only
                                                    println!(
                                                        "{:<25} {}/{}",
                                                        name,
                                                        target_dir.green(),
                                                        &name
                                                    );
                                                }
                                            }
                                            None => println!("No DateTimeOriginal found!"),
                                        }
                                    }
                                    Err(_) => {
                                        println!("{:<25} No image or invalid image format!", name)
                                    }
                                }
                            }
                            Err(_) => {
                                bail!(AppError::IntermittentIO())
                            }
                        }
                    }
                }
                Err(e) => {
                    bail!(AppError::CouldNotReadPath(e.into()))
                }
            }

            Ok(())
        }
        false => {
            bail!(AppError::InvalidSource {
                expected: args.source_dir
            })
        }
    }
}

/// Check if the given file contains exif data at all.
fn _has_exif_data(_file: File) -> Result<String> {
    bail!("No exif data found!")
}

fn _filename_contains_date_information(_file: &File) -> Result<String> {
    bail!("No date info found in filename!")
}

/// Move the given file into the given directory
///
/// creating the directory if it does not exist yet.
fn move_file_to_dir(name: &str, _file: File, dir: &str) {
    // TODO: move file
    println!("{} {} will be moved to {}", "WARNING".red(), name, dir);
}
