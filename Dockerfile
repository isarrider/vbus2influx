################
##### Builder
FROM rust:slim AS builder

WORKDIR /usr/src

# Create blank project
RUN USER=root cargo new medium-rust-dockerize

# Set the working directory
WORKDIR /usr/src/medium-rust-dockerize

# We want dependencies cached, so copy those first.
COPY Cargo.toml Cargo.lock /usr/src/medium-rust-dockerize/

## Install target platform (Cross-Compilation) --> Needed for Alpine
RUN apt update && apt upgrade -y
RUN apt install -y g++-aarch64-linux-gnu libc6-dev-arm64-cross

RUN rustup target add aarch64-unknown-linux-musl
RUN rustup toolchain install stable-aarch64-unknown-linux-musl --force-non-host

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_musl=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_musl=aarch64-linux-gnu-g++

# This is a dummy build to get the dependencies cached.
RUN cargo update
RUN cargo build --target aarch64-unknown-linux-musl --release

# Now copy in the rest of the sources
COPY src /usr/src/medium-rust-dockerize/src/

## Touch main.rs to prevent cached release build
RUN touch /usr/src/medium-rust-dockerize/src/main.rs

# This is the actual application build.
RUN cargo build --target aarch64-unknown-linux-musl --release

################
##### Runtime
FROM alpine:latest AS runtime 

# Copy application binary from builder image
COPY --from=builder /usr/src/medium-rust-dockerize/target/aarch64-unknown-linux-musl/release/vbus2influx /usr/local/bin/

# Run as nonroot
RUN adduser -S vbus2inf -G dialout
USER vbus2inf

# Run the application
CMD ["vbus2influx"]
