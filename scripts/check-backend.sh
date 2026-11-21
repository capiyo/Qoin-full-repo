#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 is required" >&2; exit 1; }

echo "== Rust formatting =="
cargo fmt --all -- --check

echo "== Rust workspace tests =="
cargo test --workspace --locked

echo "== Rust clippy =="
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "== Python backend reference tests =="
python3 -m unittest discover -s backend/tests -v

if [[ "${QOIN_SKIP_DROGON:-0}" != "1" ]]; then
  command -v cmake >/dev/null || { echo "cmake is required for Drogon checks (or set QOIN_SKIP_DROGON=1)" >&2; exit 1; }
  echo "== Drogon configure =="
  cmake -S backend/cpp-drogon -B build/drogon -DCMAKE_BUILD_TYPE=Release
  echo "== Drogon build =="
  cmake --build build/drogon --parallel
fi

echo "All selected Qoin backend checks passed."
