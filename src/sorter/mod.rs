pub mod config;
pub mod dates;
pub mod dir;
pub mod filename_date;
pub mod image;
pub mod manifest;
pub mod video;

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::bail;
use chrono::NaiveDate;
use rand::Rng as _;
use rayon::prelude::*;
use tracing::{debug, warn};

use crate::error::AppError;
use dir::scan_dir;
use image::{DateSource, Image};
use manifest::ManifestWriter;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransferMode {
    /// Leave the source untouched — the safe default for recovered media.
    Copy,
    /// Remove the source after a successful transfer.
    Move,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CollisionPolicy {
    /// Append a random suffix and store both files.
    #[default]
    Suffix,
    /// Leave the colliding source file where it is.
    Skip,
    /// Byte-compare: identical content is recorded as a duplicate and not
    /// stored again; different content gets a suffix.
    Dedupe,
}

#[derive(Clone)]
pub struct ProcessOptions {
    /// Log what would happen without touching any file.
    pub dry_run: bool,
    pub mode: TransferMode,
    pub collision: CollisionPolicy,
    /// Folder layout below the target (see `config::render_pattern`).
    pub pattern: String,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            mode: TransferMode::Copy,
            collision: CollisionPolicy::Suffix,
            pattern: config::DEFAULT_PATTERN.to_string(),
        }
    }
}

/// Routing decision for a single file, made during planning, in order of
/// trust:
/// - a metadata-derived date (EXIF/GPS/video/filename) implies valid
///   content → sorted
/// - otherwise the content signature decides: unrecognizable bytes are
///   carved garbage whose file dates mean nothing → corrupt/
/// - recognizable content without any date → unsorted/
#[derive(Clone, Debug)]
pub enum PlannedAction {
    Sorted {
        date: NaiveDate,
        date_source: DateSource,
    },
    Unsorted {
        reason: String,
    },
    Corrupt {
        reason: String,
    },
}

/// One file with its routing decision and the plain (unsuffixed) target
/// path for display. Collision handling happens at execute time.
#[derive(Clone)]
pub struct PlannedItem {
    pub image: Image,
    pub action: PlannedAction,
    pub planned_target: PathBuf,
    /// Frontends toggle this in the review step; deselected items are
    /// skipped entirely by `execute`.
    pub selected: bool,
}

/// Result of the read-only planning phase.
#[derive(Clone)]
pub struct Plan {
    pub source: PathBuf,
    pub target: PathBuf,
    pub items: Vec<PlannedItem>,
}

/// What `execute` did with a single planned item.
#[derive(Clone, Debug)]
pub enum ItemOutcome {
    Transferred { target: String, low_confidence: bool },
    Duplicate,
    CollisionSkipped,
    Unsorted,
    Corrupt,
    Failed { reason: String },
    SkippedByUser,
}

/// Outcome of a sort run. `failed` carries (path, reason) pairs so frontends
/// can show the user exactly which files need manual attention.
#[derive(Default, Debug)]
pub struct ProcessSummary {
    /// Files sorted into a date folder.
    pub transferred: usize,
    /// Of the transferred files, how many were dated from file timestamps
    /// only (low confidence — unreliable on recovered media).
    pub low_confidence: usize,
    /// Exact duplicates of already-stored files (CollisionPolicy::Dedupe).
    pub duplicates: usize,
    /// Collisions left in place (CollisionPolicy::Skip).
    pub collisions_skipped: usize,
    /// No usable date; transferred to `{target}/unsorted/`.
    pub unsorted: usize,
    /// Content not recognized as any known file type; transferred to
    /// `{target}/corrupt/`.
    pub corrupt: usize,
    /// Transfer failed; file left at the source.
    pub failed: Vec<(String, String)>,
}

impl ProcessSummary {
    pub fn total(&self) -> usize {
        self.transferred
            + self.duplicates
            + self.collisions_skipped
            + self.unsorted
            + self.corrupt
            + self.failed.len()
    }
}

