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
        let filename = format!(
            "{}.{}",
            self.source_filename.clone(),
            self.source_filetype.clone()
        );
        let a = self.source_path.clone().join(filename);

        let path = std::fs::File::open(a.as_path())?;

        let mut bufreader = std::io::BufReader::new(&path);
        let exifreader = exif::Reader::new();
        let exif = exifreader.read_from_container(&mut bufreader);

        let date_time_original = match exif {
            Ok(exif) => self.extract_datetimeoriginal_from_exif(exif),
            Err(_) => bail!(AppError::NoExifInformation()),
        }?
        .date();

        Ok(date_time_original)
    }

    fn extract_datetimeoriginal_from_exif(&self, exif: Exif) -> anyhow::Result<NaiveDateTime> {
        let date = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY);
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
