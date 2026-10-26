#include "src/shim.h"

#include <mutex>
#include <stdexcept>
#include <string>

namespace rahmen_exiv2 {

namespace {

std::mutex xmp_mutex;

void xmp_lock(void *, bool lock) {
  if (lock) {
    xmp_mutex.lock();
  } else {
    xmp_mutex.unlock();
  }
}

// exiv2 otherwise initializes the XMP toolkit on first decode, which is not
// thread-safe and crashes or deadlocks when threads open images concurrently.
// A function-local static runs the initialization exactly once, and the lock
// serializes later XMP namespace registration.
void initialize_xmp() {
  static const bool initialized = Exiv2::XmpParser::initialize(xmp_lock, nullptr);
  (void)initialized;
}

}  // namespace

std::unique_ptr<Image> open_image(rust::Str path) {
  initialize_xmp();
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