/// Read-only planning phase: scan `source`, date every file (in parallel via
/// rayon — EXIF parsing dominates the runtime) and decide where each one
/// would go. **Writes nothing** — the target directory is not even created.
/// This is the safety property review UIs rely on: nothing is touched until
/// the user confirms the plan with `execute`.
///
/// `on_progress` is `Fn + Sync` (unlike `process`'s `FnMut`) because it is
/// called from inside the parallel iterator.
pub fn plan(
    source: &Path,
    target: &Path,
    options: &ProcessOptions,
    on_progress: impl Fn(usize, usize) + Sync,
) -> anyhow::Result<Plan> {
    if !source.exists() {
        bail!(AppError::InvalidSource {
            expected: source.to_string_lossy().into_owned()
        });
    }

    let mut entries = scan_dir(source)?;

    // Never re-sort files already inside the target tree — with the default
    // arguments the target directory lives inside the source directory.
    // A target that does not exist yet trivially contains no entries (and
    // must not be created here: planning is read-only).
    if let Ok(target_canon) = target.canonicalize() {
        entries.retain(|entry| {
            entry
                .path()
                .canonicalize()
                .map(|p| !p.starts_with(&target_canon))
                .unwrap_or(true)
        });
    }

    let total = entries.len();
    let counter = AtomicUsize::new(0);

    let items: Vec<PlannedItem> = entries
        .into_par_iter()
        .map(|entry| {
            let image = Image::new(entry.into_path(), target.to_path_buf());
            let action = decide_action(&image);
            let planned_target = match &action {
                PlannedAction::Sorted { date, .. } => image
                    .target_dir_for(*date, &options.pattern)
                    .join(image.target_filename()),
                PlannedAction::Unsorted { .. } => {
                    target.join("unsorted").join(image.target_filename())
                }
                PlannedAction::Corrupt { .. } => {
                    target.join("corrupt").join(image.target_filename())
                }
            };
            let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
            on_progress(done, total);
            PlannedItem {
                image,
                action,
                planned_target,
                selected: true,
            }
        })
        .collect();

    Ok(Plan {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        items,
    })
}

fn decide_action(image: &Image) -> PlannedAction {
    match image.extract_date() {
        Ok((date, date_source)) if !date_source.is_low_confidence() => {
            debug!(
                "File '{}' has date '{date}' from {date_source}",
                image.source_full()
            );
            PlannedAction::Sorted { date, date_source }
        }
        Ok((date, date_source)) => {
            if content_recognized(Path::new(&image.source_full())) {
                warn!(
                    "File '{}': no exif date, using {date_source} '{date}' (unreliable on recovered media)",
                    image.source_full()
                );
                PlannedAction::Sorted { date, date_source }
            } else {
                PlannedAction::Corrupt {
                    reason: "content not recognized as any known file type".to_string(),
                }
            }
        }
        Err(e) => {
            if content_recognized(Path::new(&image.source_full())) {
                PlannedAction::Unsorted {
                    reason: format!("{e:#}"),
                }
            } else {
                PlannedAction::Corrupt {
                    reason: format!("no usable date and content not recognized ({e:#})"),
                }
            }
        }
    }
}

/// Execute a plan: transfer every selected item, honoring the collision
/// policy at execute time (files may have appeared between plan and
/// execute), and append every decision to the manifest. `on_item` fires
/// once per planned item, in order, with its outcome.
pub fn execute(
    plan: Plan,
    options: &ProcessOptions,
    mut on_item: impl FnMut(usize, &ItemOutcome),
) -> anyhow::Result<ProcessSummary> {
    let Plan { target, items, .. } = plan;
    fs::create_dir_all(&target)?;

    let mut manifest = ManifestWriter::new(&target, options.dry_run);
    let action_str = match options.mode {
        TransferMode::Copy => "copied",
        TransferMode::Move => "moved",
    };

    let mut summary = ProcessSummary::default();
    for (index, item) in items.into_iter().enumerate() {
        let outcome = if item.selected {
            execute_item(item, &target, options, action_str, &mut manifest, &mut summary)
        } else {
            ItemOutcome::SkippedByUser
        };
        on_item(index, &outcome);
    }

    Ok(summary)
}

