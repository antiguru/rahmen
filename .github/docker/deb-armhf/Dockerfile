FROM rust:1-trixie

ENV CARGO_INCREMENTAL=0

# Cross-compile on the host architecture against Debian's armhf libraries
# instead of emulating an armhf toolchain.
RUN dpkg --add-architecture armhf \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config dpkg-dev gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf \
        libexiv2-dev:armhf libfontconfig1-dev:armhf libpython3-dev:armhf \
        fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add armv7-unknown-linux-gnueabihf \
    && cargo install cargo-deb --version 3.8.0 --locked

# Both architectures install Python sysconfigdata in one directory, so name the armhf one.
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_LIBDIR=/usr/lib/arm-linux-gnueabihf/pkgconfig:/usr/share/pkgconfig \
    PYO3_CROSS_LIB_DIR=/usr/lib/python3.13 \
    _PYTHON_SYSCONFIGDATA_NAME=_sysconfigdata__linux_arm-linux-gnueabihf

WORKDIR /work
