use std::{fs::File, io::Read, path::Path, sync::Arc};

use eframe::egui::mutex::Mutex;
use ignore::{Walk, WalkBuilder};
use tracing::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

use super::image::Image;

fn is_image_file(entry: &ignore::DirEntry) -> bool {
    matches!(
        entry.path().extension().and_then(|s| s.to_str()),
        Some(
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "dng" | "nef" | "cr2" | "arw" | "fff"
        )
    )
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
        Box::new(|result| {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(true) && is_image_file(&entry)
                {
                    debug!(
                        "{:<100}",
                        entry.clone().into_path().into_os_string().to_str().unwrap()
                    );

                    let mut files = files_arc.lock();
                    files.push(entry);
                }
            }
            ignore::WalkState::Continue

            /*
            let entry = result.unwrap();
            // TODO: skip directories
            warn!(
                "{:<100}",
                entry.clone().into_path().into_os_string().to_str().unwrap()
            );
            let mut guard = files_arc.lock();
            guard.push(entry);

            ignore::WalkState::Continue
            */
        })
    });

    let mut guard = files_arc.lock();
    files.append(&mut guard);

    info!("Found {} images", files.len());

    Ok(files)
}