fn execute_item(
    item: PlannedItem,
    target: &Path,
    options: &ProcessOptions,
    action_str: &str,
    manifest: &mut ManifestWriter,
    summary: &mut ProcessSummary,
) -> ItemOutcome {
    let mut image = item.image;
    match item.action {
        PlannedAction::Sorted { date, date_source } => {
            // Collision handling on the plain (unsuffixed) target path.
            let plain_path = &item.planned_target;
            match options.collision {
                CollisionPolicy::Skip if plain_path.exists() => {
                    summary.collisions_skipped += 1;
                    manifest.record(
                        "collision_skipped",
                        "sorted",
                        &image.source_full(),
                        &plain_path.to_string_lossy(),
                        &date.to_string(),
                        &date_source.to_string(),
                        "target exists, --on-collision skip",
                    );
                    return ItemOutcome::CollisionSkipped;
                }
                CollisionPolicy::Dedupe if plain_path.exists() => {
                    match files_identical(Path::new(&image.source_full()), plain_path) {
                        Ok(true) => {
                            summary.duplicates += 1;
                            manifest.record(
                                "duplicate",
                                "sorted",
                                &image.source_full(),
                                &plain_path.to_string_lossy(),
                                &date.to_string(),
                                &date_source.to_string(),
                                "identical content already stored",
                            );
                            return ItemOutcome::Duplicate;
                        }
                        Ok(false) => {} // different content → suffix below
                        Err(e) => {
                            let reason = format!("dedupe compare failed: {e}");
                            summary.failed.push((image.source_full(), reason.clone()));
                            return ItemOutcome::Failed { reason };
                        }
                    }
                }
                _ => {}
            }

            match image.set_target(date, &options.pattern) {
                Ok((target_dir, target_filename)) => {
                    image.target_dir = target_dir;
                    image.target_filename = target_filename;
                    let source_str = image.source_full();
                    let target_str = image.target_full();
                    match image.transfer_to_target(options.mode, options.dry_run) {
                        Ok(()) => {
                            summary.transferred += 1;
                            let low_confidence = date_source.is_low_confidence();
                            if low_confidence {
                                summary.low_confidence += 1;
                            }
                            manifest.record(
                                action_str,
                                "sorted",
                                &source_str,
                                &target_str,
                                &date.to_string(),
                                &date_source.to_string(),
                                "",
                            );
                            ItemOutcome::Transferred {
                                target: target_str,
                                low_confidence,
                            }
                        }
                        Err(e) => {
                            let reason = format!("{e:#}");
                            warn!("Failed to transfer '{source_str}': {reason}");
                            summary.failed.push((source_str.clone(), reason.clone()));
                            manifest.record(
                                "failed",
                                "sorted",
                                &source_str,
                                &target_str,
                                &date.to_string(),
                                &date_source.to_string(),
                                &reason,
                            );
                            ItemOutcome::Failed { reason }
                        }
                    }
                }
                Err(e) => {
                    let reason = format!("{e:#}");
                    warn!(
                        "Failed to resolve target for '{}': {reason}",
                        image.source_full()
                    );
                    summary.failed.push((image.source_full(), reason.clone()));
                    ItemOutcome::Failed { reason }
                }
            }
        }
        action @ (PlannedAction::Unsorted { .. } | PlannedAction::Corrupt { .. }) => {
            let (category, reason) = match action {
                PlannedAction::Unsorted { reason } => ("unsorted", reason),
                PlannedAction::Corrupt { reason } => ("corrupt", reason),
                PlannedAction::Sorted { .. } => unreachable!(),
            };
            let category_dir = target.join(category);
            image.target_dir = category_dir.clone();
            image.target_filename =
                unique_name_in(&category_dir, &image.target_filename, &image.target_filetype);

            warn!(
                "File '{}': {reason} — {action_str} to {category}/",
                image.source_full()
            );
            let source_str = image.source_full();
            let target_str = image.target_full();
            match image.transfer_to_target(options.mode, options.dry_run) {
                Ok(()) => {
                    if category == "unsorted" {
                        summary.unsorted += 1;
                        manifest.record(action_str, category, &source_str, &target_str, "", "", &reason);
                        ItemOutcome::Unsorted
                    } else {
                        summary.corrupt += 1;
                        manifest.record(action_str, category, &source_str, &target_str, "", "", &reason);
                        ItemOutcome::Corrupt
                    }
                }
                Err(e) => {
                    let reason = format!("{e:#}");
                    warn!("Failed to transfer '{source_str}': {reason}");
                    summary.failed.push((source_str.clone(), reason.clone()));
                    manifest.record(
                        "failed",
                        category,
                        &source_str,
                        &target_str,
                        "",
                        "",
                        &reason,
                    );
                    ItemOutcome::Failed { reason }
                }
            }
        }
    }
}

