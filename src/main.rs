#![forbid(unsafe_code)]
#![allow(unused)]
#![allow(unused_imports, unused_variables)]
#![allow(deprecated)]
use clap::Parser;
use cli::args::Args;
use cli::commands::Commands;
use std::process;
use tracing::{debug, error, info, warn};
use tracing_appender::rolling;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

mod cli;
pub mod error;
mod gui;
pub mod sorter;
mod tui;

const LOGGING_PATH: &str = "./logs";

#[tokio::main]
async fn main() {
    let _guard = init_logging();

    let args = Args::parse();

    if let Some(command) = &args.command {
        match command {
            // Run TUI
            Commands::Tui => {
                if let Err(e) = tui::run_tui(args).await {
                    eprintln!("Error running TUI: {e}");
                    std::process::exit(1);
                }
            }
            // Run CLI
            Commands::Cli(cli_args) => {
                if let Err(e) = cli::run_cli(&args, cli_args) {
                    eprintln!("error: {e:#}");
                    process::exit(1);
                }
            }
        }
    } else {
        // Run GUI
        if let Err(e) = gui::run_gui(args) {
            eprintln!("error: {e:#}");
            process::exit(1);
        }
    }
}

fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {

    let file_appender = rolling::daily(LOGGING_PATH, "exif-sorter.log");
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_thread_ids(false)
        .with_target(false)
        .with_file(false)
        .with_ansi(true)
        .with_line_number(false)
        .without_time();

    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_thread_ids(false)
        .with_target(false)
        .with_file(false)
        .with_ansi(false)
        .with_line_number(false);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::Registry::default()
        .with(stdout_layer.with_filter(LevelFilter::WARN))
        .with(file_layer.with_filter(LevelFilter::DEBUG))
        .init();

    debug!("Logging initialized");

    guard
}
