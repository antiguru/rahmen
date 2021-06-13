# Rahmen: A lightweight image presenter

Rah·men [[ˈʁaːmən]](https://de.wiktionary.org/wiki/Rahmen) German: frame

Rahmen is a lightweight tool to present a slideshow of one or more JPEG images while consuming little resources. It
takes a list of files or a pattern, and periodically shows the next image. It's work in progress, but the code found
here should work.

Below the image, some information gathered from the image's metadata will be shown. This feature has to be configured in
the `rahmen.toml` configuration file. There, you can enter one or more metadata tags name known to
the [exiv2](https://exiv2.org/metadata.html) library to be displayed in the information line.

All the information items will be displayed on one line, with `", "` as (default, but read on)
separator. If this line is too long for the screen, some text will overflow and not be shown at the end of the line. Use
a wider screen or a narrower font to reduce the probability that this will happen. The font size is configurable using
the `--font_size` argument or the configuration file.

Because the data derived from the image's metadata tags is often difficult to read, ``rahmen``
offers a wide range of tools to process the raw metadata.

Rahmen is not a soup.

### Basic metadata processing

#### Case conversion

As first step of the metadata processing chain, it is possible to convert the
case. [See below, where this setting is discussed in the context of the configuration file](#changing-the-case).

#### Regular expressions for individual metadata

For each metadata entry, it's further possible to define pairs of
[regular expressions and replacements](https://docs.rs/regex/) that will be applied to the metadata for each individual
tag. Multiple regular expressions and replacements will be applied in the given
order. [More details will be discussed in the context of the configuration file](#metadata).

After this, the result will either be handed over to the [final processing step](#final-processing-step), or, before
that, undergo the advanced processing step.

### Advanced processing using Python code

It is possible to include a Python script to process the string produced by the previous steps.

Add the following to the configuration file to call a script named ``postprocess.py`` in the same directory as
``rahmen``:

```toml
py_postprocess = "postprocess"
py_path = ["."]
```

The Python code will be loaded once and executed for each new image. Be aware that this means that variables will be
kept between images.

This Python code gets the line string and the separator string as positional arguments
(in the order given here).

The main function of the Python code has to be named ``export``. It is required to return a callable taking the line
string and the separator string and returning a list of strings, representing the processed metadata items.

Other than that, it is possible to flexibly process the incoming string and build the output accordingly. We have used a
positional approach in our processing, which identifies a certain match in the metadata items list and then manipulates
items at a position relative to this match (see the ``postprocess.py`` example we have published).

More information can be
found [where this is discussed in the context of the configuration file](#advanced-metadata-processing-using-python).

### Final processing step

Empty results for metadata tags will be dropped.

Multiple occurrences of the same data will be reduced to one. It's possible to change this behaviour using
the ``uniquify`` entry in the configuration file.

After this, the items will be joined using the default or configured separator.

It's also possible to construct the metadata output line yourself in Python. You will have to return it as a list of one
item, which will effectively prevent the final processing step.

### Resource consumption

Rahmen is designed to run on low-power devices, such as the Raspberry Pi 1 (in fact it was specifically created to build
a digital picture frame out of an old monitor and an old Raspberry Pi 1 due to the lack of capable software). While it
is not heavily optimized to consume little resources, some effort has been put into loading, pre-processing and
rendering images.

## Dependencies

Rahmen depends on various libraries, which should be available on most Linux distributions. Specifically, it needs:

* `libgexiv2-dev`

Rahmen will run if there's no configuration file, but will use minimal defaults (see below), and no metadata will be
shown.

## Building

`cargo build --bin rahmen`

## Running

```shell
./rahmen --help
Rahmen client

USAGE:
rahmen [OPTIONS] <input>

ARGS:
<input>
```

The input can either be a filename, a file pattern (`IMGP4*.jpg`), or a file containing a list of file names. If you'd
like to have a random image order, use the `find` and `shuf` commands to create a file list
(see the provided shell script for an example).

```shell
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
        --font_size <font_size>                
```

The font size to use in px.

```shell
        --font <font>
            [default: /usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf]
```

Rahmen will display information from the image's metadata (see above) in a single line below the image in the given
font. If the font is not found, the program exits. If you don't want to install lots of fonts, just point this option to
a TrueType font file.

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

```shell
    -c, --config <config file>
```

Indicate the name and path of the configuration file to read. This takes precedence.

### Shell script

We have added a basic bash script (in the ``utils`` directory) which creates a random image list from a given folder and
starts ``rahmen``. You could configure the machine to use autologin and call this script from the end of your
``.bashrc`` to start a ``rahmen`` slideshow automatically after the system has started up. Of course, be sure to change
to folders and paths to match your setup.

## Configuration File (default name: rahmen.toml)

Rahmen will run without configuration file using the default settings given above, but no metadata will be displayed
below the image. To show metadata, a configuration file must be used; an example file (`rahmen.toml`) can be found among
the sources.

The default lookup paths for the configuration file are either `~/.config/rahmen.toml` or `/etc/rahmen.toml`. If both
are present, the file in the home directory takes precedence.

The configuration file has to be written in TOML and takes the following instructions:

```toml
font_size = 24
delay = 90
```

Values for font size (px) and the interval before the next image (in s, see above, --time parameter). If command line
parameters are given, they take precedence over the values in this file.

### Displaying the time

Rahmen can optionally display the current time as part of the status line. To enable showing the current time, add the
following option to the configuration file:

```toml
display_time = true
```

By default, it uses a pattern of "%H:%M:%S" ("14:28:22"). The pattern can be replaced by a custom pattern in the
configuration file:

```toml
time_format = "%H:%M"
```

For a reference of supported format specifiers,
see [Chrono's documentation](https://docs.rs/chrono/0.4.19/chrono/#formatting-and-parsing).

### Metadata

```toml
[[status_line]]
exif_tags = ["Iptc.Application2.ObjectName"]
```

Each `[[status_line]]` entry can contain one

`exif-tags = ["Some.Tag.Known.to.Exiv2"]`

entry, and optionally, one

`replace = [{ regex = 'regex1', replace = 'repl1' }, { regex = '...', replace = '...' }, ... ]`

entry, where one or more regular expressions and the replacements for the part they match could be supplied.

[The regular expressions and replacements are documented here.](https://docs.rs/regex/)

The regular expression operations will be applied one after the other in the given order. For long expressions, or if
you wish to comment them, this could also be written like

```toml
[[status_line.replace]]
# get named fields of the date
regex = '(?P<y>\d{4})[-:]0*(?P<M>\d+)[-:]0*(?P<d>\d+)\s+0*(?P<h>\d+:)0*(?P<m>\d+):(?P<s>\d{2})'
## with time
## replace = '$d.$M.$y, $h$m'
# without time
replace = '$d.$M.$y'
```

The [tag names that can be used are listed on the this exiv2 webpage](https://exiv2.org/metadata.html). This doesn't
mean that all these are actually present in your image file. Use [exiftool](https://exiftool.org/)
to show you the metadata in your file and see what is available.

##### Changing the case

Because some of the tags we used were in ALL-CAPS which doesn't look nice, we offer case conversions that you can apply
to the data _before_ they are processed by the regular expressions described above. The order in the configuration file
doesn't matter here. The [available case strings can be found here.](https://github.com/rutrum/convert-case#cases)
See the following example. The previous method of setting the `capitalize` variable is also still available.

```toml
# convert input from UPPER CASE to Title Case 
case_conversion = { from = 'Upper', to = 'Title' }
# this does the same, but only from UPPER to Title Case
capitalize = true
```

##### Custom separator

```toml
separator = "|"
```

That way it's possible to set a custom separator
(the default is `", "`).

This ends the basic processing of the metadata. The information line produced by the rules given will be handed over to
the [final processing step](#final-processing-step), unless you decide to go further and process it using Python, which
is described next, and after that, it will be shown below the image.

#### Advanced metadata processing using Python

It's possible to use Python code that receives a list of the metadata tags, after they have been processed using all the
individual and per-line regex definitions, and process them there.

Add the following to the configuration file to call a script named ``postprocess.py`` in the same directory as
``rahmen`` (the extension ``.py`` being quietly assumed):

```toml
py_postprocess = "postprocess"
py_path = ["."]
```

The main function of the Python code has to be named ``export``. It is required to return a callable taking the line
string and the separator string and returning a list of strings, representing the processed metadata items.

``py_path`` defines where to look for the Python script. The value given here is prepended to
the [standard Python search path](https://docs.python.org/3/library/sys.html#sys.path), although the default search path
described there does not apply, because no regular script is called. To search the current directory, use ``"."``. Note:
if you omit this entry and your script can be found neither via the ``$PYTHONPATH`` environment nor as a system module,
it will not be possible to find the script, and the program will abort.

##### Example script and test suite

We provide an example script (``postprocess.py``) where some processing is done for certain filters. To check the
processing, we used ``pytest``. We provide a test script (``test.py``) matching the processing rules in the example
script. On our Debian system, invoking it with ``pytest-3 test.py`` runs the tests. It is strongly recommended to create
a test for every processing rule you create to ensure it is properly working.

After the Python code has returned the list of processed entries, they will be handed over to
the [final processing step](#final-processing-step).

##### How to get the tags

The human-readable location tags we use in the enclosed `rahmen.toml` example file are based on the information you can
tell Adobe Lightroom to add when it finds a GPS location in the image metadata.

## Bugs, Issues, Desiderata

- Allow reacting to configuration file changes while running.
- Allow for testing the whole text conversion chain, not only the Python part.
- The font rendering is not really beautiful and sometimes, glyphs overlap.
- The overflowing text is just not displayed.
- The text bar might look better centered.

## Compiling for the Raspberry Pi 1

Because some of the include C libraries wouldn't readily cross-compile, at this time we do not know of a way to
cross-compile for the Raspberry Pi 1. Currently, we build Rahmen on a Raspberry Pi 4, and cross-compile to ARMv6 on this
platform- it works, although it's still a hack. At least compilation times are less than "a night."

Of course, building natively on a Pi 1 also works, but the term "nightly build" will have to be taken literally,
especially for the first run. Small changes to this source code only without the need to rebuild stuff depending on it
(no new dependencies added) will take approximately 90...100 minutes.

### Compiling on the Raspberry Pi 4

Using `cargo deb` to build a package for the Raspberry Pi 1 (armv6hf):

```shell
env \
  PYO3_CROSS_LIB_DIR="/usr/lib" \
  CFLAGS="-march=armv6" \
  PKG_CONFIG_LIBDIR=/usr/lib/arm-linux-gnueabihf/pkgconfig/ \
  FREETYPE_CFLAGS="-I/usr/include/freetype2" \
  FREETYPE_LIBS="-L/usr/lib -lfreetype" \
  CC=gcc \
  AR=ar \
  PKG_CONFIG_ALLOW_CROSS=1 \
  cargo deb --target arm-unknown-linux-gnueabihf
```

### 4k on the Raspberry Pi 1

The Raspberry Pi 1 supports 4k resolution at reduced frame rates. The following configuration works on a screen we have
at hands. It does not require overclocking, but limits the refresh rate to 15 frames per second, which seems to be fine
when displaying mostly static content.

```
disable_overscan=1
hdmi_ignore_edid=0xa5000080
hdmi_cvt=3840 2160 15
framebuffer_width=3840
framebuffer_height=2160
hdmi_group=2
hdmi_mode=87
hdmi_pixel_freq_limit=400000000
max_framebuffer_width=3840
max_framebuffer_height=2160
```

### Previous attempts to cross-compile

Cross-compilation is a mess. The instructions below worked until we decided to include a dependency on `libgexiv2` to
extract image metadata. This has some trouble cross-compiling and eventually, we decided not to spend more time on it.

Preparation:

1. Add the Rust toolchain:
   ```
   rustup target add arm-unknown-linux-gnueabihf
   ```

2. Set up the GCC toolchain. The first-generation Raspberry Pi had a BCM2835, supporting the ARMv6 instruction set.
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
  --release
```

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

The feature `fltk` is not enabled by default. Pass `--features fltk` to `cargo build` to enable.

## License

Rahmen is licensed under the terms of the GNU General Public License version 3. See the [LICENSE](LICENSE) file for a
copy.
