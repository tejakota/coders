#!/usr/bin/env bash
# Installs coders on Linux or macOS. Uses a prebuilt binary from dist/ when
# one matches the host OS/arch; otherwise falls back to `cargo build`
# (requires Rust: https://rustup.rs).
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="coders"
INSTALL_DIR="${CODERS_INSTALL_DIR:-$HOME/.local/bin}"

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
  Linux) TARGET="linux-${ARCH}" ;;
  Darwin) TARGET="macos-${ARCH}" ;;
  *) TARGET="unknown" ;;
esac

PREBUILT="$REPO_DIR/dist/$TARGET/${BIN_NAME}"

mkdir -p "$INSTALL_DIR"

if [ -f "$PREBUILT" ]; then
  echo "Using prebuilt binary for $TARGET"
  cp "$PREBUILT" "$INSTALL_DIR/${BIN_NAME}"
else
  echo "No prebuilt binary for $TARGET, building from source..."
  if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo/rustc not found. Install Rust first: https://rustup.rs" >&2
    exit 1
  fi
  cargo build --release --manifest-path "$REPO_DIR/Cargo.toml" -p coders-cli
  cp "$REPO_DIR/target/release/${BIN_NAME}" "$INSTALL_DIR/${BIN_NAME}"
fi

chmod +x "$INSTALL_DIR/${BIN_NAME}"
echo "Installed to $INSTALL_DIR/${BIN_NAME}"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo
    echo "warning: $INSTALL_DIR is not on your PATH."
    echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, ...):"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo
"$INSTALL_DIR/${BIN_NAME}" init