/// One-shot convenience used by the CLI: plan, then execute everything.
/// `on_progress` is called after each processed file with (done, total).
pub fn process(
    source: &Path,
    target: &Path,
    options: &ProcessOptions,
    mut on_progress: impl FnMut(usize, usize),
) -> anyhow::Result<ProcessSummary> {
    let plan = plan(source, target, options, |_, _| {})?;
    let total = plan.items.len();
    execute(plan, options, |index, _| on_progress(index + 1, total))
}

/// True when the first bytes carry any known file signature. Carved files
/// (PhotoRec output) frequently have an image extension but garbage content.
/// Deliberately conservative: only signature-less content is called corrupt.
fn content_recognized(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    let Ok(mut file) = File::open(path) else {
        // Unreadable says nothing about the content.
        return true;
    };
    let n = file.read(&mut buf).unwrap_or(0);
    infer::get(&buf[..n]).is_some()
}

/// Byte-compare two files (size first, then streaming chunks).
fn files_identical(a: &Path, b: &Path) -> std::io::Result<bool> {
    if fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let mut reader_a = BufReader::new(File::open(a)?);
    let mut reader_b = BufReader::new(File::open(b)?);
    let mut buf_a = [0u8; 8192];
    let mut buf_b = [0u8; 8192];
    loop {
        let n = reader_a.read(&mut buf_a)?;
        if n == 0 {
            return Ok(true);
        }
        reader_b.read_exact(&mut buf_b[..n])?;
        if buf_a[..n] != buf_b[..n] {
            return Ok(false);
        }
    }
}

/// Pick a filename that does not exist in `dir`, appending a random suffix
/// on collision (same scheme as `Image::set_target`).
fn unique_name_in(dir: &Path, stem: &str, ext: &str) -> String {
    let mut name = stem.to_string();
    while dir.join(format!("{name}.{ext}")).exists() {
        let random = rand::thread_rng().gen_range(1..999999);
        name = format!("{name}_{random}");
    }
    name
}

/// Undo a previous run from its manifest: copied files are deleted from the
/// target (only while the source still exists — never the last copy), moved
/// files are moved back. Returns (reverted, skipped).
pub fn revert(manifest_path: &Path, dry_run: bool) -> anyhow::Result<(usize, usize)> {
    let entries = manifest::read_manifest(manifest_path)?;
    let mut reverted = 0;
    let mut skipped = 0;

    for entry in entries.iter().rev() {
        let target = Path::new(&entry.target);
        let source = Path::new(&entry.source);
        match entry.action.as_str() {
            "copied" => {
                if target.exists() && source.exists() {
                    debug!("revert: remove copy {}", entry.target);
                    if !dry_run {
                        fs::remove_file(target)?;
                    }
                    reverted += 1;
                } else {
                    warn!(
                        "revert: skipping '{}' (copy or original missing)",
                        entry.target
                    );
                    skipped += 1;
                }
            }
            "moved" => {
                if target.exists() && !source.exists() {
                    debug!("revert: move {} back to {}", entry.target, entry.source);
                    if !dry_run {
                        if let Some(parent) = source.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        if fs::rename(target, source).is_err() {
                            fs::copy(target, source)?;
                            fs::remove_file(target)?;
                        }
                    }
                    reverted += 1;
                } else {
                    warn!(
                        "revert: skipping '{}' (already restored or target missing)",
                        entry.target
                    );
                    skipped += 1;
                }
            }
            // duplicate / collision_skipped / failed touched nothing
            _ => {}
        }
    }

    Ok((reverted, skipped))
}
