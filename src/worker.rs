//! Shared background-processing plumbing for the interactive frontends.
//!
//! Both the TUI and the GUI run planning and execution on a plain
//! `std::thread` and receive progress over a `std::sync::mpsc` channel,
//! keeping their render loops unblocked. `notify` fires after every event
//! send: the TUI passes a no-op (its 16 ms input poll doubles as a tick),
//! the GUI passes `ctx.request_repaint()` so frames are only drawn when
//! something changed.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

use crate::sorter::{self, ItemOutcome, Plan, ProcessOptions, ProcessSummary};

pub enum WorkerEvent {
    ScanProgress { done: usize, total: usize },
    PlanReady(Plan),
    PlanFailed(String),
    ItemDone { index: usize, outcome: ItemOutcome },
    Finished(ProcessSummary),
    ExecuteFailed(String),
}

/// Run `sorter::plan` on a background thread, streaming progress events.
pub fn spawn_plan(
    source: PathBuf,
    target: PathBuf,
    options: ProcessOptions,
    tx: Sender<WorkerEvent>,
    notify: impl Fn() + Send + Sync + 'static,
) {
    thread::spawn(move || {
        let progress_tx = tx.clone();
        let result = sorter::plan(&source, &target, &options, |done, total| {
            // A dropped receiver (UI closed) is not an error worth handling.
            let _ = progress_tx.send(WorkerEvent::ScanProgress { done, total });
            notify();
        });
        let event = match result {
            Ok(plan) => WorkerEvent::PlanReady(plan),
            Err(e) => WorkerEvent::PlanFailed(format!("{e:#}")),
        };
        let _ = tx.send(event);
        notify();
    });
}

/// Run `sorter::execute` on a background thread, streaming one event per
/// planned item plus a final summary.
pub fn spawn_execute(
    plan: Plan,
    options: ProcessOptions,
    tx: Sender<WorkerEvent>,
    notify: impl Fn() + Send + 'static,
) {
    thread::spawn(move || {
        let result = sorter::execute(plan, &options, |index, outcome| {
            let _ = tx.send(WorkerEvent::ItemDone {
                index,
                outcome: outcome.clone(),
            });
            notify();
        });
        let event = match result {
            Ok(summary) => WorkerEvent::Finished(summary),
            Err(e) => WorkerEvent::ExecuteFailed(format!("{e:#}")),
        };
        let _ = tx.send(event);
        notify();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Proves the Send/Sync bounds compile and events actually arrive: a
    /// Plan (holding Images) must be shippable across threads for the UIs
    /// to work at all.
    #[test]
    fn spawn_plan_delivers_plan_ready_over_channel() {
        // given
        let tmp = std::env::temp_dir().join(format!("exif-sorter-worker-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("a.jpg"), b"x").unwrap();
        let (tx, rx) = mpsc::channel();

        // when
        spawn_plan(
            tmp.clone(),
            tmp.join("sorted"),
            ProcessOptions::default(),
            tx,
            || {},
        );

        // then: PlanReady arrives (after any number of ScanProgress events)
        let deadline = std::time::Duration::from_secs(10);
        loop {
            match rx.recv_timeout(deadline).expect("worker sent nothing") {
                WorkerEvent::PlanReady(plan) => {
                    assert_eq!(plan.items.len(), 1);
                    break;
                }
                WorkerEvent::ScanProgress { .. } => continue,
                WorkerEvent::PlanFailed(e) => panic!("plan failed: {e}"),
                _ => panic!("unexpected event"),
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn spawn_plan_reports_failure_for_missing_source() {
        // given
        let (tx, rx) = mpsc::channel();

        // when
        spawn_plan(
            PathBuf::from("/this/does/not/exist"),
            PathBuf::from("/tmp/never"),
            ProcessOptions::default(),
            tx,
            || {},
        );

        // then
        match rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("worker sent nothing")
        {
            WorkerEvent::PlanFailed(reason) => {
                assert!(reason.contains("Invalid source directory"))
            }
            _ => panic!("expected PlanFailed"),
        }
    }
}
