use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::block::Title;
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Gauge, HighlightSpacing, Paragraph, Row, Table, Widget,
};
use ratatui::Frame;

use crate::sorter::{ItemOutcome, PlannedAction, TransferMode};
use crate::tui::app::{App, Screen, SetupFocus};

pub fn draw(frame: &mut Frame, app: &App) {
    let size: Rect = frame.size();

    let outer_layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(1), // status line: progress or run summary
        Constraint::Length(3),
    ]);
    let [source_area, target_area, content_area, status_area, actions_area] =
        outer_layout.areas(size);

    draw_dir_box(
        frame,
        source_area,
        "1",
        " Source directory",
        &app.source_dir,
        app.screen == Screen::Setup && app.focus == SetupFocus::Source,
    );
    draw_dir_box(
        frame,
        target_area,
        "2",
        " Target directory",
        &app.target_dir,
        app.screen == Screen::Setup && app.focus == SetupFocus::Target,
    );

    match app.screen {
        Screen::Setup => draw_setup_content(frame, content_area, app),
        Screen::Scanning => draw_placeholder(frame, content_area, "Scanning source directory…"),
        // The review table stays visible during execution so its Status
        // column can update live.
        Screen::Review | Screen::Executing => draw_review_table(frame, content_area, app),
    }

    draw_status(frame, status_area, app);
    draw_actions(frame, actions_area, app);
}

fn draw_dir_box(
    frame: &mut Frame,
    area: Rect,
    key: &str,
    label: &str,
    value: &str,
    focused: bool,
) {
    let title = Title::from(vec![
        Span::styled(format!("[{key}]"), Style::default().fg(Color::LightBlue)),
        label.into(),
    ]);
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let display = if focused {
        format!("{value}▏")
    } else {
        value.to_string()
    };
    Paragraph::new(display)
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .bold()
        .render(area, frame.buffer_mut());
}

fn draw_setup_content(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            "Mode: ".into(),
            mode_span(app.transfer_mode),
            "  (files are copied by default — the source stays untouched)".into(),
        ]),
        Line::from(""),
        Line::from("Press [s] to scan. Nothing is written before you confirm the plan."),
    ];
    if let Some(error) = &app.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Error: {error}"),
            Style::default().fg(Color::Red),
        )));
    }
    Paragraph::new(lines)
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .centered()
        .render(area, frame.buffer_mut());
}

fn mode_span(mode: TransferMode) -> Span<'static> {
    match mode {
        TransferMode::Copy => Span::styled("COPY", Style::default().fg(Color::Green).bold()),
        TransferMode::Move => Span::styled("MOVE", Style::default().fg(Color::Red).bold()),
    }
}

fn draw_placeholder(frame: &mut Frame, area: Rect, text: &str) {
    Paragraph::new(text)
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .centered()
        .render(area, frame.buffer_mut());
}

fn draw_review_table(frame: &mut Frame, area: Rect, app: &App) {
    let Some(plan) = &app.plan else {
        draw_placeholder(frame, area, "No plan — press [s] to scan.");
        return;
    };
    if plan.items.is_empty() {
        draw_placeholder(frame, area, "No supported media files found in the source.");
        return;
    }

    let header_style = Style::default().fg(Color::Green).bg(Color::DarkGray);
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Yellow);

    let header = ["Sel", "Source", "Date", "Via", "Planned target", "St"]
        .into_iter()
        .map(|h| Cell::from(Text::from(h)))
        .collect::<Row>()
        .style(header_style)
        .height(1);

    let source_prefix = plan.source.to_string_lossy().into_owned();
    let target_prefix = plan.target.to_string_lossy().into_owned();

    let rows = plan.items.iter().enumerate().map(|(i, item)| {
        let sel = if app.is_completed(i) {
            "[✓]" // processed — locked
        } else if item.selected {
            "[x]"
        } else {
            "[ ]"
        };
        // Always render via source_full(): Image.source_path alone is only
        // the parent directory.
        let source = relative_to(&item.image.source_full(), &source_prefix);
        let (date_cell, via_cell) = match &item.action {
            PlannedAction::Sorted { date, date_source } => {
                let color = if date_source.is_low_confidence() {
                    Color::Yellow
                } else {
                    Color::Green
                };
                (
                    Cell::from(Span::styled(date.to_string(), Style::default().fg(color))),
                    Cell::from(date_source.to_string()),
                )
            }
            PlannedAction::Unsorted { .. } => (
                Cell::from(Span::styled("unsorted", Style::default().fg(Color::Magenta))),
                Cell::from("no usable date"),
            ),
            PlannedAction::Corrupt { .. } => (
                Cell::from(Span::styled("corrupt", Style::default().fg(Color::Red))),
                Cell::from("unrecognized content"),
            ),
        };
        let target = relative_to(&item.planned_target.to_string_lossy(), &target_prefix);
        let status = status_symbol(app.outcomes.get(i).and_then(|o| o.as_ref()));

        Row::new(vec![
            Cell::from(sel),
            Cell::from(source),
            date_cell,
            via_cell,
            Cell::from(target),
            status,
        ])
    });

    let widths = [
        Constraint::Length(3),
        Constraint::Fill(10),
        Constraint::Length(10),
        Constraint::Length(22),
        Constraint::Fill(10),
        Constraint::Length(2),
    ];
    let table = Table::new(rows, widths)
        .block(
            Block::new().borders(Borders::ALL).title(format!(
                " {} files, {} selected — mode: {:?} ",
                plan.items.len(),
                app.selected_count(),
                app.transfer_mode
            )),
        )
        .header(header)
        .highlight_style(selected_style)
        .highlight_spacing(HighlightSpacing::Always);

    // TableState lives in App; clone for stateful rendering of the frame.
    let mut state = app.table.clone();
    frame.render_stateful_widget(table, area, &mut state);
}

