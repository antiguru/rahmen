//! Integration tests reading the committed `testdata/tagged.jpg` fixture, which
//! carries one Exif, one Iptc and one Xmp tag (see the crate history for how it
//! was produced).

use rahmen_exiv2::Metadata;

fn fixture() -> Metadata {
    Metadata::new_from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/tagged.jpg"))
        .expect("fixture should load")
}

#[test]
fn reads_exif_iptc_xmp() {
    let m = fixture();
    assert_eq!(
        m.get_tag_interpreted_string("Exif.Photo.DateTimeOriginal")
            .unwrap(),
        "2026:06:11 14:24:00"
    );
    assert_eq!(
        m.get_tag_interpreted_string("Iptc.Application2.City")
            .unwrap(),
        "Zurich"
    );
    assert_eq!(
        m.get_tag_interpreted_string("Xmp.dc.creator").unwrap(),
        "Moritz Hoffmann"
    );
}

#[test]
fn missing_tag_is_error() {
    let m = fixture();
    assert!(
        m.get_tag_interpreted_string("Iptc.Application2.CountryName")
            .is_err()
    );
}

#[test]
fn unknown_family_is_error() {
    let m = fixture();
    assert!(m.get_tag_interpreted_string("Bogus.Foo.Bar").is_err());
}

#[test]
fn missing_file_is_error() {
    assert!(Metadata::new_from_path("/nonexistent/path/to/image.jpg").is_err());
}
