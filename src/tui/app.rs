use std::io::Result;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::prelude::Terminal;
use ratatui::widgets::TableState;

use crate::sorter::{ItemOutcome, Plan, ProcessOptions, ProcessSummary, TransferMode};
use crate::worker::{self, WorkerEvent};

use super::{events, ui};

/// Scan → review → confirm flow. Nothing is written to disk before the
/// user confirms the plan on the Review screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Setup,
    Scanning,
    Review,
    Executing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetupFocus {
    None,
    Source,
    Target,
}

/// A terminal outcome means the file was handled (transferred, deduped, or
/// routed to unsorted/corrupt) — as opposed to still pending, skipped by the
/// user, or failed (retryable).
fn is_completed(outcome: Option<&ItemOutcome>) -> bool {
    matches!(
        outcome,
        Some(
            ItemOutcome::Transferred { .. }
                | ItemOutcome::Duplicate
                | ItemOutcome::Unsorted
                | ItemOutcome::Corrupt
        )
    )
}

/// Application state
pub struct App {
    should_exit: bool,
    pub(crate) screen: Screen,
    pub(crate) focus: SetupFocus,
    pub(crate) source_dir: String,
    pub(crate) target_dir: String,
    pub(crate) transfer_mode: TransferMode,
    pub(crate) error: Option<String>,
    /// Display copy of the plan; a clone is handed to the execute worker.
    pub(crate) plan: Option<Plan>,
    /// Index-aligned with `plan.items`; filled during execution.
    pub(crate) outcomes: Vec<Option<ItemOutcome>>,
    pub(crate) progress: Option<(usize, usize)>,
    pub(crate) summary: Option<ProcessSummary>,
    pub(crate) table: TableState,
    rx: Option<Receiver<WorkerEvent>>,
}

impl App {
    /// initialize the application state
    pub fn new(source_dir: String, target_dir: String) -> Self {
        Self {
            should_exit: false,
            screen: Screen::Setup,
            focus: SetupFocus::None,
            source_dir,
            target_dir,
            transfer_mode: TransferMode::Copy,
            error: None,
            plan: None,
            outcomes: Vec::new(),
            progress: None,
            summary: None,
            table: TableState::default(),
            rx: None,
        }
    }

    /// draw the ui and handle events
    pub fn run(&mut self, mut terminal: Terminal<impl Backend>) -> Result<()> {
        while !self.should_exit {
            self.drain_worker_events();
            terminal.draw(|frame| ui::draw(frame, self))?;
            events::handle_events(self)?;
        }
        Ok(())
    }

    /// Pull everything the background worker sent since the last frame.
    /// Collect first, apply second: applying an event may drop `self.rx`.
    fn drain_worker_events(&mut self) {
        let events: Vec<WorkerEvent> = match &self.rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        for event in events {
            self.apply_event(event);
        }
    }

