#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "[error] Usage: $0 <binary-path>"
  exit 1
fi

BIN="$1"

if [ ! -f "${BIN}" ]; then
  echo "[error] Binary not found: ${BIN}"
  exit 1
fi

echo "[run] ${BIN} --version"
"${BIN}" --version

echo "[run] ${BIN} mcp-server --help"
"${BIN}" mcp-server --help >/dev/null

echo "[ok] Smoke test passed: ${BIN}"
