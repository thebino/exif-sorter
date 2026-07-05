use std::{
    ffi::OsStr,
    fs::{self, File, FileType},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::bail;
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Utc};
use exif::{Exif, In, Tag};
use rand::Rng as _;
use tracing::{debug, error, info};

use crate::error::AppError;

use super::dates::Dates;

/// Where a date was extracted from, ordered by trustworthiness.
/// Recovered files (e.g. PhotoRec output) often carry filesystem timestamps
/// from the recovery run, not the capture — callers can use this to route
/// low-confidence dates differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateSource {
    ExifDateTimeOriginal,
    ExifDateTimeDigitized,
    ExifDateTime,
    ExifGpsDate,
    FileCreated,
    FileModified,
}

impl std::fmt::Display for DateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DateSource::ExifDateTimeOriginal => "EXIF DateTimeOriginal",
            DateSource::ExifDateTimeDigitized => "EXIF DateTimeDigitized",
            DateSource::ExifDateTime => "EXIF DateTime",
            DateSource::ExifGpsDate => "EXIF GPSDateStamp",
            DateSource::FileCreated => "file creation date",
            DateSource::FileModified => "file modified date",
        };
        f.write_str(s)
    }
}

impl DateSource {
    /// File timestamps are unreliable on recovered media (they reflect the
    /// recovery, not the capture).
    pub fn is_low_confidence(&self) -> bool {
        matches!(self, DateSource::FileCreated | DateSource::FileModified)
    }
}

#[derive(Clone)]
pub struct Image {
    pub source_path: PathBuf,
    pub source_filename: String,
    pub source_filetype: String,
    pub dates: Dates,
    pub target_dir: PathBuf,
    pub target_filename: String,
    pub target_filetype: String,
    pub error: Option<AppError>,
}

impl Image {
    pub fn new(path: PathBuf, target: PathBuf) -> Self {
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let filetype = path
            .extension()
            .unwrap_or(OsStr::new(""))
            .to_string_lossy()
            .into_owned();

        let metadata = fs::metadata(path.clone());
        let created = match metadata {
            Ok(metadata) => metadata.created().ok(),
            Err(_) => None,
        };

        let metadata = fs::metadata(path.clone());

        let modified = match metadata {
            Ok(metadata) => metadata.modified().ok(),
            Err(_) => None,
        };

        Self {
            source_path: path.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
            source_filename: filename.to_string(),
            source_filetype: filetype.clone(),
            dates: Dates::new(created, modified),
            target_dir: target,
            target_filename: filename.to_string(),
            target_filetype: filetype,
            error: None,
        }
    }

    pub fn source_filename(&self) -> String {
        format!("{}.{}", self.source_filename, self.source_filetype)
    }

    pub fn source_full(&self) -> String {
        format!(
            "{}/{}.{}",
            self.source_path.to_string_lossy(),
            self.source_filename,
            self.source_filetype
        )
    }

    pub fn target_filename(&self) -> String {
        format!("{}.{}", self.target_filename, self.target_filetype)
    }

    pub fn target_full(&self) -> String {
        format!(
            "{}/{}.{}",
            self.target_dir.to_string_lossy(),
            self.target_filename,
            self.target_filetype
        )
    }

    pub fn read_exif(&self) -> anyhow::Result<NaiveDate> {
        self.read_exif_date().map(|(date, _)| date)
    }

    /// Read the capture date from EXIF, trying tags from most to least
    /// specific: DateTimeOriginal → DateTimeDigitized → DateTime → GPSDateStamp.
    /// Implausible dates (camera clock reset to epoch/2000, dates in the
    /// future) are skipped so the next source gets a chance.
    pub fn read_exif_date(&self) -> anyhow::Result<(NaiveDate, DateSource)> {
        let full_path = self
            .source_path
            .join(format!("{}.{}", self.source_filename, self.source_filetype));

        let file = std::fs::File::open(full_path.as_path())?;

        let mut bufreader = std::io::BufReader::new(&file);
        let exifreader = exif::Reader::new();
        let exif = match exifreader.read_from_container(&mut bufreader) {
            Ok(exif) => exif,
            Err(_) => bail!(AppError::NoExifInformation()),
        };

        const DATETIME_TAGS: [(Tag, DateSource); 3] = [
            (Tag::DateTimeOriginal, DateSource::ExifDateTimeOriginal),
            (Tag::DateTimeDigitized, DateSource::ExifDateTimeDigitized),
            (Tag::DateTime, DateSource::ExifDateTime),
        ];

        for (tag, source) in DATETIME_TAGS {
            match Self::extract_datetime_from_exif(&exif, tag) {
                Ok(datetime) => {
                    let date = datetime.date();
                    if Self::is_plausible_date(date) {
                        return Ok((date, source));
                    }
                    debug!(
                        "File '{}': implausible {source} '{date}', trying next source",
                        self.source_full()
                    );
                }
                Err(e) => {
                    debug!("File '{}': no {source} ({e:#})", self.source_full());
                }
            }
        }

        // GPS date comes from the satellite fix, independent of the camera
        // clock — a good last resort when all datetime tags are missing.
        if let Some(field) = exif.get_field(Tag::GPSDateStamp, In::PRIMARY) {
            let date_str = field.display_value().to_string().replace('"', "");
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .or_else(|_| NaiveDate::parse_from_str(&date_str, "%Y:%m:%d"));
            if let Ok(date) = date {
                if Self::is_plausible_date(date) {
                    return Ok((date, DateSource::ExifGpsDate));
                }
            }
        }

        bail!(AppError::NoExifDateFound())
    }