    pub(crate) fn apply_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::ScanProgress { done, total } => {
                self.progress = Some((done, total));
            }
            WorkerEvent::PlanReady(plan) => {
                self.outcomes = vec![None; plan.items.len()];
                self.table = TableState::default();
                if !plan.items.is_empty() {
                    self.table.select(Some(0));
                }
                self.plan = Some(plan);
                self.progress = None;
                self.rx = None;
                self.screen = Screen::Review;
            }
            WorkerEvent::PlanFailed(reason) => {
                self.error = Some(reason);
                self.progress = None;
                self.rx = None;
                self.screen = Screen::Setup;
            }
            WorkerEvent::ItemDone { index, outcome } => {
                if let Some(slot) = self.outcomes.get_mut(index) {
                    // A follow-up run re-emits SkippedByUser for items that
                    // were already processed (and auto-deselected). Don't let
                    // that erase their real outcome in the status column.
                    let keep_previous = matches!(outcome, ItemOutcome::SkippedByUser)
                        && slot
                            .as_ref()
                            .is_some_and(|o| !matches!(o, ItemOutcome::SkippedByUser));
                    if !keep_previous {
                        *slot = Some(outcome);
                    }
                }
                let total = self.plan.as_ref().map(|p| p.items.len()).unwrap_or(0);
                self.progress = Some((index + 1, total));
            }
            // Stay on the review screen so the status column and the summary
            // line remain visible; the user can adjust the selection and run
            // again to process the rest.
            WorkerEvent::Finished(summary) => {
                self.deselect_completed();
                self.summary = Some(summary);
                self.error = None;
                self.progress = None;
                self.rx = None;
                self.screen = Screen::Review;
            }
            WorkerEvent::ExecuteFailed(reason) => {
                self.error = Some(reason);
                self.progress = None;
                self.rx = None;
                self.screen = Screen::Review;
            }
        }
    }

    /// Handle events like key presses
    pub(crate) fn handle_event(&mut self, event: KeyEvent) {
        match self.screen {
            Screen::Setup => self.handle_setup_key(event),
            // Workers run detached; quitting mid-scan is safe (planning is
            // read-only), quitting mid-execute is deliberately not offered.
            Screen::Scanning => {
                if matches!(event.code, KeyCode::Char('q') | KeyCode::Esc) {
                    self.should_exit = true;
                }
            }
            Screen::Executing => {}
            Screen::Review => self.handle_review_key(event),
        }
    }

    fn handle_setup_key(&mut self, event: KeyEvent) {
        match self.focus {
            SetupFocus::None => match event.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
                KeyCode::Char('1') => self.focus = SetupFocus::Source,
                KeyCode::Char('2') => self.focus = SetupFocus::Target,
                KeyCode::Char('m') => self.toggle_mode(),
                KeyCode::Char('s') | KeyCode::Enter => self.start_scan(),
                _ => {}
            },
            focused => {
                let field = match focused {
                    SetupFocus::Source => &mut self.source_dir,
                    _ => &mut self.target_dir,
                };
                match event.code {
                    KeyCode::Esc | KeyCode::Enter => self.focus = SetupFocus::None,
                    KeyCode::Backspace => {
                        field.pop();
                    }
                    KeyCode::Char(c) => field.push(c),
                    _ => {}
                }
            }
        }
    }

    fn handle_review_key(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char(' ') => self.toggle_current(),
            KeyCode::Char('a') => self.toggle_all(),
            KeyCode::Char('m') => self.toggle_mode(),
            KeyCode::Char('s') => self.start_scan(),
            KeyCode::Enter => self.start_execute(),
            _ => {}
        }
    }

    fn toggle_mode(&mut self) {
        self.transfer_mode = match self.transfer_mode {
            TransferMode::Copy => TransferMode::Move,
            TransferMode::Move => TransferMode::Copy,
        };
    }

    fn item_count(&self) -> usize {
        self.plan.as_ref().map(|p| p.items.len()).unwrap_or(0)
    }

    /// Wrapping table navigation. The empty check matters: the previous
    /// implementation computed `len - 1` unguarded, which underflows on an
    /// empty table and aborted the whole TUI on a single `j` keypress.
    pub(crate) fn select_next(&mut self) {
        let len = self.item_count();
        if len == 0 {
            return;
        }
        let i = match self.table.selected() {
            Some(i) if i + 1 >= len => 0,
            Some(i) => i + 1,
            None => 0,
        };
        self.table.select(Some(i));
    }

    pub(crate) fn select_previous(&mut self) {
        let len = self.item_count();
        if len == 0 {
            return;
        }
        let i = match self.table.selected() {
            Some(0) | None => len - 1,
            Some(i) => i - 1,
        };
        self.table.select(Some(i));
    }

    fn toggle_current(&mut self) {
        let Some(i) = self.table.selected() else {
            return;
        };
        if self.is_completed(i) {
            return; // already processed — locked
        }
        if let Some(plan) = self.plan.as_mut() {
            if let Some(item) = plan.items.get_mut(i) {
                item.selected = !item.selected;
            }
        }
    }

    /// After a run, drop the successfully-handled files out of the selection
    /// so a follow-up run does not touch them again (copying would duplicate,
    /// moving would fail on the now-missing source). Failures stay selected
    /// for a retry; deselected files remain available to select and continue.
    fn deselect_completed(&mut self) {
        let outcomes = &self.outcomes;
        if let Some(plan) = self.plan.as_mut() {
            for (item, outcome) in plan.items.iter_mut().zip(outcomes.iter()) {
                if is_completed(outcome.as_ref()) {
                    item.selected = false;
                }
            }
        }
    }

    /// A file that already reached a terminal outcome must not be selected
    /// again — re-processing would copy it twice (or fail on a moved source).
    pub(crate) fn is_completed(&self, index: usize) -> bool {
        is_completed(self.outcomes.get(index).and_then(|o| o.as_ref()))
    }

    fn toggle_all(&mut self) {
        let outcomes = &self.outcomes;
        if let Some(plan) = self.plan.as_mut() {
            // Only the not-yet-processed items participate; completed ones
            // stay locked out of the selection.
            let selectable: Vec<usize> = (0..plan.items.len())
                .filter(|&i| !is_completed(outcomes.get(i).and_then(|o| o.as_ref())))
                .collect();
            let all_on = selectable.iter().all(|&i| plan.items[i].selected);
            for &i in &selectable {
                plan.items[i].selected = !all_on;
            }
        }
    }

    pub(crate) fn selected_count(&self) -> usize {
        self.plan
            .as_ref()
            .map(|p| p.items.iter().filter(|i| i.selected).count())
            .unwrap_or(0)
    }

    fn options(&self) -> ProcessOptions {
        ProcessOptions {
            mode: self.transfer_mode,
            ..ProcessOptions::default()
        }
    }

    fn start_scan(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.error = None;
        self.plan = None;
        self.summary = None;
        self.outcomes.clear();
        self.progress = Some((0, 0));
        self.screen = Screen::Scanning;
        worker::spawn_plan(
            PathBuf::from(&self.source_dir),
            PathBuf::from(&self.target_dir),
            self.options(),
            tx,
            || {},
        );
    }

    fn start_execute(&mut self) {
        let Some(plan) = self.plan.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.error = None;
        // Keep existing outcomes: on a follow-up run the items processed
        // earlier stay marked done in the status column.
        self.progress = Some((0, plan.items.len()));
        self.screen = Screen::Executing;
        worker::spawn_execute(plan, self.options(), tx, || {});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sorter::{PlannedAction, PlannedItem};
    use crate::sorter::image::Image;

    fn app() -> App {
        App::new("/tmp/src".into(), "/tmp/dst".into())
    }

    fn plan_with(n: usize) -> Plan {
        let items = (0..n)
            .map(|i| PlannedItem {
                image: Image::new(
                    PathBuf::from(format!("/tmp/src/photo{i}.jpg")),
                    PathBuf::from("/tmp/dst"),
                ),
                action: PlannedAction::Unsorted {
                    reason: "test".into(),
                },
                planned_target: PathBuf::from(format!("/tmp/dst/unsorted/photo{i}.jpg")),
                selected: true,
            })
            .collect();
        Plan {
            source: PathBuf::from("/tmp/src"),
            target: PathBuf::from("/tmp/dst"),
            items,
        }
    }

    /// Regression: navigation on an empty table must not panic. The old
    /// StatefulList computed `items.len() - 1`, which underflows at len 0
    /// and aborted the entire TUI on a single keypress.
    #[test]
    fn navigation_on_empty_table_does_not_panic() {
        let mut app = app();
        app.select_next();
        app.select_previous();
    }

    #[test]
    fn plan_ready_switches_to_review_and_selects_first_row() {
        // given
        let mut app = app();
        app.screen = Screen::Scanning;

        // when
        app.apply_event(WorkerEvent::PlanReady(plan_with(2)));

        // then
        assert_eq!(app.screen, Screen::Review);
        assert_eq!(app.table.selected(), Some(0));
        assert_eq!(app.outcomes.len(), 2);
    }

    #[test]
    fn plan_failed_returns_to_setup_with_error() {
        let mut app = app();
        app.screen = Screen::Scanning;

        app.apply_event(WorkerEvent::PlanFailed("boom".into()));

        assert_eq!(app.screen, Screen::Setup);
        assert_eq!(app.error.as_deref(), Some("boom"));
    }

    #[test]
    fn finished_returns_to_review_with_summary() {
        let mut app = app();
        app.apply_event(WorkerEvent::PlanReady(plan_with(1)));
        app.screen = Screen::Executing;

        app.apply_event(WorkerEvent::Finished(ProcessSummary::default()));

        // Back on the review screen (not a separate summary screen), summary
        // available for the status line.
        assert_eq!(app.screen, Screen::Review);
        assert!(app.summary.is_some());
    }

    #[test]
    fn completed_items_are_deselected_for_a_follow_up_run() {
        // The continue-flow guarantee: files already transferred drop out of
        // the selection so pressing Enter again never re-processes them.
        let mut app = app();
        app.apply_event(WorkerEvent::PlanReady(plan_with(2)));
        assert_eq!(app.selected_count(), 2);

        app.apply_event(WorkerEvent::ItemDone {
            index: 0,
            outcome: ItemOutcome::Transferred {
                target: "t".into(),
                low_confidence: false,
            },
        });
        // item 1 gets no outcome (e.g. user had it deselected)
        app.apply_event(WorkerEvent::Finished(ProcessSummary::default()));

        assert_eq!(app.selected_count(), 1, "transferred item auto-deselected");
    }

    #[test]
    fn completed_items_cannot_be_reselected() {
        // Guards the recovery-critical property: once a file is copied, no
        // key (Space or the all/none toggle) can put it back in the selection
        // and cause a second copy.
        let mut app = app();
        app.apply_event(WorkerEvent::PlanReady(plan_with(2)));
        app.apply_event(WorkerEvent::ItemDone {
            index: 0,
            outcome: ItemOutcome::Transferred {
                target: "t".into(),
                low_confidence: false,
            },
        });
        app.apply_event(WorkerEvent::Finished(ProcessSummary::default()));
        // item 0 done (deselected), item 1 selected
        assert_eq!(app.selected_count(), 1);

        // Space on the completed row does nothing
        app.table.select(Some(0));
        app.handle_event(KeyEvent::from(KeyCode::Char(' ')));
        assert!(app.is_completed(0));
        assert_eq!(app.selected_count(), 1);

        // Toggling all off then on never re-includes the completed item: at
        // most the one not-done item is ever selected (never 2).
        app.handle_event(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.selected_count(), 0);
        app.handle_event(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.selected_count(), 1, "completed item stays out of the selection");
    }

    #[test]
    fn follow_up_skip_does_not_erase_a_prior_outcome() {
        // On the second run the completed item is re-emitted as
        // SkippedByUser; its "done" status must survive in the column.
        let mut app = app();
        app.apply_event(WorkerEvent::PlanReady(plan_with(1)));
        app.apply_event(WorkerEvent::ItemDone {
            index: 0,
            outcome: ItemOutcome::Transferred {
                target: "t".into(),
                low_confidence: false,
            },
        });
        app.apply_event(WorkerEvent::ItemDone {
            index: 0,
            outcome: ItemOutcome::SkippedByUser,
        });
        assert!(matches!(
            app.outcomes[0],
            Some(ItemOutcome::Transferred { .. })
        ));
    }

    #[test]
    fn space_toggles_selection_and_a_toggles_all() {
        // given
        let mut app = app();
        app.apply_event(WorkerEvent::PlanReady(plan_with(3)));

        // when: Space on the first row
        app.handle_event(KeyEvent::from(KeyCode::Char(' ')));
        // then
        assert_eq!(app.selected_count(), 2);

        // when: 'a' with mixed selection → select all, 'a' again → none
        app.handle_event(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.selected_count(), 3);
        app.handle_event(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn typing_edits_focused_directory_field() {
        // given
        let mut app = app();
        assert_eq!(app.screen, Screen::Setup);

        // when: focus source, type, backspace, unfocus
        app.handle_event(KeyEvent::from(KeyCode::Char('1')));
        app.handle_event(KeyEvent::from(KeyCode::Char('x')));
        app.handle_event(KeyEvent::from(KeyCode::Backspace));
        app.handle_event(KeyEvent::from(KeyCode::Char('y')));
        app.handle_event(KeyEvent::from(KeyCode::Esc));

        // then: text applied, Esc left edit mode instead of quitting
        assert_eq!(app.source_dir, "/tmp/srcy");
        assert_eq!(app.focus, SetupFocus::None);
        assert!(!app.should_exit);
    }

    #[test]
    fn mode_toggle_flips_between_copy_and_move() {
        let mut app = app();
        assert_eq!(app.transfer_mode, TransferMode::Copy);
        app.handle_event(KeyEvent::from(KeyCode::Char('m')));
        assert_eq!(app.transfer_mode, TransferMode::Move);
        app.handle_event(KeyEvent::from(KeyCode::Char('m')));
        assert_eq!(app.transfer_mode, TransferMode::Copy);
    }
}
