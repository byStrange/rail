#!/usr/bin/env sh
# Build rail in release mode and install it to ~/.local/bin.
set -eu
cd "$(dirname "$0")"
cargo build --release
install -Dm755 target/release/rail "$HOME/.local/bin/rail"
echo "installed: $HOME/.local/bin/rail"
