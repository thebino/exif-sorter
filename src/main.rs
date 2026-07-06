#![forbid(unsafe_code)]
#![allow(unused)]
#![allow(unused_imports, unused_variables)]
#![allow(deprecated)]
use clap::Parser;
use cli::args::Args;
use cli::commands::Commands;
use std::path::PathBuf;
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
mod worker;

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
            // Undo a previous run
            Commands::Revert(revert_args) => {
                if let Err(e) = cli::run_revert(revert_args) {
                    eprintln!("error: {e:#}");
                    process::exit(1);
                }
            }
            Commands::Completions { shell } => {
                use clap::CommandFactory as _;
                let mut cmd = Args::command();
                clap_complete::generate(*shell, &mut cmd, "exif-sorter", &mut std::io::stdout());
            }
            Commands::Manpage => {
                use clap::CommandFactory as _;
                let man = clap_mangen::Man::new(Args::command());
                if let Err(e) = man.render(&mut std::io::stdout()) {
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

/// Per-user directory for the rotating log file. Must be writable regardless
/// of the working directory: GUI launchers (macOS Finder, Linux desktop
/// files) start the process with the cwd set to `/`, so a cwd-relative
/// `./logs` would be unwritable and previously crashed the app at startup.
fn log_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join("Library").join("Logs").join("exif-sorter"))
    } else {
        // Linux: XDG_STATE_HOME; Windows: %LOCALAPPDATA%.
        dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .map(|d| d.join("exif-sorter").join("logs"))
    }
}

/// Initialise logging. The file layer is best-effort: if the log directory
/// cannot be created, logging degrades to stdout-only instead of panicking —
/// a failed log must never prevent the app from starting.
fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_thread_ids(false)
        .with_target(false)
        .with_file(false)
        .with_ansi(true)
        .with_line_number(false)
        .without_time();

    // Only build the file layer if we can actually create the directory.
    let (file_layer, guard) = match log_dir().filter(|dir| std::fs::create_dir_all(dir).is_ok()) {
        Some(dir) => {
            let (writer, guard) = tracing_appender::non_blocking(rolling::daily(dir, "exif-sorter.log"));
            let layer = tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_thread_ids(false)
                .with_target(false)
                .with_file(false)
                .with_ansi(false)
                .with_line_number(false)
                .with_filter(LevelFilter::DEBUG);
            (Some(layer), Some(guard))
        }
        None => (None, None),
    };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    // `Option<Layer>` is itself a `Layer` (no-op when `None`), so the file
    // layer can be added unconditionally.
    tracing_subscriber::Registry::default()
        .with(env_filter)
        .with(stdout_layer.with_filter(LevelFilter::WARN))
        .with(file_layer)
        .init();

    debug!("Logging initialized");

    guard
}
