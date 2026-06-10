#!/usr/bin/env bash
set -euo pipefail

clean=false
run_tests=false
build_debug=true
build_release=true
dist_dir="dist"

usage() {
  cat <<'EOF'
Usage: scripts/build-windows.sh [options]

Build Windows debug and release binaries, then copy them into dist folders.

Options:
  --clean          Run cargo clean before building.
  --run-tests      Run cargo test --workspace before building.
  --debug-only     Build only debug binaries.
  --release-only   Build only release binaries.
  --dist-dir DIR   Copy binaries under DIR instead of dist.
  -h, --help       Show this help.

Outputs:
  dist/debug/studio-app.exe
  dist/debug/studio-cli.exe
  dist/release/studio-app.exe
  dist/release/studio-cli.exe

For compatibility with the existing release layout, release binaries are also
copied to:
  dist/studio-app.exe
  dist/studio-cli.exe
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --clean)
      clean=true
      shift
      ;;
    --run-tests)
      run_tests=true
      shift
      ;;
    --debug-only)
      build_debug=true
      build_release=false
      shift
      ;;
    --release-only)
      build_debug=false
      build_release=true
      shift
      ;;
    --dist-dir)
      if [[ $# -lt 2 ]]; then
        echo "error: --dist-dir requires a value" >&2
        exit 2
      fi
      dist_dir="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$(uname -s 2>/dev/null || echo unknown)" in
  MINGW*|MSYS*|CYGWIN*)
    ;;
  *)
    echo "warning: this script is intended for Windows Bash environments such as Git Bash or MSYS2." >&2
    ;;
esac

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
cd "$repo_root"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo was not found in PATH" >&2
  exit 1
fi

binaries=("studio-app.exe" "studio-cli.exe")

copy_profile() {
  local profile="$1"
  local target_dir="$repo_root/target/$profile"
  local output_dir="$repo_root/$dist_dir/$profile"

  mkdir -p "$output_dir"

  for binary in "${binaries[@]}"; do
    local source="$target_dir/$binary"
    local output="$output_dir/$binary"

    if [[ ! -f "$source" ]]; then
      echo "error: expected binary was not found: $source" >&2
      exit 1
    fi

    cp -f "$source" "$output"
  done
}

copy_release_compat() {
  local target_dir="$repo_root/target/release"
  local output_dir="$repo_root/$dist_dir"

  mkdir -p "$output_dir"

  for binary in "${binaries[@]}"; do
    cp -f "$target_dir/$binary" "$output_dir/$binary"
  done
}

print_outputs() {
  echo
  echo "Build outputs:"
  for profile in debug release; do
    local output_dir="$repo_root/$dist_dir/$profile"
    if [[ -d "$output_dir" ]]; then
      for binary in "${binaries[@]}"; do
        local output="$output_dir/$binary"
        if [[ -f "$output" ]]; then
          local bytes
          bytes="$(wc -c < "$output" | tr -d '[:space:]')"
          echo "  $output ($bytes bytes)"
        fi
      done
    fi
  done
}

if [[ "$clean" == true ]]; then
  echo "Cleaning cargo artifacts..."
  cargo clean
fi

if [[ "$run_tests" == true ]]; then
  echo "Running workspace tests..."
  cargo test --workspace
fi

if [[ "$build_debug" == true ]]; then
  echo "Building debug workspace..."
  cargo build --workspace
  copy_profile debug
fi

if [[ "$build_release" == true ]]; then
  echo "Building release workspace..."
  cargo build --release --workspace
  copy_profile release
  copy_release_compat
fi

print_outputs
