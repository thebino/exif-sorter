use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::prelude::Terminal;
use ratatui::widgets::TableState;
use std::io::Result;
use std::path::PathBuf;

use crate::sorter;
use crate::sorter::image::Image;

use super::{events, ui};

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
        // let mut images = sorter::scan_dir(&PathBuf::from(self.source_dir.clone())).unwrap();

        // sorter::read_exif_and_metadata(&mut images).expect("Failed to read image from source");
        // self.items.items = images;
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
