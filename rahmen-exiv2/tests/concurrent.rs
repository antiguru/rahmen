//! Opening images from several threads at once must not race on exiv2's
//! one-time XMP toolkit initialization. This file is its own test binary, so
//! these threads are the first exiv2 users in the process.

use std::sync::{Arc, Barrier};

use rahmen_exiv2::Metadata;

#[test]
fn concurrent_first_use_reads_metadata() {
    const THREADS: usize = 8;
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let metadata = Metadata::new_from_path(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/testdata/tagged.jpg"
                ))
                .expect("fixture should load");
                metadata
                    .get_tag_interpreted_string("Xmp.dc.creator")
                    .unwrap()
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), "Moritz Hoffmann");
    }
}
