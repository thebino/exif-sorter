use chrono::NaiveDate;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::{
    event::KeyEventKind,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::prelude::{Stylize, Terminal};
use ratatui::style::palette::tailwind;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::block::Title;
use ratatui::widgets::{
    BorderType, Clear, Gauge, ListState, Padding, Paragraph, StatefulWidget, TableState, Widget,
};
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem},
};

use std::fs::File;
use std::io::{stdout, Result};
use std::net::Ipv4Addr;
use std::path::Path;
use std::{env, io};

use crate::sorter::image::{Dates, Image};

use super::{events, ui};

// pub struct ImageFile {
//     pub(crate) source: String,
//     pub(crate) target: String,
//     pub(crate) moved: bool,
// }

/// Application state
pub struct App {
    /// exit the tui when true
    should_exit: bool,
    pub(crate) progress: Option<f64>,
    pub(crate) source_dir: String,
    pub(crate) target_dir: String,
    pub(crate) items: StatefulList,
}

pub struct StatefulList {
    pub(crate) state: TableState,
    pub items: Vec<Image>,
    pub(crate) last_selected: Option<usize>,
}

impl App {
    /// initialize the application state
    pub fn new(source_dir: String, target_dir: String) -> Self {
        Self {
            should_exit: false,
            progress: None,
            source_dir,
            target_dir,
            items: StatefulList {
                state: TableState::default(),
                items: vec![],
                last_selected: None,
            },
        }
    }

    /// draw the ui and handle events
    pub fn run(&mut self, mut terminal: Terminal<impl Backend>) -> Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| ui::draw(frame, self))?;

            events::handle_events(self)?;
        }
        Ok(())
    }

    /// Handle events like key presses
    pub(crate) fn handle_event(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('j') | KeyCode::Down => self.items.next(),
            KeyCode::Char('k') | KeyCode::Up => self.items.previous(),
            KeyCode::Char('s') => self.scan_source(),
            KeyCode::Char('p') => self.process_all(),
            KeyCode::Char('P') => self.process_selected(),
            _ => {}
        }
    }

    fn scan_source(&mut self) {
        // TODO: scan source directory
        if (self.progress.is_some()) {
            self.progress = None;
        } else {
            self.progress = Some(10f64);
            self.items.items = vec![
                ///
                Image {
                    source_path: "/tmp/images/".to_string(),
                    source_filename: "DSC_1234".to_string(),
                    source_filetype: "NEF".to_string(),
                    dates: Dates {
                        exif_date_time_original: None,
                        file_creation_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                        file_modified_date: todo!() },
                    target_path: "/tmp/images/sorted/2024-01-01".to_string(),
                    target_filename: "DSC_1234".to_string(),
                    target_filetype: "NEF".to_string(),
                    error: None,
                },
                Image {
                    source_path: "/tmp/images/".to_string(),
                    source_filename: "DSC_1235".to_string(),
                    source_filetype: "NEF".to_string(),
                    dates: Dates {
                        exif_date_time_original: None,
                        file_creation_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                        file_modified_date: todo!() },
                    target_path: "/tmp/images/sorted/2024-01-01".to_string(),
                    target_filename: "DSC_1235".to_string(),
                    target_filetype: "NEF".to_string(),
                    error: None,
                },
                Image {
                    source_path: "/tmp/images/".to_string(),
                    source_filename: "DSC_1236".to_string(),
                    source_filetype: "NEF".to_string(),
                    dates: Dates {
                        exif_date_time_original: None,
                        file_creation_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
                        file_modified_date: todo!() },
                    target_path: "/tmp/images/sorted/2024-01-02".to_string(),
                    target_filename: "DSC_1236".to_string(),
                    target_filetype: "NEF".to_string(),
                    error: None,
                },
            ]
        }
    }

    fn process_all(&mut self) {
        // TODO: process ALL
    }

    fn process_selected(&mut self) {
        if let Some(i) = self.items.state.selected() {
            // TODO: process selected only
        }
    }
}

impl StatefulList {
    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }

    fn unselect(&mut self) {
        let offset = self.state.offset();
        self.last_selected = self.state.selected();
        self.state.select(None);
        *self.state.offset_mut() = offset;
    }
}
