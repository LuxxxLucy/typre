#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

case "${1:-build}" in
  run)     shift; cargo run --release -- "${@:-examples/showcase.md}" ;;
  install) cargo install --path . ;;
  build)   cargo build --release ;;
  *) echo "usage: build.sh [build|run|install]" >&2; exit 1 ;;
esac
