use std::io::IsTerminal;
use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::args::{CliArgs, CollisionArg, RevertArgs};
use crate::sorter::config::SorterConfig;
use crate::sorter::{self, CollisionPolicy, ProcessOptions, TransferMode};

use self::args::Args;

pub mod args;
pub mod commands;

pub fn run_cli(args: &Args, cli_args: &CliArgs) -> anyhow::Result<()> {
    // Precedence: command-line flag > config file > built-in default.
    let config = SorterConfig::load(cli_args.config.as_deref().map(Path::new));

    let collision = cli_args
        .on_collision
        .map(|arg| match arg {
            CollisionArg::Suffix => CollisionPolicy::Suffix,
            CollisionArg::Skip => CollisionPolicy::Skip,
            CollisionArg::Dedupe => CollisionPolicy::Dedupe,
        })
        .or_else(|| match config.on_collision.as_deref() {
            Some("suffix") => Some(CollisionPolicy::Suffix),
            Some("skip") => Some(CollisionPolicy::Skip),
            Some("dedupe") => Some(CollisionPolicy::Dedupe),
            Some(other) => {
                eprintln!("warning: unknown on_collision '{other}' in config, using suffix");
                None
            }
            None => None,
        })
        .unwrap_or_default();

    let options = ProcessOptions {
        dry_run: cli_args.dry_run,
        mode: if cli_args.move_files || config.move_files.unwrap_or(false) {
            TransferMode::Move
        } else {
            TransferMode::Copy
        },
        collision,
        pattern: cli_args
            .pattern
            .clone()
            .or(config.pattern)
            .unwrap_or_else(|| sorter::config::DEFAULT_PATTERN.to_string()),
    };
    let is_move = options.mode == TransferMode::Move;

    // Live progress on stderr only when attached to a terminal — keeps
    // piped/CI output clean.
    let bar = if std::io::stderr().is_terminal() {
        let bar = ProgressBar::no_length();
        bar.set_style(
            ProgressStyle::with_template(
                "{bar:40.green/black} {pos}/{len} ({percent}%) elapsed {elapsed} eta {eta}",
            )
            .expect("static template is valid"),
        );
        Some(bar)
    } else {
        None
    };
    let summary = sorter::process(
        Path::new(&args.source_dir),
        Path::new(&args.target_dir),
        &options,
        |done, total| {
            if let Some(bar) = &bar {
                bar.set_length(total as u64);
                bar.set_position(done as u64);
            }
        },
    )?;
    if let Some(bar) = &bar {
        bar.finish_and_clear();
    }

    let verb = match (cli_args.dry_run, is_move) {
        (true, true) => "Would move",
        (true, false) => "Would copy",
        (false, true) => "Moved",
        (false, false) => "Copied",
    };
    println!(
        "{verb} {} of {} images into '{}'.",
        summary.transferred,
        summary.total(),
        args.target_dir
    );
    if summary.low_confidence > 0 {
        println!(
            "{} of them dated from file timestamps only (no exif date — verify manually if this is recovered media).",
            summary.low_confidence
        );
    }
    if summary.unsorted > 0 {
        println!(
            "{} files without a usable date placed in 'unsorted/'.",
            summary.unsorted
        );
    }
    if summary.corrupt > 0 {
        println!(
            "{} files with unrecognizable content placed in 'corrupt/'.",
            summary.corrupt
        );
    }
    if summary.duplicates > 0 {
        println!(
            "{} exact duplicates not stored again (already in target).",
            summary.duplicates
        );
    }
    if summary.collisions_skipped > 0 {
        println!(
            "{} files left in place due to name collisions (--on-collision skip).",
            summary.collisions_skipped
        );
    }
    if !summary.failed.is_empty() {
        println!("Failed to transfer {} files:", summary.failed.len());
        for (path, reason) in &summary.failed {
            println!("  {path} ({reason})");
        }
    }
    if !cli_args.dry_run && summary.total() > 0 {
        println!(
            "Manifest: {}/{} (undo with: exif-sorter revert -m <manifest>)",
            args.target_dir,
            sorter::manifest::MANIFEST_FILENAME
        );
    }

    Ok(())
}

pub fn run_revert(revert_args: &RevertArgs) -> anyhow::Result<()> {
    let (reverted, skipped) =
        sorter::revert(Path::new(&revert_args.manifest), revert_args.dry_run)?;
    let verb = if revert_args.dry_run {
        "Would revert"
    } else {
        "Reverted"
    };
    println!("{verb} {reverted} transfers ({skipped} skipped).");
    Ok(())
}
