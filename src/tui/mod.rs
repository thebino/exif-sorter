use anyhow::{bail, Result};
use chrono::ParseError;
use clap::Parser;
use colored::*;
use exif::{Exif, In, Tag};
use std::fs::{self, DirEntry, File};
use std::io::{self, stdout, Stdout};
use std::ops::Not;
use std::path::Path;

use crossterm::{execute, terminal::*};
use ratatui::prelude::*;

use ratatui::widgets::Widget;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

mod app;

use app::App;

pub async fn run_tui() -> Result<()> {
    let terminal = init()?;

    App::new().run(terminal).await?;

    restore();
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
