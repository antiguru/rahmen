#pragma once

// C++ shim exposing a minimal, read-only slice of libexiv2 to Rust via cxx.
// Only what rahmen needs: open an image and read a tag as an interpreted
// (human-readable) string across the Exif, Iptc and Xmp families.

#include <memory>

#include <exiv2/exiv2.hpp>

#include "rust/cxx.h"

namespace rahmen_exiv2 {

// Opaque-to-Rust handle owning a loaded exiv2 image with its metadata read.
// Defined fully here so cxx can instantiate the std::unique_ptr<Image> glue.
//
// Hold the image as a plain std::unique_ptr rather than Exiv2::Image::UniquePtr:
// the latter typedef only exists since exiv2 0.28 (it was AutoPtr before), and
// both spellings are std::unique_ptr<Exiv2::Image>, so this works across
// versions and accepts the ImageFactory::open() return value either way.
class Image {
 public:
  std::unique_ptr<Exiv2::Image> img;
};

// Open the file at `path` and read its metadata. Throws Exiv2::Error (a
// std::exception) on failure, which cxx converts into a Rust Err.
std::unique_ptr<Image> open_image(rust::Str path);

// Return the interpreted string for `key` (e.g. "Exif.Photo.DateTimeOriginal",
// "Iptc.Application2.City", "Xmp.dc.creator"). Throws when the tag is absent,
// the key is malformed, or the family prefix is unknown.
rust::String tag_interpreted(const Image &image, rust::Str key);

}  // namespace rahmen_exiv2
