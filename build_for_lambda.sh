#!/usr/bin/env bash

set -euxo pipefail

docker pull clux/muslrust
docker run \
       -v cargo-cache:/root/.cargo/registry \
       -v "$PWD:/volume" \
       --rm -it clux/muslrust cargo build --release

cp ./target/x86_64-unknown-linux-musl/release/playlister bootstrap

zip -j lambda.zip ./bootstrap
