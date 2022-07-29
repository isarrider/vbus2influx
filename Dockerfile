# syntax=docker/dockerfile:1.0-experimental

############################
# STEP 1 build executable binary
############################
FROM rust:latest as build

# create a new empty shell project
RUN USER=root cargo new --bin vbus2influx
WORKDIR /vbus2influx

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/vbus2influx*
RUN cargo build --release

#############################
## STEP 2 build a small image
#############################

FROM debian:bullseye-slim

# Copy our static executable
COPY --from=build /vbus2influx/target/release/vbus2influx /usr/local/bin/vbus2influx

RUN useradd -M -G dialout vbus2influx
USER vbus2influx

CMD ["vbus2influx"]