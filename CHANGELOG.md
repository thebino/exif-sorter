# Changelog

All notable changes to this project are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-07-06

First proper release. exif-sorter reads the metadata of photos and videos and
sorts them into date-based folders, built for cleaning up recovered media
(e.g. PhotoRec output) where trustworthy dates are scarce and safety matters.

### Added

- **Date fallback chain.** Capture date is resolved in order of trust: EXIF
  `DateTimeOriginal` → `DateTimeDigitized` → `DateTime` → GPS date stamp →
  MP4/QuickTime creation time → a date embedded in the filename
  (`IMG_20190412_…`, WhatsApp, Signal, `YYYY-MM-DD`) → file timestamps. Every
  candidate passes a plausibility check (rejects dead-clock epoch resets and
  future dates); file-timestamp dates are flagged low-confidence.
- **Copy by default.** Files are copied, not moved — the source stays
  untouched unless `--move` is given. Both modes preserve the file's modified
  time (PhotoRec stamps the capture date there).
- **Manifest and undo.** Every decision is appended to
  `{target}/exif-sorter-manifest.csv`; `exif-sorter revert -m <manifest>`
  reverses a run.
- **Safe routing.** Files with no usable date go to `unsorted/`;
  content with no recognizable signature (carved garbage) goes to `corrupt/`,
  so nothing is silently misfiled.
- **Collision policies.** `--on-collision suffix|skip|dedupe`; dedupe
  byte-compares and stores identical content only once.
- **Formats.** Images (png, jpg, jpeg, gif, bmp, webp, heic, heif), raw
  (dng, nef, cr2, arw, fff) and video (mp4, mov, m4v, 3gp), matched
  case-insensitively.
- **Working TUI and GUI.** Both offer a scan → review → confirm flow: scanning
  is read-only, the review table shows every file with its detected date, the
  date's origin and the planned target; you adjust the selection and run,
  watching per-file progress. Processed files are locked so a follow-up run
  never double-processes them.
- **Configuration.** `~/.config/exif-sorter/config.toml` (`pattern`, `move`,
  `on_collision`); folder layout tokens `{year}`, `{month}`, `{day}`,
  `{date}`. Command-line flags override the config.
- **Distribution.** Native installers built out of the box — `.deb`/`.rpm`
  (Linux), `.dmg` (macOS) and `.msi` (Windows) — plus portable archives, shell
  completions and a man page. Published to crates.io and a Homebrew tap.

### Notes

- The macOS `.dmg` and Windows `.msi` are currently **unsigned**: on macOS
  open via right-click → Open the first time; on Windows click through the
  SmartScreen "unknown publisher" prompt. Signing can be enabled later without
  a workflow change.

[1.0.0]: https://github.com/thebino/exif-sorter/releases/tag/v1.0.0
