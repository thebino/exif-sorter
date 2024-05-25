#![forbid(unsafe_code)]
#![allow(unused)]
use anyhow::{bail, Result};
use chrono::ParseError;
use clap::Parser;
use cli::args::Args;
use colored::*;
use exif::{Exif, In, Tag};
use std::fs::{self, DirEntry, File};
use std::ops::Not;
use std::path::Path;
use std::process;

mod cli;
mod sorter;
mod tui;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Some(command) = args.command {
        if let Err(e) = tui::run_tui(args.source_dir, args.target_dir).await {
            eprintln!("Error running TUI: {}", e);
            std::process::exit(1);
        }
    } else if let Err(e) = cli::run_cli(args) {
        eprintln!("error: {:#}", e);
        process::exit(1);
    }
}
