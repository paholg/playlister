#!/usr/bin/env bash

docker pull clux/muslrust
docker run \
       -v cargo-cache:/root/.cargo/registry \
       -v "$PWD:/volume" \
       --rm -it clux/muslrust cargo build --release

zip -j rust.zip ./target/x86_64-unknown-linux-musl/release/bootstrap
