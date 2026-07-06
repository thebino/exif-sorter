use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::sorter::{
    ItemOutcome, Plan, PlannedAction, ProcessOptions, ProcessSummary, TransferMode,
};
use crate::worker::{self, WorkerEvent};

/// Scan → review → confirm, same phases as the TUI. Nothing is written to
/// disk before the user confirms the plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Phase {
    Setup,
    Planning,
    Review,
    Executing,
}

pub(crate) struct SorterApp {
    pub(crate) phase: Phase,
    pub(crate) source_dir: String,
    pub(crate) target_dir: String,
    pub(crate) move_files: bool,
    pub(crate) error: Option<String>,
    /// Display copy of the plan; a clone is handed to the execute worker.
    pub(crate) plan: Option<Plan>,
    /// Index-aligned with `plan.items`; filled during execution.
    pub(crate) outcomes: Vec<Option<ItemOutcome>>,
    pub(crate) progress: Option<(usize, usize)>,
    pub(crate) summary: Option<ProcessSummary>,
    rx: Option<Receiver<WorkerEvent>>,
}

impl SorterApp {
    pub(crate) fn new(source_dir: String, target_dir: String) -> Self {
        Self {
            phase: Phase::Setup,
            source_dir,
            target_dir,
            move_files: false, // copy is the safe default for recovered media
            error: None,
            plan: None,
            outcomes: Vec::new(),
            progress: None,
            summary: None,
            rx: None,
        }
    }

    pub(crate) fn apply_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::ScanProgress { done, total } => {
                self.progress = Some((done, total));
            }
            WorkerEvent::PlanReady(plan) => {
                self.outcomes = vec![None; plan.items.len()];
                self.plan = Some(plan);
                self.progress = None;
                self.rx = None;
                self.phase = Phase::Review;
            }
            WorkerEvent::PlanFailed(reason) => {
                self.error = Some(reason);
                self.progress = None;
                self.rx = None;
                self.phase = Phase::Setup;
            }
            WorkerEvent::ItemDone { index, outcome } => {
                if let Some(slot) = self.outcomes.get_mut(index) {
                    // A follow-up run re-emits SkippedByUser for items already
                    // processed (and auto-deselected); don't erase their real
                    // outcome in the status column.
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
            // Return to the review table (not a separate screen) so the
            // status column and the summary bar stay visible and the user
            // can adjust the selection and run again.
            WorkerEvent::Finished(summary) => {
                self.deselect_completed();
                self.summary = Some(summary);
                self.error = None;
                self.progress = None;
                self.rx = None;
                self.phase = Phase::Review;
            }
            WorkerEvent::ExecuteFailed(reason) => {
                self.error = Some(reason);
                self.progress = None;
                self.rx = None;
                self.phase = Phase::Review;
            }
        }
    }

    /// After a run, drop successfully-handled files from the selection so a
    /// follow-up run does not re-process them (copy would duplicate, move
    /// would fail on the missing source). Failures stay selected for retry.
    fn deselect_completed(&mut self) {
        let outcomes = &self.outcomes;
        if let Some(plan) = self.plan.as_mut() {
            for (item, outcome) in plan.items.iter_mut().zip(outcomes.iter()) {
                if matches!(
                    outcome,
                    Some(
                        ItemOutcome::Transferred { .. }
                            | ItemOutcome::Duplicate
                            | ItemOutcome::Unsorted
                            | ItemOutcome::Corrupt
                    )
                ) {
                    item.selected = false;
                }
            }
        }
    }

    pub(crate) fn can_scan(&self) -> bool {
        !self.source_dir.is_empty() && PathBuf::from(&self.source_dir).is_dir()
    }

    pub(crate) fn selected_count(&self) -> usize {
        self.plan
            .as_ref()
            .map(|p| p.items.iter().filter(|i| i.selected).count())
            .unwrap_or(0)
    }

    fn options(&self) -> ProcessOptions {
        ProcessOptions {
            mode: if self.move_files {
                TransferMode::Move
            } else {
                TransferMode::Copy
            },
            ..ProcessOptions::default()
        }
    }

    fn start_scan(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.error = None;
        self.plan = None;
        self.summary = None;
        self.outcomes.clear();
        self.progress = Some((0, 0));
        self.phase = Phase::Planning;
        let repaint = ctx.clone();
        worker::spawn_plan(
            PathBuf::from(&self.source_dir),
            PathBuf::from(&self.target_dir),
            self.options(),
            tx,
            move || repaint.request_repaint(),
        );
    }

