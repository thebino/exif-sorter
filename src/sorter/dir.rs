use std::path::Path;
use std::sync::{Arc, Mutex};

use ignore::WalkBuilder;
use tracing::{debug, info};

const SUPPORTED_EXTENSIONS: [&str; 17] = [
    // images
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "heic", "heif",
    // raw
    "dng", "nef", "cr2", "arw", "fff",
    // video (MP4/QuickTime containers)
    "mp4", "mov", "m4v", "3gp",
];

fn is_image_file(entry: &ignore::DirEntry) -> bool {
    // Case-insensitive: cameras write uppercase extensions (DSC09903.ARW,
    // R0010002.JPG) into DCIM directories.
    entry
        .path()
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|e| ext.eq_ignore_ascii_case(e))
        })
        .unwrap_or(false)
}

/// Scan given directory including its subdirectories and returns a list of findings including their source path
pub fn scan_dir(dir: &Path) -> anyhow::Result<Vec<ignore::DirEntry>> {
    let mut files: Vec<ignore::DirEntry> = Vec::new();
    let files_arc: Arc<Mutex<Vec<ignore::DirEntry>>> = Arc::new(Mutex::new(Vec::new()));

    let walker = WalkBuilder::new(dir)
        .hidden(true) // ignoring hidden files
        .filter_entry(|entry| {
            entry
                .file_type()
                .map(|ft| ft.is_dir()) // add subdirectories to walker
                .unwrap_or(false)
                || is_image_file(entry)
        })
        .build_parallel();

    walker.run(|| {
        let files_arc = Arc::clone(&files_arc);
        Box::new(move |result| {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(true) && is_image_file(&entry)
                {
                    // Use OsStr-based display so non-UTF-8 filenames don't panic.
                    debug!("{:<100}", entry.path().as_os_str().to_string_lossy());

                    files_arc.lock().expect("mutex poisoned").push(entry);
                }
            }
            ignore::WalkState::Continue
        })
    });

    let mut guard = files_arc.lock().expect("mutex poisoned");
    files.append(&mut guard);

    info!("Found {} images", files.len());

    Ok(files)
}
