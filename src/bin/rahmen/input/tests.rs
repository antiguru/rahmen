use super::select;
use image::{ImageFormat, RgbImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rahmen-input-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn image(&self, name: &str, format: ImageFormat) -> PathBuf {
        let path = self.0.join(name);
        RgbImage::new(2, 2).save_with_format(&path, format).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn assert_single_image(path: &Path) {
    let mut provider = select(path.to_str().unwrap(), Cursor::new(Vec::new())).unwrap();
    assert_eq!(provider.next_image().unwrap(), Some(path.to_owned()));
    assert_eq!(provider.next_image().unwrap(), None);
    assert_eq!(provider.next_image().unwrap(), None);
}

#[test]
fn direct_jpeg_returns_its_path_once() {
    let fixture = Fixture::new();
    assert_single_image(&fixture.image("photo.jpg", ImageFormat::Jpeg));
}

#[test]
fn direct_png_returns_its_path_once() {
    let fixture = Fixture::new();
    assert_single_image(&fixture.image("photo.png", ImageFormat::Png));
}

#[test]
fn image_with_glob_characters_is_a_literal_path() {
    let fixture = Fixture::new();
    fixture.image("photo1x.png", ImageFormat::Png);
    assert_single_image(&fixture.image("photo[1]*?.png", ImageFormat::Png));
}

#[test]
fn image_detection_uses_contents() {
    let fixture = Fixture::new();
    assert_single_image(&fixture.image("photo.data", ImageFormat::Png));
}

#[test]
fn text_list_preserves_entries_and_exhaustion() {
    let fixture = Fixture::new();
    let path = fixture.0.join("images.jpg");
    std::fs::write(&path, "first.png\nsecond.jpg\n").unwrap();
    let mut provider = select(path.to_str().unwrap(), Cursor::new(Vec::new())).unwrap();
    assert_eq!(provider.next_image().unwrap(), Some("first.png".into()));
    assert_eq!(provider.next_image().unwrap(), Some("second.jpg".into()));
    assert_eq!(provider.next_image().unwrap(), None);
}

#[test]
fn stdin_preserves_entries_and_exhaustion() {
    let mut provider = select("-", Cursor::new(b"first.png\nsecond.jpg\n")).unwrap();
    assert_eq!(provider.next_image().unwrap(), Some("first.png".into()));
    assert_eq!(provider.next_image().unwrap(), Some("second.jpg".into()));
    assert_eq!(provider.next_image().unwrap(), None);
}

#[test]
fn pattern_expands_matching_paths() {
    let fixture = Fixture::new();
    let first = fixture.image("first.png", ImageFormat::Png);
    let second = fixture.image("second.png", ImageFormat::Png);
    fixture.image("excluded.jpg", ImageFormat::Jpeg);
    let pattern = fixture.0.join("*.png");
    let mut provider = select(pattern.to_str().unwrap(), Cursor::new(Vec::new())).unwrap();
    assert_eq!(provider.next_image().unwrap(), Some(first));
    assert_eq!(provider.next_image().unwrap(), Some(second));
    assert_eq!(provider.next_image().unwrap(), None);
}

#[cfg(target_os = "linux")]
#[test]
fn named_pipe_preserves_list_entries_and_exhaustion() {
    let fixture = Fixture::new();
    let path = fixture.0.join("images.fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        std::fs::write(writer_path, "first.png\nsecond.jpg\n").unwrap();
    });
    let result = select(path.to_str().unwrap(), Cursor::new(Vec::new()));
    writer.join().unwrap();
    let mut provider = result.unwrap();
    assert_eq!(provider.next_image().unwrap(), Some("first.png".into()));
    assert_eq!(provider.next_image().unwrap(), Some("second.jpg".into()));
    assert_eq!(provider.next_image().unwrap(), None);
}
