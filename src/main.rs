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

mod cli;
mod tui;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.command.is_some() {
        if let Err(e) = tui::run_tui().await {
            eprintln!("Error running TUI: {}", e);
            std::process::exit(1);
        }
    } else {
        cli::run_cli(args);
    }
}
