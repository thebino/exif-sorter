use anyhow::bail;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};
use crate::cli::args::CliArgs;
use crate::error::AppError;
use crate::sorter::dir::scan_dir;
use crate::sorter::image::Image;

use self::args::Args;

pub mod args;
pub mod commands;

pub fn run_cli(args: &Args, cli_args: &CliArgs) -> anyhow::Result<()> {
    let source_path = Path::new(&args.source_dir);
    let target_path = Path::new(&args.target_dir);
    fs::create_dir_all(target_path)?;

    match source_path.exists() {
        true => {
            for entry in scan_dir(source_path)? {
                let mut image = Image::new(entry.into_path(), target_path.to_path_buf());

                match image.extract_date() {
                    Ok((date, source)) => {
                        if source.is_low_confidence() {
                            warn!(
                                "File '{}': no exif date, using {source} '{date}' (unreliable on recovered media)",
                                image.source_full()
                            );
                        } else {
                            debug!(
                                "File '{}' has date '{date}' from {source}",
                                image.source_full()
                            );
                        }
                        let (target_path, target_filename) = image.set_target(date)?;
                        image.target_dir = target_path;
                        image.target_filename = target_filename;
                        if cli_args.immediate {
                            let source = image.source_full();
                            if let Err(e) = image.move_to_target(cli_args.dry_run) {
                                warn!("Failed to move '{source}': {e:#}");
                            }
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
