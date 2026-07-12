#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 3 ]; then
  echo "[error] Usage: $0 <target-triple> <platform-id> <artifact-binary>"
  exit 1
fi

TARGET_TRIPLE="$1"
PLATFORM_ID="$2"
ARTIFACT_BINARY="$3"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SRC_BIN="${REPO_ROOT}/rust/target/${TARGET_TRIPLE}/release/${ARTIFACT_BINARY}"
DST_DIR="${REPO_ROOT}/dist/${PLATFORM_ID}"
DST_BIN="${DST_DIR}/${ARTIFACT_BINARY}"

echo "[run] Building mcpstore target=${TARGET_TRIPLE}"
export CARGO_TARGET_DIR="${REPO_ROOT}/rust/target"
cargo build --release --target "${TARGET_TRIPLE}" --manifest-path "${REPO_ROOT}/rust/Cargo.toml" --bin mcpstore

if [ ! -f "${SRC_BIN}" ]; then
  echo "[error] Build artifact not found: ${SRC_BIN}"
  exit 1
fi

mkdir -p "${DST_DIR}"
cp "${SRC_BIN}" "${DST_BIN}"

case "${ARTIFACT_BINARY}" in
  *.exe) ;;
  *) chmod +x "${DST_BIN}" ;;
esac

echo "[ok] Wrote: ${DST_BIN}"
