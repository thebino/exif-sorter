use chrono::NaiveDate;

use super::image::Image;

/// Extract a capture date embedded in a file name.
///
/// Phone and messenger exports are often EXIF-stripped but keep the date in
/// the name: `IMG_20190412_183000.jpg`, `PXL_20210704_123456789.jpg`,
/// `IMG-20200105-WA0001.jpg` (WhatsApp), `signal-2020-01-05-120000.jpg`,
/// `2019-04-12 vacation.jpg`.
///
/// Two shapes are recognized:
/// - a run of 8+ digits starting with a valid `YYYYMMDD`
/// - `YYYY?MM?DD` with a single separator character out of `-_.: `
///
/// Every candidate must pass `Image::is_plausible_date`, which rejects the
/// false positives that pure digit matching produces (serial numbers, frame
/// counters, unix timestamps).
pub fn date_from_filename(stem: &str) -> Option<NaiveDate> {
    let chars: Vec<char> = stem.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && chars[i].is_ascii_digit() {
            i += 1;
        }
        let run: String = chars[start..i].iter().collect();

        // YYYYMMDD (possibly followed by more digits, e.g. YYYYMMDDHHMMSS)
        if run.len() >= 8 {
            if let Some(date) = parse_ymd(&run[0..4], &run[4..6], &run[6..8]) {
                return Some(date);
            }
        }

        // YYYY<sep>MM<sep>DD with the same separator twice
        if run.len() == 4 && i + 6 <= n {
            let sep = chars[i];
            if matches!(sep, '-' | '_' | '.' | ':' | ' ')
                && chars[i + 1].is_ascii_digit()
                && chars[i + 2].is_ascii_digit()
                && chars[i + 3] == sep
                && chars[i + 4].is_ascii_digit()
                && chars[i + 5].is_ascii_digit()
            {
                let month: String = chars[i + 1..i + 3].iter().collect();
                let day: String = chars[i + 4..i + 6].iter().collect();
                if let Some(date) = parse_ymd(&run, &month, &day) {
                    return Some(date);
                }
            }
        }
    }
    None
}

fn parse_ymd(year: &str, month: &str, day: &str) -> Option<NaiveDate> {
    let date = NaiveDate::from_ymd_opt(
        year.parse().ok()?,
        month.parse().ok()?,
        day.parse().ok()?,
    )?;
    Image::is_plausible_date(date).then_some(date)
}
