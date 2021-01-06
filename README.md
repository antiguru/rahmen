# Rahmen: A lightweight image presenter

Rah·men [[ˈʁaːmən]](https://de.wiktionary.org/wiki/Rahmen) German: frame

Rahmen is a lightweight tool to present images while consuming little resources. It takes a list of files or a pattern,
and periodically shows the next image.

Rahmen is designed to run on low-power devices, such as the Raspberry Pi 1. While it is not heavily optimized to consume
little resources, some effort has been put into loading, pre-processing and rendering images.

## Dependencies

Rahmen depends on various libraries, which should be available on most Linux distributions. Specifically, it needs:

* `libgexiv2-dev`

## Building

`cargo build --bin rahmen`

## Cross-compiling for the Raspberry Pi 1

Preparation:

1. Add the Rust toolchain:
   ```
   rustup target add arm-unknown-linux-gnueabihf
   ```

2. Setup the GCC toolchanin. The first-generation Raspberry Pi had a BCM2835, supporting the ARMv6 instruction set.
   Current ARM compilers on Debian only support armv7. For this reason, we need to use a different toolchain, for
   example the one provided specifically for the Raspberry Pi
   on [github.com/raspberrypi/tools](https://github.com/raspberrypi/tools). Export its `bin` directory to the local
   path.

   Tell Cargo to use the correct cross-compiler by adding the following content to `~/.cargo/config.toml`
   or `.cargo/config.toml` in the project directory:

   ```toml
   [target.arm-unknown-linux-gnueabihf]
   linker = "arm-linux-gnueabihf-gcc"
   ar = "arm-linux-gnueabihf-ar"
   ```

   Add the toolchain to the current environment by adding it to the path:

   ```shell
   git clone https://github.com/raspberrypi/tools
   export PATH="$PATH:$(pwd)/tools/arm-bcm2708/arm-linux-gnueabihf/bin/"
   ```

3. Add the `armhf` target to Debian and install a dependency:

   ```shell
   dpkg --add-architecture armhf
   apt install libgexiv2-dev:armhf libfontconfig1-dev:armhf
   ```

Now, issue the following command to cross-compile the binary.

```shell
cargo build --target arm-unknown-linux-gnueabihf --bin rahmen \
  --release --no-default-features
```

We pass `--no-default-features` to disable the FLTK display support.

If the build fails in `font-kit` with a message that the C compiler cannot produce executables, try to force CC and AR
using the following command line:

```shell
AR=arm-linux-gnueabihf-ar CC=arm-linux-gnueabihf-gcc cargo build \
  --target arm-unknown-linux-gnueabihf --bin rahmen \
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
