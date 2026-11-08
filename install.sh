#!/bin/sh
set -eu
if ! command -v cargo >/dev/null 2>&1; then
    echo "pulsar: cargo not found. Install Rust first: https://rustup.rs" >&2
    exit 1
fi
exec cargo install --git https://github.com/devansharora18/pulsar pulsar --force