    fn start_execute(&mut self, ctx: &egui::Context) {
        let Some(plan) = self.plan.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.error = None;
        // Keep existing outcomes: on a follow-up run the items processed
        // earlier stay marked done in the status column.
        self.progress = Some((0, plan.items.len()));
        self.phase = Phase::Executing;
        let repaint = ctx.clone();
        worker::spawn_execute(plan, self.options(), tx, move || repaint.request_repaint());
    }

    fn reset(&mut self) {
        self.phase = Phase::Setup;
        self.error = None;
        self.plan = None;
        self.summary = None;
        self.outcomes.clear();
        self.progress = None;
    }
}

impl eframe::App for SorterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Collect first, apply second: applying Finished/PlanReady drops
        // self.rx, which would fight the borrow of the drain iterator.
        let events: Vec<WorkerEvent> = self
            .rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            self.apply_event(event);
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        // Review controls live in their own bottom panel — a hard region
        // like the top menu bar, so the table's column-resize handles (which
        // span the central panel) can never extend across them.
        if self.phase == Phase::Review {
            egui::TopBottomPanel::bottom("review_controls").show(ctx, |ui| {
                self.ui_review_controls(ui, ctx);
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.phase {
            Phase::Setup => self.ui_setup(ui, ctx),
            Phase::Planning => self.ui_progress(ui, "Scanning source directory…"),
            Phase::Review => self.ui_review(ui),
            Phase::Executing => {
                self.ui_progress(ui, "Processing…");
                ui.separator();
                self.ui_table(ui, false);
            }
        });
    }
}

