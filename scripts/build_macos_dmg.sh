#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TAURI_DIR="${REPO_ROOT}/desktop/tauri"
TARGET="${1:-}"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "[error] macOS DMG builds require Darwin"
  exit 1
fi

VERSION="$(python3 - "${REPO_ROOT}" <<'PY'
import json
import sys
from pathlib import Path

config = json.loads(Path(sys.argv[1]).joinpath("desktop/tauri/tauri.conf.json").read_text(encoding="utf-8"))
print(config["version"])
PY
)"

echo "[run] Building React web assets"
(
  cd "${REPO_ROOT}/web"
  npm run build
)

if [ -n "${TARGET}" ]; then
  case "${TARGET}" in
    aarch64-apple-darwin) MARKER="aarch64" ;;
    x86_64-apple-darwin) MARKER="x64" ;;
    universal-apple-darwin) MARKER="universal" ;;
    *) MARKER="${TARGET}" ;;
  esac
  TARGET_DIR="${TAURI_DIR}/target/${TARGET}/release"
else
  HOST="$(rustc -vV | awk '/^host:/ { print $2 }')"
  case "${HOST}" in
    aarch64-apple-darwin) MARKER="aarch64" ;;
    x86_64-apple-darwin) MARKER="x64" ;;
    *) MARKER="${HOST}" ;;
  esac
  TARGET_DIR="${TAURI_DIR}/target/release"
fi

echo "[run] Building macOS app bundle"
(
  cd "${TAURI_DIR}"
  if [ -n "${TARGET}" ]; then
    cargo tauri build --bundles app --ci --target "${TARGET}"
  else
    cargo tauri build --bundles app --ci
  fi
)

APP_ROOT="${TARGET_DIR}/bundle/macos"
APP_PATH="${APP_ROOT}/mcpstore.app"
if [ ! -d "${APP_PATH}" ]; then
  echo "[error] Missing app bundle: ${APP_PATH}"
  exit 1
fi

DMG_DIR="${TARGET_DIR}/bundle/dmg"
DMG_PATH="${DMG_DIR}/mcpstore_${VERSION}_${MARKER}.dmg"
STAGING_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mcpstore-dmg.XXXXXX")"
trap 'rm -rf "${STAGING_DIR}"' EXIT

mkdir -p "${DMG_DIR}"
rm -f "${DMG_PATH}"
cp -R "${APP_PATH}" "${STAGING_DIR}/mcpstore.app"
ln -s /Applications "${STAGING_DIR}/Applications"

echo "[run] Creating DMG without Finder automation"
hdiutil create \
  -volname "mcpstore" \
  -srcfolder "${STAGING_DIR}" \
  -ov \
  -format UDZO \
  "${DMG_PATH}"

if [ ! -f "${DMG_PATH}" ]; then
  echo "[error] DMG was not created: ${DMG_PATH}"
  exit 1
fi

echo "[ok] Wrote ${DMG_PATH}"
