#include "src/shim.h"

#include <stdexcept>
#include <string>

namespace rahmen_exiv2 {

std::unique_ptr<Image> open_image(rust::Str path) {
  // exiv2 wants a std::string; rust::Str is not NUL-terminated.
  std::string p(path);
  // ImageFactory::open never returns null; it throws on failure.
  auto image = std::make_unique<Image>();
  image->img = Exiv2::ImageFactory::open(p);
  image->img->readMetadata();
  return image;
}

rust::String tag_interpreted(const Image &image, rust::Str key) {
  std::string k(key);
  // Dispatch on the metadata family encoded in the key prefix. exiv2 keeps
  // Exif, Iptc and Xmp in separate containers, each with its own key type.
  if (k.rfind("Exif.", 0) == 0) {
    auto &data = image.img->exifData();
    auto pos = data.findKey(Exiv2::ExifKey(k));
    if (pos == data.end()) {
      throw std::runtime_error("tag not found: " + k);
    }
    // Pass the surrounding ExifData so context-dependent tags interpret fully.
    return rust::String::lossy(pos->print(&data));
  }
  if (k.rfind("Iptc.", 0) == 0) {
    auto &data = image.img->iptcData();
    auto pos = data.findKey(Exiv2::IptcKey(k));
    if (pos == data.end()) {
      throw std::runtime_error("tag not found: " + k);
    }
    return rust::String::lossy(pos->print());
  }
  if (k.rfind("Xmp.", 0) == 0) {
    auto &data = image.img->xmpData();
    auto pos = data.findKey(Exiv2::XmpKey(k));
    if (pos == data.end()) {
      throw std::runtime_error("tag not found: " + k);
    }
    return rust::String::lossy(pos->print());
  }
  throw std::runtime_error("unknown tag family: " + k);
}

}  // namespace rahmen_exiv2
