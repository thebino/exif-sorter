use anyhow::{bail, Result};
use chrono::ParseError;
use clap::Parser;
use colored::*;
use exif::{Exif, In, Tag};
use std::fs::{self, DirEntry, File};
use std::ops::Not;
use std::path::Path;

use crate::sorter;

use self::args::Args;

pub mod args;
pub mod commands;

pub fn run_cli(args: Args) -> Result<()> {
    match Path::new(&args.source_dir).exists() {
        true => sorter::parse_dir(
            args.dry_run,
            Path::new(&args.source_dir),
            &sorter::handle_file,
        ),
        false => {
            bail!(sorter::AppError::InvalidSource {
                expected: args.source_dir
            })
        }
    }
}
