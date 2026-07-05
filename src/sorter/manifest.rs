use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;

pub const MANIFEST_FILENAME: &str = "exif-sorter-manifest.csv";
pub const MANIFEST_HEADER: &str = "timestamp,action,category,source,target,date,date_source,reason";

/// Appends one CSV row per decision to `{target}/exif-sorter-manifest.csv` —
/// the audit trail ("why is this photo in 2009?") and the input for `revert`.
/// Opens the file lazily so runs that touch nothing leave nothing behind;
/// records nothing on dry runs.
pub struct ManifestWriter {
    path: PathBuf,
    dry_run: bool,
    file: Option<File>,
}

/// A single manifest row, as consumed by `revert`.
pub struct ManifestEntry {
    pub action: String,
    pub source: String,
    pub target: String,
}

impl ManifestWriter {
    pub fn new(target_dir: &Path, dry_run: bool) -> Self {
        Self {
            path: target_dir.join(MANIFEST_FILENAME),
            dry_run,
            file: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        action: &str,
        category: &str,
        source: &str,
        target: &str,
        date: &str,
        date_source: &str,
        reason: &str,
    ) {
        if self.dry_run {
            return;
        }
        if self.file.is_none() {
            self.file = open_appending(&self.path);
        }
        if let Some(file) = &mut self.file {
            let row = [
                Utc::now().to_rfc3339().as_str(),
                action,
                category,
                source,
                target,
                date,
                date_source,
                reason,
            ]
            .iter()
            .map(|f| csv_escape(f))
            .collect::<Vec<_>>()
            .join(",");
            // A failed manifest write must not abort the sort itself.
            let _ = writeln!(file, "{row}");
        }
    }
}

fn open_appending(path: &Path) -> Option<File> {
    let is_new = !path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()?;
    if is_new {
        writeln!(file, "{MANIFEST_HEADER}").ok()?;
    }
    Some(file)
}

/// Read all entries of a manifest file (header skipped).
pub fn read_manifest(path: &Path) -> anyhow::Result<Vec<ManifestEntry>> {
    let content = std::fs::read_to_string(path)?;
    let entries = content
        .lines()
        .skip(1) // header
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let fields = parse_csv_line(line);
            // timestamp,action,category,source,target,...
            if fields.len() >= 5 {
                Some(ManifestEntry {
                    action: fields[1].clone(),
                    source: fields[3].clone(),
                    target: fields[4].clone(),
                })
            } else {
                None
            }
        })
        .collect();
    Ok(entries)
}

fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Minimal CSV line parser, the inverse of `csv_escape`.
pub fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => fields.push(std::mem::take(&mut current)),
            c => current.push(c),
        }
    }
    fields.push(current);
    fields
}
