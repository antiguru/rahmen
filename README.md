# Rahmen: A lightweight image presenter

Rah·men
/Ráhmen/
German: frame

Rahmen is a lightweight tool to present images while consuming little resources.
It takes a list of files or a pattern, and periodically shows the next image.

Rahmen is designed to run on low-power devices, such as the Raspberry Pi 1. While it is not heaily optimized to consume
little resources, some effort has been put into loading, pre-processing and rendering images.

## Building

`cargo build --bin rahmen`

## Cross-compiling for the Raspberry Pi 1

The first-generation Raspberry Pi had a BCM2835, supporting the ARMv6 instruction set. Current ARM compilers on Debian
only support armv7. For this reason, we need to use a different toolchain, for example the one provided specifically for
the Raspberry Pi on [github.com/raspberrypi/tools](https://github.com/raspberrypi/tools).

`cargo build --target arm-unknown-linux-gnueabihf --bin rahmen --release`

## Minifb support

The minifb renders a window on X, which can be used for development.

Enable the optional feature `minifb`