fn status_symbol(outcome: Option<&ItemOutcome>) -> Cell<'static> {
    match outcome {
        None => Cell::from("·"),
        Some(ItemOutcome::Transferred { .. }) => {
            Cell::from(Span::styled("✓", Style::default().fg(Color::Green)))
        }
        Some(ItemOutcome::Duplicate) => Cell::from("≡"),
        Some(ItemOutcome::CollisionSkipped) => Cell::from("→"),
        Some(ItemOutcome::Unsorted) => {
            Cell::from(Span::styled("u", Style::default().fg(Color::Magenta)))
        }
        Some(ItemOutcome::Corrupt) => {
            Cell::from(Span::styled("c", Style::default().fg(Color::Red)))
        }
        Some(ItemOutcome::Failed { .. }) => {
            Cell::from(Span::styled("✗", Style::default().fg(Color::Red)))
        }
        Some(ItemOutcome::SkippedByUser) => Cell::from("-"),
    }
}

fn relative_to<'a>(path: &'a str, prefix: &str) -> String {
    path.strip_prefix(prefix)
        .map(|p| p.trim_start_matches('/').to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Single status line under the table: a live gauge while scanning or
/// executing, otherwise a one-line summary of the last run. Per-file detail
/// lives in the table's Status column, so this stays compact.
fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    if let Some((done, total)) = app.progress {
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
            .ratio(done as f64 / total.max(1) as f64)
            .label(Span::styled(
                format!("{done}/{total}"),
                Style::default().fg(Color::White),
            ))
            .render(area, frame.buffer_mut());
        return;
    }

    if let Some(error) = &app.error {
        Paragraph::new(Line::from(Span::styled(
            format!(" {error}"),
            Style::default().fg(Color::Red),
        )))
        .render(area, frame.buffer_mut());
        return;
    }

    if let Some(summary) = &app.summary {
        let verb = match app.transfer_mode {
            TransferMode::Copy => "Copied",
            TransferMode::Move => "Moved",
        };
        let mut spans = vec![Span::styled(
            format!(" {verb} {}/{}", summary.transferred, summary.total()),
            Style::default().fg(Color::Green),
        )];
        if summary.low_confidence > 0 {
            spans.push(Span::styled(
                format!(" · {} low-confidence", summary.low_confidence),
                Style::default().fg(Color::Yellow),
            ));
        }
        if summary.unsorted > 0 {
            spans.push(format!(" · {} unsorted", summary.unsorted).into());
        }
        if summary.corrupt > 0 {
            spans.push(Span::styled(
                format!(" · {} corrupt", summary.corrupt),
                Style::default().fg(Color::Red),
            ));
        }
        if summary.duplicates > 0 {
            spans.push(format!(" · {} duplicate", summary.duplicates).into());
        }
        if !summary.failed.is_empty() {
            spans.push(Span::styled(
                format!(" · {} failed", summary.failed.len()),
                Style::default().fg(Color::Red),
            ));
        }
        spans.push(Span::styled(
            "  — adjust selection and press ↵ to continue",
            Style::default().fg(Color::DarkGray),
        ));
        Paragraph::new(Line::from(spans)).render(area, frame.buffer_mut());
    }
}

fn draw_actions(frame: &mut Frame, area: Rect, app: &App) {
    let key = |k: &'static str| Span::styled(k, Style::default().fg(Color::LightBlue));
    let actions: Line = match app.screen {
        Screen::Setup => vec![
            key("1"),
            "/".into(),
            key("2"),
            " edit dirs ".into(),
            key("m"),
            " copy/move ".into(),
            key("s"),
            " scan ".into(),
            key("q"),
            " quit ".into(),
        ]
        .into(),
        Screen::Scanning => vec!["scanning… ".into(), key("q"), " quit ".into()].into(),
        Screen::Review => vec![
            key("␣"),
            " toggle ".into(),
            key("a"),
            " all/none ".into(),
            key("m"),
            " copy/move ".into(),
            key("↵"),
            " start ".into(),
            key("s"),
            " rescan ".into(),
            key("q"),
            " quit ".into(),
        ]
        .into(),
        Screen::Executing => vec!["processing — please wait…".into()].into(),
    };

    Paragraph::new(actions)
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .centered()
        .render(area, frame.buffer_mut());
}

