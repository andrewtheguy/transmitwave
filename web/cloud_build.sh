#!/bin/bash
set -ex

# if the BUILD_FOR_CLOUD variable is not true, exit the script
if [ "$BUILD_FOR_CLOUD" != "true" ]; then
  echo "BUILD_FOR_CLOUD is not set to true, exiting script."
  exit 1
fi

# Install Rust and Cargo if not already installed
if ! command -v cargo &> /dev/null; then
  echo "Cargo not found, installing Rust..."
  curl https://sh.rustup.rs -sSf | sh -s -- -y
  source "$HOME/.cargo/env"
else
  echo "Cargo already installed, skipping Rust installation."
fi

# Build WASM module
cd ../wasm
wasm-pack build --release --target web
