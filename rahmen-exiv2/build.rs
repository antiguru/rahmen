//! Build script: compile the libexiv2 C++ shim and link against exiv2.

fn main() {
    // Locate libexiv2 via pkg-config; this also emits the link flags.
    let exiv2 = pkg_config::Config::new()
        .probe("exiv2")
        .expect("libexiv2-dev not found (pkg-config could not find `exiv2`)");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut build = cxx_build::bridge("src/lib.rs");
    build
        .file("src/shim.cc")
        .std("c++17")
        // Let `include!("src/shim.h")` and shim.cc resolve relative to the crate.
        .include(&manifest_dir);
    for path in &exiv2.include_paths {
        build.include(path);
    }
    build.compile("rahmen_exiv2");

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/shim.cc");
    println!("cargo:rerun-if-changed=src/shim.h");
}
