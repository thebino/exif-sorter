use std::path::Path;

use serde::Deserialize;
use tracing::warn;

/// Folder layout below the target directory; see `render_pattern` for tokens.
pub const DEFAULT_PATTERN: &str = "{year}/{date}";

/// Optional user configuration, loaded from
/// `~/.config/exif-sorter/config.toml` (or the platform equivalent) or an
/// explicit `--config` path. Command-line flags take precedence over it.
///
/// ```toml
/// pattern = "{year}/{month}"
/// move = false
/// on_collision = "dedupe"
/// ```
#[derive(Debug, Default, Deserialize)]
pub struct SorterConfig {
    /// Folder layout, e.g. "{year}/{month}/{day}" or "{year}/{date}".
    pub pattern: Option<String>,
    /// Move instead of copy.
    #[serde(rename = "move")]
    pub move_files: Option<bool>,
    /// "suffix", "skip" or "dedupe".
    pub on_collision: Option<String>,
}

impl SorterConfig {
    pub fn load(explicit_path: Option<&Path>) -> Self {
        let path = match explicit_path {
            Some(path) => path.to_path_buf(),
            None => match dirs::config_dir() {
                Some(dir) => dir.join("exif-sorter").join("config.toml"),
                None => return Self::default(),
            },
        };
        if !path.exists() {
            if explicit_path.is_some() {
                warn!("config file '{}' not found, using defaults", path.display());
            }
            return Self::default();
        }
        match std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|content| toml::from_str(&content).map_err(anyhow::Error::from))
        {
            Ok(config) => config,
            Err(e) => {
                warn!("could not read config '{}': {e:#}", path.display());
                Self::default()
            }
        }
    }
}

/// Expand the folder pattern for a date. Tokens: `{year}`, `{month}`,
/// `{day}` (zero-padded) and `{date}` (`YYYY-MM-DD`).
pub fn render_pattern(pattern: &str, date: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    pattern
        .replace("{year}", &date.year().to_string())
        .replace("{month}", &format!("{:02}", date.month()))
        .replace("{day}", &format!("{:02}", date.day()))
        .replace("{date}", &date.to_string())
}
