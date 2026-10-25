FROM rust:1-trixie

ENV CARGO_INCREMENTAL=0

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config dpkg-dev libexiv2-dev libfontconfig1-dev python3-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-deb --version 3.8.0 --locked

WORKDIR /work
