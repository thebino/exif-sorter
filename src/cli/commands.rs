use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Display a Terminal user interface
    Tui,
}
