use anyhow::bail;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};
use walkdir::DirEntry;

use crate::cli::args::CliArgs;
use crate::error::AppError;
use crate::sorter;
use crate::sorter::dir::scan_dir;
use crate::sorter::image::Image;

use self::args::Args;

pub mod args;
pub mod commands;

pub fn run_cli(args: &Args, cli_args: &CliArgs) -> anyhow::Result<()> {
    let source_path = Path::new(&args.source_dir);
    let target_path = Path::new(&args.target_dir);
    fs::create_dir(target_path);

    match source_path.exists() {
        true => {
            let mut files: Vec<ignore::DirEntry> = scan_dir(source_path).unwrap();
            for entry in files {
                let mut image = Image::new(entry.into_path(), target_path.to_path_buf());

                let date = image.read_exif();
                match date {
                    Ok(date) => {
                        debug!(
                            "File '{}' has date '{}'",
                            image.source_full(),
                            &date.to_string()
                        );
                        let (target_path, target_filename) = image.set_target(date)?;
                        image.target_dir = target_path;
                        image.target_filename = target_filename;
                        if cli_args.immediate {
                            image.move_to_target(cli_args.dry_run);
                        }
                    }
                    Err(e) => {
                        warn!("File '{}' can't read! ({e:#})", image.source_full());
                    }
                }
            }

            if !cli_args.immediate {
                // TODO: add non-immediate mode
                info!("non-immediate mode not implemeted yet!");
            }

            Ok(())
        }
        false => {
            bail!(AppError::InvalidSource {
                expected: args.source_dir.clone()
            })
        }
    }
}
