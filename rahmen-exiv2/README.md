# rahmen-exiv2

A minimal, safe wrapper over [libexiv2](https://exiv2.org/), built for
[rahmen](https://github.com/antiguru/rahmen).

It exposes only the read-only image-metadata operations rahmen needs — opening an
image and reading a single Exif/Iptc/Xmp tag as an interpreted (human-readable)
string — through a small C++ shim bound with [`cxx`](https://cxx.rs/). The
unavoidable FFI `unsafe` is confined to this crate, so dependents can keep
`#![forbid(unsafe_code)]`.

## Requirements

- `libexiv2` development files (e.g. `libexiv2-dev` on Debian/Ubuntu); discovered
  at build time via `pkg-config`. exiv2 0.27 and 0.28 are both supported.
- A C++17 compiler.

## Example

```rust,no_run
use rahmen_exiv2::Metadata;

let meta = Metadata::new_from_path("image.jpg")?;
let when = meta.get_tag_interpreted_string("Exif.Photo.DateTimeOriginal")?;
println!("{when}");
# Ok::<(), rahmen_exiv2::Exiv2Error>(())
```

## License

GPL-3.0-only. See the repository's `LICENSE`.
