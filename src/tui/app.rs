use crossterm::event::{self, Event, KeyCode};
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
    BorderType, Clear, Gauge, ListState, Padding, Paragraph, StatefulWidget, Widget,
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

// #[derive(Default)]
pub struct App {
    items: StatefulList,
}

pub struct StatefulList {
    state: ListState,
    // items: Vec<ReleaseItem<'a>>,
    last_selected: Option<usize>,
    in_progress: Option<usize>,
}

impl<'a> App {
    pub fn new() -> Self {
        Self {
            items: StatefulList {
                state: ListState::default(),
                // items: releases.iter().map(ReleaseItem::from).collect(),
                last_selected: None,
                in_progress: None,
            },
        }
    }

    fn go_top(&mut self) {
        self.items.state.select(Some(0));
    }

    fn go_bottom(&mut self) {
        // self.items.state.select(Some(self.items.items.len() - 1));
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let outer_layout = Layout::vertical([Constraint::Percentage(90), Constraint::Fill(2)]);
        let [top_area, actions_area] = outer_layout.areas(area);

        let inner_layout =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]);
        let [releases_area, info_area] = inner_layout.areas(top_area);

        // self.render_releases(releases_area, buf);
        // self.render_info(info_area, buf);
        self.render_actions(actions_area, buf);

        // if self.items.in_progress.is_some() {
        // self.render_popup(top_area, buf);
        // }
        Paragraph::new("This is a sample".to_string())
            .block(Block::new().borders(Borders::ALL))
            .bold()
            .render(area, buf);
    }
}

impl App {
    pub async fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        loop {
            self.draw(&mut terminal)?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    use KeyCode::*;
                    match key.code {
                        Char('q') | Esc => return Ok(()),
                        // Char('h') | Left => self.items.unselect(),
                        // Char('j') | Down => self.items.next(),
                        // Char('k') | Up => self.items.previous(),
                        // Char('l') | Right | Enter => self.flip_status(),
                        Char('g') => self.go_top(),
                        Char('G') => self.go_bottom(),
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| f.render_widget(self, f.size()))?;
        Ok(())
    }

    fn render_actions(&mut self, area: Rect, buf: &mut Buffer) {
        // actions
        let actions: Line = vec![
            Span::styled("↓↑".to_string(), Style::default().fg(Color::LightBlue)),
            " to move ".into(),
            Span::styled("←".to_string(), Style::default().fg(Color::LightBlue)),
            " to unselect ".into(),
            Span::styled("→".to_string(), Style::default().fg(Color::LightBlue)),
            " to change status ".into(),
            Span::styled("g/G".to_string(), Style::default().fg(Color::LightBlue)),
            " to go to top/bottom ".into(),
            Span::styled("q".to_string(), Style::default().fg(Color::LightBlue)),
            " to quit ".into(),
        ]
        .into();

        Paragraph::new(actions)
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .centered()
            .render(area, buf);
    }
}

impl StatefulList {
    fn next(&mut self) {
        // let i = match self.state.selected() {
        //     Some(i) => {
        //         if i >= self.items.len() - 1 {
        //             0
        //         } else {
        //             i + 1
        //         }
        //     }
        //     None => self.last_selected.unwrap_or(0),
        // };
        // self.state.select(Some(i));
    }

    fn previous(&mut self) {
        // let i = match self.state.selected() {
        //     Some(i) => {
        //         if i == 0 {
        //             self.items.len() - 1
        //         } else {
        //             i - 1
        //         }
        //     }
        //     None => self.last_selected.unwrap_or(0),
        // };
        // self.state.select(Some(i));
    }

    fn unselect(&mut self) {
        let offset = self.state.offset();
        self.last_selected = self.state.selected();
        self.state.select(None);
        *self.state.offset_mut() = offset;
    }
}
