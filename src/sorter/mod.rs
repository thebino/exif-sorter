pub mod config;
pub mod dates;
pub mod dir;
pub mod filename_date;
pub mod image;
pub mod manifest;
pub mod video;

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::bail;
use rand::Rng as _;
use rayon::prelude::*;
use tracing::{debug, warn};

use crate::error::AppError;
use dir::scan_dir;
use image::Image;
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

/// Outcome of a sort run. `failed` carries (path, reason) pairs so frontends
/// can show the user exactly which files need manual attention.
#[derive(Default)]
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

/// Scan `source`, date every image and transfer it into
/// `target/{year}/{date}/`. Undatable files land in `target/unsorted/`,
/// unrecognizable content in `target/corrupt/`. Every decision is appended
/// to `target/exif-sorter-manifest.csv`.
///
/// This is the shared core for all frontends (CLI/TUI/GUI). `on_progress`
/// is called after each processed file with (done, total).
///
/// Date extraction (EXIF parsing dominates the runtime) runs in parallel;
/// the transfers themselves stay sequential so collision handling never
/// races against itself.
pub fn process(
    source: &Path,
    target: &Path,
    options: &ProcessOptions,
    mut on_progress: impl FnMut(usize, usize),
) -> anyhow::Result<ProcessSummary> {
    if !source.exists() {
        bail!(AppError::InvalidSource {
            expected: source.to_string_lossy().into_owned()
        });
    }
    fs::create_dir_all(target)?;

    let mut entries = scan_dir(source)?;

    // Never re-sort files already inside the target tree — with the default
    // arguments the target directory lives inside the source directory.
    let target_canon = target.canonicalize()?;
    entries.retain(|entry| {
        entry
            .path()
            .canonicalize()
            .map(|p| !p.starts_with(&target_canon))
            .unwrap_or(true)
    });

    let total = entries.len();

    let dated: Vec<_> = entries
        .into_par_iter()
        .map(|entry| {
            let image = Image::new(entry.into_path(), target.to_path_buf());
            let date = image.extract_date();
            (image, date)
        })
        .collect();

    let mut manifest = ManifestWriter::new(target, options.dry_run);
    let action = match options.mode {
        TransferMode::Copy => "copied",
        TransferMode::Move => "moved",
    };

    // Routing decision for a single file, in order of trust:
    // - a metadata-derived date (EXIF/GPS) implies valid content → sorted
    // - otherwise the content signature decides: unrecognizable bytes are
    //   carved garbage whose file dates mean nothing → corrupt/
    // - recognizable content without any date → unsorted/
    enum Route {
        Sorted(chrono::NaiveDate, image::DateSource),
        Unsorted(String),
        Corrupt(String),
    }

    let mut summary = ProcessSummary::default();
    for (done, (mut image, date)) in dated.into_iter().enumerate() {
        let route = match date {
            Ok((date, date_source)) if !date_source.is_low_confidence() => {
                Route::Sorted(date, date_source)
            }
            Ok((date, date_source)) => {
                if content_recognized(Path::new(&image.source_full())) {
                    warn!(
                        "File '{}': no exif date, using {date_source} '{date}' (unreliable on recovered media)",
                        image.source_full()
                    );
                    Route::Sorted(date, date_source)
                } else {
                    Route::Corrupt("content not recognized as any known file type".to_string())
                }
            }
            Err(e) => {
                if content_recognized(Path::new(&image.source_full())) {
                    Route::Unsorted(format!("{e:#}"))
                } else {
                    Route::Corrupt(format!(
                        "no usable date and content not recognized ({e:#})"
                    ))
                }
            }
        };

        match route {
            Route::Sorted(date, date_source) => {
                debug!(
                    "File '{}' has date '{date}' from {date_source}",
                    image.source_full()
                );

                // Collision handling on the plain (unsuffixed) target path.
                let plain_dir = image.target_dir_for(date, &options.pattern);
                let plain_path = plain_dir.join(image.target_filename());
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
                        on_progress(done + 1, total);
                        continue;
                    }
                    CollisionPolicy::Dedupe if plain_path.exists() => {
                        match files_identical(Path::new(&image.source_full()), &plain_path) {
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
                                on_progress(done + 1, total);
                                continue;
                            }
                            Ok(false) => {} // different content → suffix below
                            Err(e) => {
                                summary
                                    .failed
                                    .push((image.source_full(), format!("dedupe compare failed: {e}")));
                                on_progress(done + 1, total);
                                continue;
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
                                if date_source.is_low_confidence() {
                                    summary.low_confidence += 1;
                                }
                                manifest.record(
                                    action,
                                    "sorted",
                                    &source_str,
                                    &target_str,
                                    &date.to_string(),
                                    &date_source.to_string(),
                                    "",
                                );
                            }
                            Err(e) => {
                                warn!("Failed to transfer '{source_str}': {e:#}");
                                summary.failed.push((source_str.clone(), format!("{e:#}")));
                                manifest.record(
                                    "failed",
                                    "sorted",
                                    &source_str,
                                    &target_str,
                                    &date.to_string(),
                                    &date_source.to_string(),
                                    &format!("{e:#}"),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to resolve target for '{}': {e:#}", image.source_full());
                        summary.failed.push((image.source_full(), format!("{e:#}")));
                    }
                }
            }
            route @ (Route::Unsorted(_) | Route::Corrupt(_)) => {
                let (category, reason) = match route {
                    Route::Unsorted(reason) => ("unsorted", reason),
                    Route::Corrupt(reason) => ("corrupt", reason),
                    Route::Sorted(..) => unreachable!(),
                };
                let category_dir = target.join(category);
                image.target_dir = category_dir.clone();
                image.target_filename = unique_name_in(
                    &category_dir,
                    &image.target_filename,
                    &image.target_filetype,
                );

                warn!(
                    "File '{}': {reason} — {action} to {category}/",
                    image.source_full()
                );
                let source_str = image.source_full();
                let target_str = image.target_full();
                match image.transfer_to_target(options.mode, options.dry_run) {
                    Ok(()) => {
                        match category {
                            "unsorted" => summary.unsorted += 1,
                            _ => summary.corrupt += 1,
                        }
                        manifest.record(action, category, &source_str, &target_str, "", "", &reason);
                    }
                    Err(e) => {
                        warn!("Failed to transfer '{source_str}': {e:#}");
                        summary.failed.push((source_str.clone(), format!("{e:#}")));
                        manifest.record(
                            "failed",
                            category,
                            &source_str,
                            &target_str,
                            "",
                            "",
                            &format!("{e:#}"),
                        );
                    }
                }
            }
        }
        on_progress(done + 1, total);
    }

    Ok(summary)
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
