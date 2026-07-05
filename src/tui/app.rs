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
        if let Ok(entries) = sorter::dir::scan_dir(&PathBuf::from(self.source_dir.clone())) {
            self.items.items = entries
                .into_iter()
                .map(|entry| {
                    Image::new(entry.into_path(), PathBuf::from(self.target_dir.clone()))
                })
                .collect();
        }
    }

    fn process_all(&mut self) {
        let options = sorter::ProcessOptions::default();
        // NOTE: runs on the UI thread — the gauge only shows the final state.
        // Live updates need the processing moved to a background thread that
        // reports through a channel.
        let progress = &mut self.progress;
        let result = sorter::process(
            &PathBuf::from(self.source_dir.clone()),
            &PathBuf::from(self.target_dir.clone()),
            &options,
            |done, total| {
                *progress = Some(done as f64 / total.max(1) as f64 * 100.0);
            },
        );
        if result.is_ok() {
            self.items.items.clear();
        }
    }

    fn process_selected(&mut self) {
        if let Some(i) = self.items.state.selected() {
            if let Some(image) = self.items.items.get(i).cloned() {
                if let Ok((date, _)) = image.extract_date() {
                    let mut image = image;
                    if let Ok((target_dir, target_filename)) =
                        image.set_target(date, sorter::config::DEFAULT_PATTERN)
                    {
                        image.target_dir = target_dir;
                        image.target_filename = target_filename;
                        if image
                            .transfer_to_target(sorter::TransferMode::Copy, false)
                            .is_ok()
                        {
                            self.items.items.remove(i);
                        }
                    }
                }
            }
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