impl SorterApp {
    fn ui_setup(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("exif-sorter");
        ui.add_space(8.0);

        egui::Grid::new("dirs").num_columns(3).show(ui, |ui| {
            ui.label("Source:");
            ui.add(
                egui::TextEdit::singleline(&mut self.source_dir).desired_width(420.0),
            );
            if ui.button("Browse…").clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.source_dir = dir.to_string_lossy().into_owned();
                }
            }
            ui.end_row();

            ui.label("Target:");
            ui.add(
                egui::TextEdit::singleline(&mut self.target_dir).desired_width(420.0),
            );
            if ui.button("Browse…").clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.target_dir = dir.to_string_lossy().into_owned();
                }
            }
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.checkbox(
            &mut self.move_files,
            "Move files instead of copying (the source is removed)",
        );
        ui.add_space(12.0);

        let scan = ui.add_enabled(self.can_scan(), egui::Button::new("Scan"));
        if !self.can_scan() {
            ui.label(
                egui::RichText::new("Select an existing source directory to scan.").weak(),
            );
        }
        if scan.clicked() {
            self.start_scan(ctx);
        }

        if let Some(error) = &self.error {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::RED, format!("Error: {error}"));
        }

        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(
                "Scanning only reads files — nothing is written before you confirm the plan.",
            )
            .weak(),
        );
    }

    fn ui_progress(&mut self, ui: &mut egui::Ui, label: &str) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(label);
        });
        if let Some((done, total)) = self.progress {
            let fraction = done as f32 / total.max(1) as f32;
            ui.add(
                egui::ProgressBar::new(fraction)
                    .text(format!("{done}/{total}"))
                    .animate(true),
            );
        }
    }

    fn ui_review(&mut self, ui: &mut egui::Ui) {
        let (total, low_confidence, unsorted, corrupt) = match &self.plan {
            Some(plan) => {
                let mut low = 0;
                let mut uns = 0;
                let mut cor = 0;
                for item in &plan.items {
                    match &item.action {
                        PlannedAction::Sorted { date_source, .. } => {
                            if date_source.is_low_confidence() {
                                low += 1;
                            }
                        }
                        PlannedAction::Unsorted { .. } => uns += 1,
                        PlannedAction::Corrupt { .. } => cor += 1,
                    }
                }
                (plan.items.len(), low, uns, cor)
            }
            None => (0, 0, 0, 0),
        };

        ui.horizontal(|ui| {
            ui.heading(format!("{total} files found"));
            if low_confidence > 0 {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("{low_confidence} low-confidence"),
                );
            }
            if unsorted > 0 {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("{unsorted} unsorted"));
            }
            if corrupt > 0 {
                ui.colored_label(egui::Color32::RED, format!("{corrupt} corrupt"));
            }
        });

        ui.horizontal(|ui| {
            // Completed items stay locked: selecting them again would copy
            // twice (or fail on a moved source).
            let outcomes = &self.outcomes;
            if ui.button("Select all").clicked() {
                if let Some(plan) = self.plan.as_mut() {
                    for (i, item) in plan.items.iter_mut().enumerate() {
                        if !is_completed(outcomes.get(i).and_then(|o| o.as_ref())) {
                            item.selected = true;
                        }
                    }
                }
            }
            if ui.button("Select none").clicked() {
                if let Some(plan) = self.plan.as_mut() {
                    for item in plan.items.iter_mut() {
                        item.selected = false;
                    }
                }
            }
            ui.checkbox(
                &mut self.move_files,
                "Move instead of copy (source is removed)",
            );
        });
        ui.separator();

        self.ui_table(ui, true);
    }

    /// Bottom-panel controls for the review phase: the last run's summary
    /// plus the confirm / new-scan buttons. Lives in its own `TopBottomPanel`
    /// so the table above cannot draw its resize handles over it.
    fn ui_review_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(4.0);
        self.ui_summary_line(ui);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("Error: {error}"));
        }
        let n = self.selected_count();
        let move_files = self.move_files;
        ui.horizontal(|ui| {
            let verb = if move_files { "Move" } else { "Copy" };
            let button = egui::Button::new(format!("{verb} {n} files"));
            if ui.add_enabled(n > 0, button).clicked() {
                self.start_execute(ctx);
            }
            if ui.button("New scan").clicked() {
                self.reset();
            }
        });
        ui.add_space(4.0);
    }

    /// Compact one-line summary of the last run; per-file detail is in the
    /// table's Status column, and full failure reasons in the tooltips there.
    fn ui_summary_line(&self, ui: &mut egui::Ui) {
        let Some(summary) = &self.summary else {
            return;
        };
        let verb = if self.move_files { "Moved" } else { "Copied" };
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{verb} {} of {}.", summary.transferred, summary.total()));
            if summary.low_confidence > 0 {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("{} low-confidence", summary.low_confidence),
                );
            }
            if summary.unsorted > 0 {
                ui.label(format!("{} unsorted", summary.unsorted));
            }
            if summary.corrupt > 0 {
                ui.colored_label(egui::Color32::RED, format!("{} corrupt", summary.corrupt));
            }
            if summary.duplicates > 0 {
                ui.label(format!("{} duplicate", summary.duplicates));
            }
            if !summary.failed.is_empty() {
                ui.colored_label(
                    egui::Color32::RED,
                    format!("{} failed", summary.failed.len()),
                );
            }
            ui.weak("— adjust selection and run again to continue");
        });
    }

    fn ui_table(&mut self, ui: &mut egui::Ui, editable: bool) {
        let Some(plan) = self.plan.as_mut() else {
            return;
        };
        let source_prefix = plan.source.to_string_lossy().into_owned();
        let target_prefix = plan.target.to_string_lossy().into_owned();

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true) // user-adjustable column widths (drag the separators)
            // shrink vertically to the rows — otherwise the resize
            // separators are drawn across the empty space below the table
            .auto_shrink([false, true])
            .column(Column::exact(24.0)) // selected
            .column(Column::initial(220.0).at_least(120.0).clip(true)) // source
            .column(Column::initial(90.0).at_least(80.0)) // date
            // wide enough for "EXIF DateTimeDigitized" on one line
            .column(Column::initial(185.0).at_least(100.0).clip(true)) // via
            .column(Column::remainder().at_least(120.0).clip(true)) // target
            .column(Column::initial(110.0).at_least(60.0).clip(true)) // status
            .header(20.0, |mut header| {
                for title in ["", "Source", "Date", "Via", "Planned target", "Status"] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                body.rows(18.0, plan.items.len(), |mut row| {
                    let index = row.index();
                    let item = &mut plan.items[index];
                    let done = is_completed(self.outcomes.get(index).and_then(|o| o.as_ref()));
                    row.col(|ui| {
                        if done {
                            ui.label("✓"); // processed — locked
                        } else if editable {
                            ui.checkbox(&mut item.selected, "");
                        } else {
                            ui.label(if item.selected { "☑" } else { "☐" });
                        }
                    });
                    row.col(|ui| {
                        // Always source_full(): source_path alone is the
                        // parent directory.
                        truncated_label(
                            ui,
                            relative_to(&item.image.source_full(), &source_prefix),
                        );
                    });
                    let (date_text, date_color, via) = match &item.action {
                        PlannedAction::Sorted { date, date_source } => (
                            date.to_string(),
                            if date_source.is_low_confidence() {
                                egui::Color32::YELLOW
                            } else {
                                egui::Color32::GREEN
                            },
                            date_source.to_string(),
                        ),
                        PlannedAction::Unsorted { .. } => (
                            "unsorted".to_string(),
                            egui::Color32::LIGHT_RED,
                            "no usable date".to_string(),
                        ),
                        PlannedAction::Corrupt { .. } => (
                            "corrupt".to_string(),
                            egui::Color32::RED,
                            "unrecognized content".to_string(),
                        ),
                    };
                    row.col(|ui| {
                        truncated_label(ui, egui::RichText::new(date_text).color(date_color));
                    });
                    row.col(|ui| {
                        truncated_label(ui, via);
                    });
                    row.col(|ui| {
                        truncated_label(
                            ui,
                            relative_to(&item.planned_target.to_string_lossy(), &target_prefix),
                        );
                    });
                    let status = match self.outcomes.get(index).and_then(|o| o.as_ref()) {
                        None => "· pending".to_string(),
                        Some(ItemOutcome::Transferred { .. }) => "✓ done".to_string(),
                        Some(ItemOutcome::Duplicate) => "≡ duplicate".to_string(),
                        Some(ItemOutcome::CollisionSkipped) => "→ skipped".to_string(),
                        Some(ItemOutcome::Unsorted) => "unsorted/".to_string(),
                        Some(ItemOutcome::Corrupt) => "corrupt/".to_string(),
                        Some(ItemOutcome::Failed { reason }) => format!("✗ {reason}"),
                        Some(ItemOutcome::SkippedByUser) => "deselected".to_string(),
                    };
                    row.col(|ui| {
                        truncated_label(ui, status);
                    });
                });
            });
    }

}

