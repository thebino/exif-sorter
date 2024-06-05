use crate::tui::app::App;
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
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::block::Title;
use ratatui::widgets::{
    BorderType, Cell, Clear, Gauge, HighlightSpacing, ListState, Padding, Paragraph, Row,
    StatefulWidget, Table, Widget,
};
use ratatui::Frame;
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem},
};

use std::fs::File;
use std::io::{stdout, Result};
use std::net::Ipv4Addr;
use std::path::Path;
use std::{env, io};

pub fn draw(frame: &mut Frame, app: &App) {
    let size: Rect = frame.size();

    let outer_layout = Layout::vertical([
        Constraint::Min(3),
        Constraint::Min(3),
        Constraint::Fill(80),
        Constraint::Min(3),
    ]);
    let [source_area, target_area, content_area, actions_area] = outer_layout.areas(size);

    let content = vec![
        Span::styled("[1]".to_string(), Style::default().fg(Color::LightBlue)),
        " Source directory".into(),
    ];
    let title = Title::from(content);
    Paragraph::new(app.source_dir.clone())
        .block(Block::new().borders(Borders::ALL).title(title))
        .bold()
        .render(source_area, frame.buffer_mut());

    let content = vec![
        Span::styled("[2]".to_string(), Style::default().fg(Color::LightBlue)),
        " Target directory".into(),
    ];
    let title = Title::from(content);
    Paragraph::new(app.target_dir.clone())
        .block(Block::new().borders(Borders::ALL).title(title))
        .bold()
        .render(target_area, frame.buffer_mut());

    render_content(app, content_area, frame);
    render_actions(actions_area, frame.buffer_mut());

    if (app.progress.is_some()) {
        render_popup(app, content_area, frame);
    }
}

fn render_actions(area: Rect, buf: &mut Buffer) {
    // actions
    let actions: Line = vec![
        Span::styled("s".to_string(), Style::default().fg(Color::LightBlue)),
        " to scan source ".into(),
        Span::styled("p".to_string(), Style::default().fg(Color::LightBlue)),
        " to process ALL ".into(),
        Span::styled("P".to_string(), Style::default().fg(Color::LightBlue)),
        " to process SELECTED ".into(),
        Span::styled("↓↑".to_string(), Style::default().fg(Color::LightBlue)),
        " to move ".into(),
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

fn render_content(app: &App, area: Rect, frame: &mut Frame) {
    if app.items.items.is_empty() {
        Paragraph::new("Select a source and start a new scan first.")
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .centered()
            .render(area, frame.buffer_mut());
    } else {
        let header_style = Style::default().fg(Color::Green).bg(Color::DarkGray);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Yellow);

        let header = [
            Text::from("Source"),
            Text::from("Target"),
            Text::from("Moved").alignment(ratatui::layout::Alignment::Center),
        ]
        .into_iter()
        .map(Cell::from)
        .collect::<Row>()
        .style(header_style)
        .height(1);

        // TODO: fill content with real findings
        let rows = app.items.items.iter().enumerate().map(|(i, data)| {
            //     let color = match i % 2 {
            //         0 => Color::Magenta,
            //         _ => Color::LightMagenta,
            //     };
            //     // let item = data.ref_array();
            //     // item.into_iter()
            //     //     .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
            //     //     .collect::<Row>()
            //     //     .style(Style::new().fg(Color::White).bg(color))
            //     //     .height(4)

            Row::new(vec![
                Cell::from(data.source_path.to_string()),
                Cell::from(Line::from(vec![
                    "/tmp/images/sorted/".into(),
                    Span::styled("2024-01-01/".to_string(), Style::default().fg(Color::Green)),
                    "DSC_1234.NEF".into(),
                ])),
                Cell::from(Text::from("✓").alignment(ratatui::layout::Alignment::Center))
                    .style(Style::default()),
            ])
        });

        // let rows = [
        //     Row::new(vec![
        //         Cell::from("/tmp/images/DSC_1234.NEF"),
        //         Cell::from(Line::from(vec![
        //             "/tmp/images/sorted/".into(),
        //             Span::styled("2024-01-01/".to_string(), Style::default().fg(Color::Green)),
        //             "DSC_1234.NEF".into(),
        //         ])),
        //         Cell::from(Text::from("✓").alignment(ratatui::layout::Alignment::Center))
        //             .style(Style::default()),
        //     ]),
        //     Row::new(vec![
        //         Cell::from("/tmp/images/DSC_1235.NEF"),
        //         Cell::from(Line::from(vec![
        //             "/tmp/images/sorted/".into(),
        //             Span::styled("2024-01-01/".to_string(), Style::default().fg(Color::Green)),
        //             "DSC_1235.NEF".into(),
        //         ])),
        //         Cell::from(Text::from("×").alignment(ratatui::layout::Alignment::Center)),
        //     ]),
        //     Row::new(vec![
        //         Cell::from("/tmp/imagesq/DSC_1236.NEF"),
        //         Cell::from(Line::from(vec![
        //             "/tmp/images/sorted/".into(),
        //             Span::styled("2024-01-02/".to_string(), Style::default().fg(Color::Green)),
        //             "DSC_1236.NEF".into(),
        //         ])),
        //         Cell::from(Text::from("×").alignment(ratatui::layout::Alignment::Center)),
        //     ]),
        // ];
        let bar = " █ ";
        let widths = [
            Constraint::Fill(5),
            Constraint::Fill(10),
            Constraint::Min(5),
        ];
        let t = Table::new(rows, widths)
            .block(Block::new().borders(Borders::ALL))
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::from(vec![
                "".into(),
                bar.into(),
                bar.into(),
                "".into(),
            ]))
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_widget(t, area);
    }
}

fn render_popup(app: &App, area: Rect, frame: &mut Frame) {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - 20) / 2),
        Constraint::Percentage(20),
        Constraint::Percentage((100 - 20) / 2),
    ])
    .split(area);

    let popup_area = Layout::horizontal([
        Constraint::Percentage((100 - 60) / 2),
        Constraint::Percentage(60),
        Constraint::Percentage((100 - 60) / 2),
    ])
    .split(popup_layout[1])[1];

    Clear.render(popup_area, frame.buffer_mut());
    let title = Title::from("Progress").alignment(Alignment::Center);
    let label = Span::styled(
        format!("{:.1}/100", app.progress.unwrap()),
        Style::new().italic().bold().fg(Color::LightGreen),
    );

    let title = Block::default()
        .title(Title::from("Progress").alignment(Alignment::Center))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    // TODO: get the real progress?
    Gauge::default()
        .block(title)
        .gauge_style(Color::Green)
        .ratio(app.progress.unwrap() / 100.0)
        .label(label)
        .percent(10u16)
        .render(popup_area, frame.buffer_mut());
}
