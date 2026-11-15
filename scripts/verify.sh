#!/usr/bin/env bash
set -euo pipefail

MODE="all"
if [[ $# -gt 0 ]]; then
  case "$1" in
    --docs) MODE="docs" ;;
    --rust) MODE="rust" ;;
    *)
      echo "Usage: $0 [--docs|--rust]" >&2
      exit 1
      ;;
  esac
fi

run_tool() {
  if command -v mise >/dev/null 2>&1; then
    mise exec -- "$@"
  else
    "$@"
  fi
}

check_docs() {
  echo "==> Running typos..."
  run_tool typos

  echo "==> Running lychee..."
  run_tool lychee --offline --no-progress --include-fragments './**/*.md' |
    sed -E 's/[🔍🔗✅🚫👻⏳🔀❓⛔📝❌] ?//g'

  echo "==> Running actionlint..."
  run_tool actionlint

  echo "==> Running gitleaks..."
  run_tool gitleaks dir --no-banner --redact .
  run_tool gitleaks git --no-banner --redact .
}

check_rust() {
  if [[ ! -f Cargo.toml ]]; then
    echo "No Cargo.toml found; skipping rust checks."
    return 0
  fi

  echo "==> Running cargo fmt..."
  run_tool cargo fmt --check

  echo "==> Running cargo clippy..."
  run_tool cargo clippy --all-targets --locked -- -D warnings

  echo "==> Running cargo nextest..."
  run_tool cargo nextest run --locked --no-tests=warn

  echo "==> Running cargo test --doc..."
  run_tool cargo test --doc --locked

  echo "==> Running cargo deny..."
  run_tool cargo deny check

  echo "==> Running cargo shear..."
  run_tool cargo shear
}

if [[ "$MODE" == "all" || "$MODE" == "docs" ]]; then
  check_docs
fi

if [[ "$MODE" == "all" || "$MODE" == "rust" ]]; then
  check_rust
fi

echo "verify: OK"
