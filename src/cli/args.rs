use clap::Parser;

use super::commands::Commands;

const HELP_TEMPLATE: &str = "\
{name} - {version}
{about-section}
{author}
https://github.com/thebino/exif-sorter

{usage-heading}
{tab}{usage}

{all-args}{after-help}
";

#[derive(Parser)]
#[clap(version, author, help_template = HELP_TEMPLATE, about, long_about)]
pub struct Args {
    /// Directory to walk through and search for exif data.
    #[arg(short, long, default_value = ".")]
    pub source_dir: String,

    /// Base directory to move source files into.
    #[arg(short, long, default_value = "./sorted")]
    pub target_dir: String,

    /// Include target directory while searching for exif data.
    #[arg(long)]
    pub include_target: bool,

    /// Print findings to the output instead of moving any file.
    #[arg(long)]
    pub dry_run: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}
