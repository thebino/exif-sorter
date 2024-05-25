use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Displays a Terminal user interface
    Tui,
}
