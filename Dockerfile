# syntax=docker/dockerfile:1

################
##### Builder
# Runs on the build machine's own architecture and cross-compiles for the
# target platform with cargo-zigbuild, so no emulation is needed for the build.
FROM --platform=$BUILDPLATFORM rust:slim AS builder

# Zig acts as the cross C compiler and linker (needed for ring, used by rustls)
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3-pip \
 && rm -rf /var/lib/apt/lists/* \
 && pip3 install --no-cache-dir --break-system-packages \
      ziglang==0.17.0 cargo-zigbuild==0.23.4

# Map the Docker platform to the matching Rust (static musl) target
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
      linux/amd64)  echo x86_64-unknown-linux-musl ;; \
      linux/arm64)  echo aarch64-unknown-linux-musl ;; \
      linux/arm/v7) echo armv7-unknown-linux-musleabihf ;; \
      linux/arm/v6) echo arm-unknown-linux-musleabihf ;; \
      linux/riscv64) echo riscv64gc-unknown-linux-musl ;; \
      *) echo "Unsupported platform: $TARGETPLATFORM" >&2; exit 1 ;; \
    esac > /rust-target \
 && rustup target add "$(cat /rust-target)"

WORKDIR /usr/src/vbus2influx

# Build the dependencies first, so they stay cached unless Cargo.toml or
# Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
 && echo 'fn main() {}' > src/main.rs \
 && cargo zigbuild --locked --release --target "$(cat /rust-target)" \
 && rm -rf src

# Now copy in the real sources and build the application
COPY src ./src
RUN touch src/main.rs \
 && cargo zigbuild --locked --release --target "$(cat /rust-target)" \
 && cp "target/$(cat /rust-target)/release/vbus2influx" /vbus2influx

################
##### Runtime
FROM alpine:latest AS runtime

# Copy application binary from builder image
COPY --from=builder /vbus2influx /usr/local/bin/vbus2influx

# Run as nonroot: uid 100, group dialout (gid 20) for access to the serial port.
# Numeric ids, so this stage has no RUN step and needs no emulation.
USER 100:20

# Run the application
CMD ["vbus2influx"]
