# Rahmen: A lightweight image presenter

Rah·men /Ráhmen/ German: frame

Rahmen is a lightweight tool to present images while consuming little resources. It takes a list of files or a pattern,
and periodically shows the next image.

Rahmen is designed to run on low-power devices, such as the Raspberry Pi 1. While it is not heaily optimized to consume
little resources, some effort has been put into loading, pre-processing and rendering images.

## Building

`cargo build --bin rahmen`

## Cross-compiling for the Raspberry Pi 1

Preparation:
1. Add the Rust toolchain:
   ```
   rustup target add arm-unknown-linux-gnueabihf
   ```

2. The first-generation Raspberry Pi had a BCM2835, supporting the ARMv6 instruction set. Current ARM compilers on
   Debian only support armv7. For this reason, we need to use a different toolchain, for example the one provided
   specifically for the Raspberry Pi on [github.com/raspberrypi/tools](https://github.com/raspberrypi/tools). Export
   its `bin` directory to the local path.

   Tell Cargo to use the correct cross-compiler by adding the following content to `~/.cargo/config.toml`
   or `.cargo/config.toml` in the project directory:

   ```yaml
   [target.arm-unknown-linux-gnueabihf]
   linker = "arm-linux-gnueabihf-gcc"
   ar = "arm-linux-gnueabihf-ar"
   ```

Now, issue the following command to cross-compile the binary.

`cargo build --target arm-unknown-linux-gnueabihf --bin rahmen --release --no-default-features`

We pass `--no-default-features` to disable the FLTK display support.

If the build fails in `font-kit` with a message that the C compiler cannot produce executables, try to force CC and AR
using the following command line:

```shell
AR=arm-linux-gnueabihf-ar CC=arm-linux-gnueabihf-gcc cargo build --target arm-unknown-linux-gnueabihf --bin rahmen \
  --release --no-default-features
  ```

Find the binary in `target/arm-unknown-linux-gnueabihf/release/rahmen`

### Stripping the binary

The binary includes debug symbols, which consume a rather large amount of space. The `strip` tool can be used to remove
the debug symbols from the binary:

`arm-linux-gnueabihf-strip target/arm-unknown-linux-gnueabihf/release/rahmen`

## FLTK support

The FLTK renders a window on various platforms, which can be used for development.

The feature `fltk` is enabled by default. Pass `--no-default-features` to disable.
