#!/usr/bin/env bash
# mise.toml and rust-toolchain.toml must pin the same Rust version.
set -euo pipefail

mise_version=$(sed -nE 's/^rust = \{ version = "([^"]+)".*/\1/p' mise.toml)
toolchain_version=$(sed -nE 's/^channel = "([^"]+)"/\1/p' rust-toolchain.toml)

if [[ -z "$mise_version" || "$mise_version" != "$toolchain_version" ]]; then
  echo "rust version mismatch: mise.toml=$mise_version rust-toolchain.toml=$toolchain_version" >&2
  exit 1
fi
