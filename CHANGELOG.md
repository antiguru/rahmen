# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0](https://github.com/antiguru/rahmen/compare/rahmen-v0.4.0...rahmen-v0.5.0) - 2026-11-20

### Changed

* The framebuffer display writes pixels directly in the device's layout, derived from the framebuffer's channel bit fields instead of assuming BGRA, and converts images row by row without a screen-sized shadow buffer. Placements outside the screen are clipped instead of failing. ([#68](https://github.com/antiguru/rahmen/pull/68))
* The background loader prepares images for the framebuffer: JPEG images are decoded straight into its pixel layout and resized to the image area, so the display only copies rows. ([#69](https://github.com/antiguru/rahmen/pull/69))
* The default `--buffer_max_size` rises from 16 MB to 24 MB, because JPEG images decoded for the framebuffer count 4 bytes per pixel. ([#69](https://github.com/antiguru/rahmen/pull/69))
* **Breaking:** Images travel through the dataflow and to displays as `Picture`, which holds either a `DynamicImage` or pixels in a display's native layout. This changes `Display::render`, `Configuration::Splash`, `ImageStream` and `ImagePosStream`. ([#69](https://github.com/antiguru/rahmen/pull/69))
* **Breaking:** Update timely to 0.32. `FormatText` and `ResizeImage` now require totally ordered timestamps. ([#72](https://github.com/antiguru/rahmen/pull/72))
* Update harfrust to 0.16 and log to 0.4.35. ([#73](https://github.com/antiguru/rahmen/pull/73), [#71](https://github.com/antiguru/rahmen/pull/71))

### Fixed

* Build with Rust 1.100, which warns about `package.homepage` duplicating `package.repository`. CI and release-plz pin their Rust versions. ([#74](https://github.com/antiguru/rahmen/pull/74), [#75](https://github.com/antiguru/rahmen/pull/75))

## [0.4.0](https://github.com/antiguru/rahmen/compare/rahmen-v0.3.0...rahmen-v0.4.0) - 2026-10-26

### Added

* Shape text with HarfBuzz (`harfrust`), applying the font's ligatures such as `ff` and `fi`, kerning, and mark positioning. Glyphs are spaced by the font's own advances instead of an extra 5%, and characters missing from the font render as its placeholder glyph instead of being skipped. ([#63](https://github.com/antiguru/rahmen/pull/63))

### Changed

* **Breaking:** `FontRenderer` no longer implements `UnwindSafe` or `RefUnwindSafe`. ([#63](https://github.com/antiguru/rahmen/pull/63))

### Fixed

* Attach Debian packages to a draft release and publish it once they are uploaded, so releases succeed with immutable releases enabled. ([#62](https://github.com/antiguru/rahmen/pull/62))

## [0.3.0](https://github.com/antiguru/rahmen/compare/rahmen-v0.2.0...rahmen-v0.3.0) - 2026-10-25

### Added

* Prefetch the next image on a background thread while the UI continues updating. A single slot holds either an in-flight load or a ready image, and a clock symbol after the timestamp indicates loading. ([#59](https://github.com/antiguru/rahmen/pull/59))
* Accept a JPEG or PNG file directly as slideshow input. ([#56](https://github.com/antiguru/rahmen/pull/56))
* Build Debian Trixie armhf packages for Raspberry Pi Zero 2 in CI and publish packages with SHA256 checksums on matching version tags. ([#48](https://github.com/antiguru/rahmen/pull/48))

### Changed

* **Breaking:** Update to Timely 0.31. The public `FormatText` and `ResizeImage` traits now take a lifetime parameter. ([#37](https://github.com/antiguru/rahmen/pull/37))
* Require Rust 1.88 or newer. ([#51](https://github.com/antiguru/rahmen/pull/51))
* Select a system sans-serif font by default while preserving explicit `--font` overrides. ([#58](https://github.com/antiguru/rahmen/pull/58))
* Update dependencies, including Python bindings, the window backend, metadata bindings, and text processing libraries. ([#50](https://github.com/antiguru/rahmen/pull/50), [#52](https://github.com/antiguru/rahmen/pull/52))
* Automate releases with independent package versioning. ([#28](https://github.com/antiguru/rahmen/pull/28), [#49](https://github.com/antiguru/rahmen/pull/49))

### Fixed

* Enforce `--buffer_max_size` as a decoded-image byte budget, downscale JPEGs within decoder limits, and skip images that cannot fit. ([#55](https://github.com/antiguru/rahmen/pull/55))
* Respect framebuffer stride and virtual offsets when writing pixels. ([#54](https://github.com/antiguru/rahmen/pull/54))
* Keep rendering and blanking within small or empty viewports, retaining content for later resizing. ([#57](https://github.com/antiguru/rahmen/pull/57))
* Build and render fonts correctly on macOS, preserving previously rendered glyphs and lines. ([#58](https://github.com/antiguru/rahmen/pull/58))
* Load packaged defaults without requiring the optional Python postprocessor. ([#53](https://github.com/antiguru/rahmen/pull/53))
* Preserve the previous slideshow playlist when regeneration fails, retry failed player starts with a delay, and include JPEG and PNG extensions case-insensitively. ([#60](https://github.com/antiguru/rahmen/pull/60))
* Validate ISO and legacy date formats in the example Python postprocessor, apply only the earliest matching timespan, and preserve unrelated metadata when fields are missing. ([#61](https://github.com/antiguru/rahmen/pull/61))
