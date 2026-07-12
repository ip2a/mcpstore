#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MCPSTORE_MANIFEST="$ROOT_DIR/rust/apps/mcpstore/Cargo.toml"
TAURI_MANIFEST="$ROOT_DIR/desktop/tauri/Cargo.toml"
WEB_DIR="$ROOT_DIR/web"

for bin_dir in /usr/local/bin /opt/homebrew/bin; do
  if [ -d "$bin_dir" ] && [[ ":$PATH:" != *":$bin_dir:"* ]]; then
    PATH="$bin_dir:$PATH"
  fi
done

print_menu() {
  cat <<'MENU'

mcpstore debug menu
1) 外部开发模式 Web (Rust API + React/Vite)
2) 本地运行 App (Tauri)
3) 本地运行 TUI
4) 本地运行内置 Web (mcpstore web)
5) 清理构建产物
0) 退出
MENU
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "缺少命令: $1" >&2
    exit 1
  fi
}

ensure_web_deps() {
  require_cmd npm
  if [ ! -d "$WEB_DIR/node_modules" ]; then
    echo "[Web] node_modules 不存在，先安装依赖..."
    if [ -f "$WEB_DIR/package-lock.json" ]; then
      npm --prefix "$WEB_DIR" ci
    else
      npm --prefix "$WEB_DIR" install
    fi
  fi
}

run_external_web() {
  require_cmd cargo
  ensure_web_deps

  local api_host="${MCPSTORE_API_HOST:-127.0.0.1}"
  local api_port="${MCPSTORE_API_PORT:-18200}"
  local vite_host="${MCPSTORE_VITE_HOST:-127.0.0.1}"
  local api_target="http://${api_host}:${api_port}"
  local api_pid=""

  cleanup_api() {
    local status=$?
    trap - RETURN INT TERM
    if [ -n "$api_pid" ] && kill -0 "$api_pid" >/dev/null 2>&1; then
      echo "[API] 停止后台进程 $api_pid"
      kill "$api_pid" >/dev/null 2>&1 || true
      wait "$api_pid" >/dev/null 2>&1 || true
    fi
    return "$status"
  }
  trap cleanup_api RETURN
  trap 'cleanup_api; exit 130' INT TERM

  echo "[API] 启动 Rust API: $api_target"
  cargo run --manifest-path "$MCPSTORE_MANIFEST" --bin mcpstore -- api --host "$api_host" --port "$api_port" &
  api_pid="$!"

  sleep 1
  if ! kill -0 "$api_pid" >/dev/null 2>&1; then
    wait "$api_pid"
  fi

  echo "[Web] 启动 Vite，并接入 VITE_MCPSTORE_API_BASE=/api（通过 Vite proxy 转发到后端 API）"
  VITE_MCPSTORE_API_BASE="/api" npm --prefix "$WEB_DIR" run dev -- --host "$vite_host"
}

run_app() {
  require_cmd cargo
  cargo run --manifest-path "$TAURI_MANIFEST"
}

run_tui() {
  require_cmd cargo
  cargo run --manifest-path "$MCPSTORE_MANIFEST" --bin mcpstore -- tui
}

run_embedded_web() {
  require_cmd cargo
  ensure_web_deps

  local web_host="${MCPSTORE_WEB_HOST:-127.0.0.1}"
  local web_port="${MCPSTORE_WEB_PORT:-8080}"

  echo "[Web] 构建 React 产物..."
  npm --prefix "$WEB_DIR" run build

  echo "[Web] 启动内置 Web: http://${web_host}:${web_port}/"
  MCPSTORE_WEB_ASSETS_DIR="$WEB_DIR/dist" \
    cargo run --manifest-path "$MCPSTORE_MANIFEST" --bin mcpstore -- web --host "$web_host" --port "$web_port"
}

clean_artifacts() {
  require_cmd cargo
  echo "[Clean] 清理 Rust workspace 构建产物..."
  cargo clean --manifest-path "$ROOT_DIR/rust/Cargo.toml"

  if [ -f "$TAURI_MANIFEST" ]; then
    echo "[Clean] 清理 Tauri 构建产物..."
    cargo clean --manifest-path "$TAURI_MANIFEST"
  fi

  echo "[Clean] 清理 Web dist..."
  rm -rf "$WEB_DIR/dist"
}

main() {
  while true; do
    print_menu
    printf '\n请选择: '
    if ! read -r choice; then
      echo
      exit 0
    fi

    case "$choice" in
      1) run_external_web ;;
      2) run_app ;;
      3) run_tui ;;
      4) run_embedded_web ;;
      5) clean_artifacts ;;
      0) exit 0 ;;
      *) echo "未知选项: $choice" ;;
    esac
  done
}

main "$@"
