use clap::Subcommand;

use crate::cli::args::{CliArgs, RevertArgs};

#[derive(Subcommand)]
pub enum Commands {
    /// Displays a Terminal user interface
    Tui,

    ///  Uses the Command line interface
    Cli(CliArgs),

    /// Undo a previous run using its manifest file
    Revert(RevertArgs),

    /// Generate shell completions (used by the release pipeline)
    #[command(hide = true)]
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Generate the man page (used by the release pipeline)
    #[command(hide = true)]
    Manpage,
}
