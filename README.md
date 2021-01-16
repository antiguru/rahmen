# Rahmen: A lightweight image presenter

Rah·men [[ˈʁaːmən]](https://de.wiktionary.org/wiki/Rahmen) German: frame

Rahmen is a lightweight tool to present an image slideshow while consuming little resources. It takes a list of files or
a pattern, and periodically shows the next image.

Below the image, some information gathered from the image's metadata will be shown.
Right now, this is location data, time and date (formatted to German m.d.yyyy, h:mm), and
the creator info (gathered from the copyright info set in the camera).
If the data is not found, nothing is displayed.
It's planned to make this feature configurable in the future.

Rahmen is designed to run on low-power devices, such as the Raspberry Pi 1. While it is not heavily optimized to consume
little resources, some effort has been put into loading, pre-processing and rendering images.

## Dependencies

Rahmen depends on various libraries, which should be available on most Linux distributions. Specifically, it needs:

* `libgexiv2-dev`

## Building

`cargo build --bin rahmen`

## Running

```shell
./rahmen --help`
Rahmen client

USAGE:
rahmen [OPTIONS] <input>

ARGS:
<input>

FLAGS:
-h, --help       Prints help information
-V, --version    Prints version information

OPTIONS:
--buffer_max_size <buffer_max_size>    [default: 16000000]
```

The buffer size (in Bytes) determines the downscaling of images. All images that are larger than the buffer size in
Bytes will be scaled down to the buffer size. This should be larger than your monitor to avoid scaling
artefacts/jaggies.

Rule of thumb: `long side of the monitor ^ 2 * 2`, e.g. for a 1600 * 1200 monitor: `1600 * 1600 * 2 = 5120000`.

(Images smaller than your monitor will be scaled up to the monitor size and will possibly appear blurred. Avoid them if
you don't like this.)

```shell
-d, --display <display>
Select the display provider [default: framebuffer] [possible values: framebuffer]
```

(If compiled with the FLTK option, the FLTK display provider will also be available, use `fltk` as value.)

```shell
        --font <font>
            [default: /usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf]
```

Rahmen tries to guess the location of the image by looking up the GPS coordinates (if any), and will display it below
the image in the given font. If the font is not found, the program exits. If you don't want to install lots of fonts,
just point this option to a TrueType font file.

```shell
    -o, --output <output>                      
    -t, --time <time>                          [default: 90]
```

The output points to the frame buffer to be used. Usually `/dev/fb0`.

The time (in seconds) defines the interval to change to the next slide. On the Raspberry Pi version 1, it takes several
seconds to scale larger images. If the time given is shorter than what it takes to display the image, no images will be
skipped, the image will be displayed to the next full second after it is fully loaded plus the time it takes to load the
next image. So on low-resource systems this should not be set too short, otherwise if the next image is very small, it
could lead to the image displaying for less than 1 second.

## Cross-compiling for the Raspberry Pi 1

Preparation:

1. Add the Rust toolchain:
   ```
   rustup target add arm-unknown-linux-gnueabihf
   ```

2. Setup the GCC toolchain. The first-generation Raspberry Pi had a BCM2835, supporting the ARMv6 instruction set.
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

The feature `fltk` is enabled by default. Pass `--no-default-features` to `cargo build` to disable.
