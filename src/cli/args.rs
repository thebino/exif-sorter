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
    #[arg(short, long, default_value = ".", global = true)]
    pub source_dir: String,

    /// Base directory to move source files into.
    #[arg(short, long, default_value = "./sorted", global = true)]
    pub target_dir: String,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Parser)]
#[clap(version, author, help_template = HELP_TEMPLATE, about, long_about)]
pub struct CliArgs {
    /// Deprecated, no effect: processing is always two-phase now
    /// (scan and date all files first, then move them).
    #[arg(long)]
    pub immediate: bool,

    /// Include target directory while searching for exif data.
    #[arg(long)]
    pub include_target: bool,

    /// Print findings to the output instead of touching any file.
    #[arg(long)]
    pub dry_run: bool,

    /// Move files into the target instead of copying them.
    /// Default is copy: the source stays untouched.
    #[arg(long = "move")]
    pub move_files: bool,

    /// What to do when the target filename already exists
    /// (default: suffix; may also come from the config file).
    #[arg(long, value_enum)]
    pub on_collision: Option<CollisionArg>,

    /// Folder layout below the target directory. Tokens: {year}, {month},
    /// {day}, {date}. Default: "{year}/{date}".
    #[arg(long)]
    pub pattern: Option<String>,

    /// Path to a config file (default: ~/.config/exif-sorter/config.toml).
    #[arg(long)]
    pub config: Option<String>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum CollisionArg {
    /// Append a random suffix and store both files (default).
    Suffix,
    /// Leave the colliding source file where it is.
    Skip,
    /// Byte-compare; identical files are recorded as duplicates and not
    /// stored twice, different content gets a suffix.
    Dedupe,
}

#[derive(Parser)]
#[clap(version, author, help_template = HELP_TEMPLATE, about, long_about)]
pub struct RevertArgs {
    /// Path to the manifest CSV written by a previous run.
    #[arg(short, long)]
    pub manifest: String,

    /// Print what would be reverted without touching any file.
    #[arg(long)]
    pub dry_run: bool,
}
