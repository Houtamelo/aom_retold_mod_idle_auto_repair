#!/usr/bin/env bash
# Dependencies needed inside the claude-sandbox container for this project.
# This script runs as root on every container creation.
set -euo pipefail

apt-get update -y
apt-get install -y build-essential gcc curl openjdk-21-jdk-headless

# Install rustup + nightly toolchain system-wide so the `rust-toolchain.toml`
# in tools/xs-language-server/ can activate nightly for `cargo build` /
# `cargo test` (the version installed by `apt install cargo` is rustc 1.85,
# which can't compile scraper 0.27 / fluent-uri — they use let-chains,
# stabilized in Rust 1.88+).
export RUSTUP_HOME=/usr/local/rustup
export CARGO_HOME=/usr/local/cargo
mkdir -p "$RUSTUP_HOME" "$CARGO_HOME"
chmod 755 "$RUSTUP_HOME" "$CARGO_HOME"
curl -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain nightly --profile minimal
# Make the toolchain + registry world-readable so the non-root `claude`
# user can use it without sudo.
chmod -R a+rx "$RUSTUP_HOME" "$CARGO_HOME"
# Symlink into /usr/local/bin so `cargo`/`rustup` are on the default PATH
# (ahead of /usr/bin where the apt cargo binary lives, if it ever gets
# reinstalled).
ln -sf /usr/local/cargo/bin/cargo  /usr/local/bin/cargo
ln -sf /usr/local/cargo/bin/rustup /usr/local/bin/rustup
ln -sf /usr/local/cargo/bin/rustc  /usr/local/bin/rustc