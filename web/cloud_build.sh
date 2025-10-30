#!/bin/bash
set -ex

# if the BUILD_FOR_CLOUD variable is not true, exit the script
if [ "$BUILD_FOR_CLOUD" != "true" ]; then
  echo "BUILD_FOR_CLOUD is not set to true, exiting script."
  exit 1
fi

# Install Rust and Cargo
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"

# Build WASM module
cd ../wasm
wasm-pack build --release --target web