/// One-line cell text: truncate with … instead of wrapping into taller rows.
fn truncated_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.add(egui::Label::new(text).truncate());
}

/// A terminal outcome means the file was handled (transferred, deduped, or
/// routed to unsorted/corrupt) — as opposed to still pending, skipped by the
/// user, or failed (retryable). Such items are locked out of re-selection.
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

fn relative_to(path: &str, prefix: &str) -> String {
    path.strip_prefix(prefix)
        .map(|p| p.trim_start_matches('/').to_string())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sorter::image::Image;
    use crate::sorter::PlannedItem;

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

    #[test]
    fn plan_ready_switches_to_review() {
        let mut app = SorterApp::new("/tmp/src".into(), "/tmp/dst".into());
        app.phase = Phase::Planning;

        app.apply_event(WorkerEvent::PlanReady(plan_with(2)));

        assert_eq!(app.phase, Phase::Review);
        assert_eq!(app.selected_count(), 2);
        assert_eq!(app.outcomes.len(), 2);
    }

    #[test]
    fn plan_failed_returns_to_setup_with_error() {
        let mut app = SorterApp::new("/tmp/src".into(), "/tmp/dst".into());
        app.phase = Phase::Planning;

        app.apply_event(WorkerEvent::PlanFailed("boom".into()));

        assert_eq!(app.phase, Phase::Setup);
        assert_eq!(app.error.as_deref(), Some("boom"));
    }

    #[test]
    fn finished_returns_to_review_with_summary() {
        let mut app = SorterApp::new("/tmp/src".into(), "/tmp/dst".into());
        app.apply_event(WorkerEvent::PlanReady(plan_with(1)));
        app.phase = Phase::Executing;

        app.apply_event(WorkerEvent::Finished(ProcessSummary::default()));

        // Back on the review screen (not a separate done screen), summary
        // available for the status bar.
        assert_eq!(app.phase, Phase::Review);
        assert!(app.summary.is_some());
    }

    #[test]
    fn completed_items_are_deselected_for_a_follow_up_run() {
        // Files already transferred drop out of the selection so the confirm
        // button never re-processes them.
        let mut app = SorterApp::new("/tmp/src".into(), "/tmp/dst".into());
        app.apply_event(WorkerEvent::PlanReady(plan_with(2)));
        assert_eq!(app.selected_count(), 2);

        app.apply_event(WorkerEvent::ItemDone {
            index: 0,
            outcome: ItemOutcome::Transferred {
                target: "t".into(),
                low_confidence: false,
            },
        });
        app.apply_event(WorkerEvent::Finished(ProcessSummary::default()));

        assert_eq!(app.selected_count(), 1);
    }

    #[test]
    fn can_scan_requires_existing_source_directory() {
        // Prevents launching a scan that would immediately fail: the Scan
        // button stays disabled until the source exists.
        let mut app = SorterApp::new("/definitely/not/a/dir".into(), "/tmp/dst".into());
        assert!(!app.can_scan());
        app.source_dir = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(app.can_scan());
    }
}
