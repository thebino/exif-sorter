use anyhow::Result;
use std::io::{self, stdout, Stdout};

use crossterm::{execute, terminal::*};
use ratatui::prelude::*;

mod app;
mod events;
mod ui;
use app::App;

use crate::cli::args::Args;

pub async fn run_tui(args: Args) -> Result<()> {
    let terminal = init()?;

    App::new(args.source_dir, args.target_dir).run(terminal)?;

    restore()?;

    Ok(())
}

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> io::Result<Tui> {
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

pub fn restore() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
