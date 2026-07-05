use exif_sorter::sorter::dir::scan_dir;

#[test]
fn scan_finds_uppercase_extensions() {
    // Cameras write uppercase extensions (DCIM convention). A case-sensitive
    // match silently skips every file on such media — the sorter reports
    // "Found 0 images" on a card full of photos.
    // given
    let tmp = testdir::testdir!();
    std::fs::write(tmp.join("DSC09903.ARW"), b"x").unwrap();
    std::fs::write(tmp.join("R0010002.JPG"), b"x").unwrap();
    std::fs::write(tmp.join("mixed_case.Jpeg"), b"x").unwrap();
    std::fs::write(tmp.join("IMG_0001.HEIC"), b"x").unwrap();
    std::fs::write(tmp.join("clip.MOV"), b"x").unwrap();
    std::fs::write(tmp.join("notes.txt"), b"x").unwrap();

    // when
    let entries = scan_dir(&tmp).unwrap();

    // then
    assert_eq!(entries.len(), 5, "expected all media files regardless of extension case");
}
