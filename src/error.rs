#[derive(thiserror::Error, Clone, Debug)]
pub enum AppError {
    #[error("Could not parse {0} from DateTimeOriginal!")]
    DateTimeParsingEror(chrono::ParseError),
    #[error("Intermittent IO error during iteration")]
    IntermittentIO(),
    #[error("Invalid source directory: {expected:?} could not be found!")]
    InvalidSource { expected: String },
    #[error("No DateTimeOriginal found!")]
    NoDateTimeOriginalFound(),
    #[error("No usable date tag found in exif data!")]
    NoExifDateFound(),
    #[error("No exif information!")]
    NoExifInformation(),
    #[error("No date found in exif data or file metadata!")]
    NoDateFound(),
}
