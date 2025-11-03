use std::time::SystemTime;

use chrono::{DateTime, NaiveDate, Utc};

#[derive(Default, Clone)]
pub struct Dates {
    pub file_creation_date: Option<NaiveDate>,
    pub file_modified_date: Option<NaiveDate>,
    pub exif_date_time_original: Option<NaiveDate>,
}

impl Dates {
    pub fn new(created: Option<SystemTime>, modified: Option<SystemTime>) -> Self {
        let created = created.map(|created| DateTime::<Utc>::from(created).date_naive());
        let modified = modified.map(|modified| DateTime::<Utc>::from(modified).date_naive());

        Self {
            file_creation_date: created,
            file_modified_date: modified,
            exif_date_time_original: None,
        }
    }
}