    /// Full fallback chain: EXIF tags first, then filesystem timestamps
    /// (earliest of creation/modified — on copied or recovered files the
    /// modified date often predates the creation date).
    pub fn extract_date(&self) -> anyhow::Result<(NaiveDate, DateSource)> {
        if let Ok(result) = self.read_exif_date() {
            return Ok(result);
        }

        let created = self.dates.file_creation_date.filter(|d| Self::is_plausible_date(*d));
        let modified = self.dates.file_modified_date.filter(|d| Self::is_plausible_date(*d));

        match (created, modified) {
            (Some(c), Some(m)) if m < c => Ok((m, DateSource::FileModified)),
            (Some(c), _) => Ok((c, DateSource::FileCreated)),
            (None, Some(m)) => Ok((m, DateSource::FileModified)),
            (None, None) => bail!(AppError::NoDateFound()),
        }
    }

    /// Cameras with a dead clock battery reset to 1970 (epoch); dates before
    /// consumer digital photography or in the future are rejected. A reset to
    /// 2000-01-01 is indistinguishable from a real photo taken that day and
    /// passes this check.
    pub fn is_plausible_date(date: NaiveDate) -> bool {
        date.year() >= 1980 && date <= Utc::now().date_naive()
    }

    fn extract_datetime_from_exif(exif: &Exif, tag: Tag) -> anyhow::Result<NaiveDateTime> {
        let date = exif.get_field(tag, In::PRIMARY);
        match date {
            Some(date) => {
                let date_str = date.display_value().to_string();
                // some e.g. hasselblad_x1d have additional quotes
                let date_str = date_str.replace("\"", "");
                let datetime = if date_str.contains("T") {
                    chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S")
                } else {
                    chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S")
                };

                match datetime {
                    Ok(datetime) => Ok(datetime),
                    Err(e) => {
                        bail!(AppError::DateTimeParsingEror(e))
                    }
                }
            }
            None => {
                bail!(AppError::NoDateTimeOriginalFound())
            }
        }
    }

    pub fn extract_file_creation_date(self) -> anyhow::Result<NaiveDate> {
        let file = File::open(self.source_path)?;
        let system_time = file.metadata()?.created()?;
        let duration_since_epoch = system_time
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow::anyhow!("File creation timestamp is before Unix epoch: {e}"))?;

        let secs = duration_since_epoch.as_secs() as i64;
        let nanos = duration_since_epoch.subsec_nanos();

        Ok(NaiveDateTime::from_timestamp_opt(secs, nanos)
            .ok_or_else(|| anyhow::anyhow!("Timestamp out of range: {secs}s {nanos}ns"))?
            .date())
    }

    pub fn extract_file_modified_date(self) -> anyhow::Result<NaiveDate> {
        let file = File::open(self.source_path)?;
        let system_time = file.metadata()?.modified()?;
        let duration_since_epoch = system_time
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow::anyhow!("File modification timestamp is before Unix epoch: {e}"))?;

        let secs = duration_since_epoch.as_secs() as i64;
        let nanos = duration_since_epoch.subsec_nanos();

        Ok(NaiveDateTime::from_timestamp_opt(secs, nanos)
            .ok_or_else(|| anyhow::anyhow!("Timestamp out of range: {secs}s {nanos}ns"))?
            .date())
    }

    /// Set target fields based on configuration
    pub fn set_target(&self, date: NaiveDate) -> anyhow::Result<(PathBuf, String)> {
        let year_str = date.year().to_string();
        let date_str = format!("{}/{year_str}/{date}", self.target_dir.to_string_lossy());
        let target_dir = Path::new(date_str.as_str()).to_path_buf();

        let mut filename = self.target_filename.clone();
        let filetype = self.target_filetype.clone();
        let target_str = format!("{filename}.{filetype}");
        let mut target_path = target_dir.join(target_str);

        while target_path.exists() {
            error!("target_path exists {}", target_path.to_string_lossy());
            let mut rng = rand::thread_rng();

            let random = rng.gen_range(1..999999);
            filename = format!("{filename}_{random}");
            let target_str = format!(
                "{}/{}.{}",
                date_str.clone(),
                filename.clone(),
                filetype.clone()
            );
            target_path = Path::new(&target_str.to_owned()).to_path_buf()
        }

        Ok((target_dir, filename))
    }

    /// Move given files based on its target configuration
    pub fn move_to_target(self, dry_run: bool) -> anyhow::Result<()> {
        if !self.target_dir.exists() {
            debug!("Create target dir {}", self.target_dir.to_string_lossy());

            if !dry_run {
                fs::create_dir_all(self.target_dir.clone())?;
            }
        }

        info!("move {} to {}", self.source_full(), self.target_full());
        if !dry_run {
            let source_str = self.source_full();
            let target_str = self.target_full();
            let source = Path::new(&source_str);
            let target = Path::new(&target_str);

            // Atomically claim the target path before writing. If another
            // process created the file between set_target() and here (TOCTOU),
            // create_new returns AlreadyExists and we abort rather than
            // silently overwriting data that belongs to someone else.
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|e| anyhow::anyhow!("Cannot create target {}: {e}", self.target_full()))?;

            if let Err(e) = fs::copy(source, target) {
                let _ = fs::remove_file(target); // remove the empty claim file
                return Err(e.into());
            }

            fs::remove_file(source)?;
        }

        Ok(())
    }
}
