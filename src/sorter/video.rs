use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use chrono::NaiveDate;

/// Seconds between the MP4/QuickTime epoch (1904-01-01) and the unix epoch.
const MP4_EPOCH_OFFSET: i64 = 2_082_844_800;

pub fn is_video_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp4" | "mov" | "m4v" | "3gp"
    )
}

/// Read the creation date from an MP4/QuickTime container (`moov`/`mvhd`
/// box). MOV and MP4 share this structure. Returns None when the box is
/// missing or the creation time is unset (cameras that never had a clock
/// write 0, i.e. 1904-01-01 — filtered by the caller's plausibility check).
pub fn creation_date(path: &Path) -> Option<NaiveDate> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let (moov_start, moov_size) = find_box(&mut file, 0, len, b"moov")?;
    let (mvhd_start, mvhd_size) = find_box(&mut file, moov_start, moov_start + moov_size, b"mvhd")?;
    if mvhd_size < 12 {
        return None;
    }

    file.seek(SeekFrom::Start(mvhd_start)).ok()?;
    let mut head = [0u8; 12];
    file.read_exact(&mut head).ok()?;

    // FullBox: version (1) + flags (3), then creation_time — u32 in
    // version 0, u64 in version 1.
    let seconds_since_1904 = match head[0] {
        0 => u32::from_be_bytes(head[4..8].try_into().ok()?) as i64,
        1 => i64::try_from(u64::from_be_bytes({
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&head[4..12]);
            buf
        }))
        .ok()?,
        _ => return None,
    };

    let unix = seconds_since_1904 - MP4_EPOCH_OFFSET;
    Some(chrono::NaiveDateTime::from_timestamp_opt(unix, 0)?.date())
}

/// Walk sibling boxes in `[offset, end)` and return (content_start,
/// content_size) of the first box named `name`.
fn find_box(file: &mut File, mut offset: u64, end: u64, name: &[u8; 4]) -> Option<(u64, u64)> {
    while offset + 8 <= end {
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut header = [0u8; 8];
        file.read_exact(&mut header).ok()?;
        let size32 = u32::from_be_bytes(header[0..4].try_into().ok()?);
        let box_type: [u8; 4] = header[4..8].try_into().ok()?;

        let (header_len, box_size) = match size32 {
            0 => (8u64, end - offset), // box extends to end of enclosing space
            1 => {
                let mut large = [0u8; 8];
                file.read_exact(&mut large).ok()?;
                (16u64, u64::from_be_bytes(large))
            }
            s => (8u64, s as u64),
        };
        if box_size < header_len {
            return None; // corrupt size, stop walking
        }
        if &box_type == name {
            return Some((offset + header_len, box_size - header_len));
        }
        offset = offset.checked_add(box_size)?;
    }
    None
}
