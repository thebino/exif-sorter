use clap::Subcommand;

use crate::cli::args::CliArgs;

#[derive(Subcommand)]
pub enum Commands {
    /// Displays a Terminal user interface
    Tui,

    ///  Uses the Command line interface
    Cli(CliArgs),
}
