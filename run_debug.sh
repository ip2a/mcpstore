#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MCPSTORE_MANIFEST="$ROOT_DIR/rust/apps/mcpstore/Cargo.toml"
TAURI_MANIFEST="$ROOT_DIR/desktop/tauri/Cargo.toml"
WEB_DIR="$ROOT_DIR/web"
PYTHON_DIR="$ROOT_DIR/python"
PYTHON_VENV="$PYTHON_DIR/.venv"
PYTHON_BIN="$PYTHON_VENV/bin/python"
PYTHON_MATURIN="$PYTHON_VENV/bin/maturin"

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
5) Python API demo (FastAPI, demos/python_api)
6) 清理构建产物
7) 构建并安装最新本地 mcpstore 到 python/.venv
8) 从 python/.venv 卸载 mcpstore
9) 强制重建并重装 mcpstore（清缓存全量编译）
10) 发版前工作
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

free_tcp_port() {
  local port="$1"
  local pids

  pids="$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)"
  if [ -z "$pids" ]; then
    return
  fi

  echo "[Port] $port 被占用，停止进程: $(echo "$pids" | tr '\n' ' ')"
  while IFS= read -r pid; do
    kill "$pid" >/dev/null 2>&1 || true
  done <<< "$pids"

  sleep 1
  pids="$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)"
  if [ -n "$pids" ]; then
    echo "[Port] 强制停止进程: $(echo "$pids" | tr '\n' ' ')"
    while IFS= read -r pid; do
      kill -9 "$pid" >/dev/null 2>&1 || true
    done <<< "$pids"
  fi
}

run_external_web() {
  require_cmd cargo
  ensure_web_deps

  local api_host="${MCPSTORE_API_HOST:-127.0.0.1}"
  local api_port="${MCPSTORE_API_PORT:-1820}"
  local vite_host="${MCPSTORE_VITE_HOST:-127.0.0.1}"
  local vite_port="${MCPSTORE_VITE_PORT:-1828}"
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

  require_cmd lsof
  free_tcp_port "$api_port"
  free_tcp_port "$vite_port"

  echo "[API] 启动 Rust API: $api_target"
  cargo run --manifest-path "$MCPSTORE_MANIFEST" --bin mcpstore -- api --host "$api_host" --port "$api_port" &
  api_pid="$!"

  sleep 1
  if ! kill -0 "$api_pid" >/dev/null 2>&1; then
    wait "$api_pid"
  fi

  echo "[Web] 启动 Vite，并接入 VITE_MCPSTORE_API_BASE=/api（通过 Vite proxy 转发到后端 API）"
  VITE_MCPSTORE_API_BASE="/api" npm --prefix "$WEB_DIR" run dev -- --host "$vite_host" --port "$vite_port"
}

run_app() {
  require_cmd cargo
  ensure_web_deps

  echo "[Web] 构建 React 产物（确保桌面端加载最新前端）..."
  npm --prefix "$WEB_DIR" run build

  echo "[App] 启动 Tauri 桌面端（通过 MCPSTORE_WEB_ASSETS_DIR 实时加载最新 dist）..."
  MCPSTORE_WEB_ASSETS_DIR="$WEB_DIR/dist" \
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

run_python_demo() {
  local demo_dir="$ROOT_DIR/demos/python_api"
  local demo_host="${MCPSTORE_DEMO_HOST:-127.0.0.1}"
  local demo_port="${MCPSTORE_DEMO_PORT:-18201}"

  if [ ! -f "$demo_dir/app.py" ]; then
    echo "[Demo] 找不到 $demo_dir/app.py" >&2
    exit 1
  fi
  if [ ! -x "$PYTHON_BIN" ]; then
    echo "[Demo] 找不到 uv 虚拟环境: $PYTHON_BIN" >&2
    echo "       请先在 python/ 目录执行 uv sync" >&2
    exit 1
  fi

  echo "[Demo] 启动 FastAPI demo: http://${demo_host}:${demo_port}/"
  echo "[Demo] API 文档:         http://${demo_host}:${demo_port}/docs"
  (cd "$demo_dir" && "$PYTHON_BIN" -m uvicorn app:app --reload --host "$demo_host" --port "$demo_port")
}

build_and_install_wheel() {
  local wheel_dir wheel
  wheel_dir="$(mktemp -d)"
  echo "[Python] 构建 Rust binding 和 Python wheel..."
  if ! (cd "$PYTHON_DIR" && "$PYTHON_MATURIN" build --interpreter "$PYTHON_BIN" --out "$wheel_dir"); then
    rm -rf "$wheel_dir"
    return 1
  fi

  wheel="$(find "$wheel_dir" -type f -name 'mcpstore-*.whl' -print -quit)"
  if [ -z "$wheel" ]; then
    echo "[Python] 构建完成但没有找到 mcpstore wheel" >&2
    rm -rf "$wheel_dir"
    return 1
  fi

  echo "[Python] 安装到 $PYTHON_VENV..."
  if ! uv pip install --python "$PYTHON_BIN" --force-reinstall "$wheel"; then
    rm -rf "$wheel_dir"
    return 1
  fi
  rm -rf "$wheel_dir"
}

verify_python_install() {
  "$PYTHON_BIN" - <<'PY'
import mcpstore
import mcpstore._rust as rust
import sys

print(f"[Python] mcpstore: {mcpstore.__file__}")
print(f"[Python] Rust 扩展: {rust.__file__}")
print(f"[Python] restart_control_reactor 可用: {hasattr(rust.MCPStore, 'restart_control_reactor')}")
if "python/src" in mcpstore.__file__:
    print("[Python] ⚠️ 仍加载自源码树 python/src，不是 wheel")
    print(f"[Python] 作怪的 sys.path 条目: {[p for p in sys.path if 'python/src' in p]}")
    print("[Python] 检查是否有 editable .pth 残留或 PYTHONPATH 环境变量")
PY
}

install_python_package() {
  require_cmd uv
  if [ ! -x "$PYTHON_BIN" ] || [ ! -x "$PYTHON_MATURIN" ]; then
    echo "[Python] 找不到 $PYTHON_VENV；请先在 python/ 目录执行 uv sync" >&2
    return 1
  fi
  build_and_install_wheel
  verify_python_install
}

force_reinstall_python_package() {
  require_cmd uv
  require_cmd cargo
  if [ ! -x "$PYTHON_BIN" ] || [ ! -x "$PYTHON_MATURIN" ]; then
    echo "[Python] 找不到 $PYTHON_VENV；请先在 python/ 目录执行 uv sync" >&2
    return 1
  fi

  echo "[Python] 卸载现有 mcpstore..."
  uv pip uninstall --python "$PYTHON_BIN" mcpstore >/dev/null 2>&1 || true

  echo "[Python] 清理 editable 安装残留（.pth / finder / 旧目录）..."
  "$PYTHON_BIN" - <<'PY'
import glob, os, shutil, site
sp = site.getsitepackages()[0]
removed = 0
for pattern in ("__editable__*mcpstore*", "__editable___mcpstore*",
                "*mcpstore*.pth", "_mcpstore*"):
    for path in glob.glob(os.path.join(sp, pattern)):
        try:
            os.remove(path)
            removed += 1
        except OSError:
            pass
pkg = os.path.join(sp, "mcpstore")
if os.path.isdir(pkg):
    shutil.rmtree(pkg, ignore_errors=True)
    removed += 1
print(f"[Python] 清理 {removed} 项残留")
PY

  echo "[Rust] 清理 target 缓存（强制全量重编译，下次 Rust 构建也会变慢）..."
  cargo clean --manifest-path "$ROOT_DIR/rust/Cargo.toml"

  build_and_install_wheel
  verify_python_install
}

uninstall_python_package() {
  require_cmd uv
  if [ ! -x "$PYTHON_BIN" ]; then
    echo "[Python] 找不到 uv 虚拟环境: $PYTHON_BIN" >&2
    return 1
  fi

  echo "[Python] 从 $PYTHON_VENV 卸载 mcpstore..."
  uv pip uninstall --python "$PYTHON_BIN" mcpstore
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

read_current_release_version() {
  python3 - <<PY
import sys
sys.path.insert(0, "${ROOT_DIR}/scripts")
from release_version import read_canonical_version
print(read_canonical_version())
PY
}

run_pre_release_work() {
  require_cmd python3

  local sync_script="$ROOT_DIR/scripts/sync_version.py"
  local preflight_script="$ROOT_DIR/scripts/release_preflight.py"
  local current new_version

  if [ ! -f "$sync_script" ] || [ ! -f "$preflight_script" ]; then
    echo "[Release] 找不到版本同步脚本（scripts/sync_version.py / release_preflight.py）" >&2
    return 1
  fi

  current="$(read_current_release_version)"
  echo
  echo "[Release] 发版前工作"
  echo "[Release] 当前版本: ${current}"
  printf '[Release] 输入新版本: '
  if ! read -r new_version; then
    echo
    return 1
  fi

  new_version="${new_version#v}"
  new_version="${new_version// /}"
  if [ -z "$new_version" ]; then
    echo "[Release] 版本号不能为空" >&2
    return 1
  fi

  if ! [[ "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
    echo "[Release] 版本号格式无效: ${new_version}" >&2
    echo "[Release] 期望 semver，例如 2.0.1 或 2.0.1-rc.1" >&2
    return 1
  fi

  if [ "$new_version" = "$current" ]; then
    echo "[Release] 新版本与当前版本相同，跳过写入"
  else
    echo "[Release] 同步 ${current} -> ${new_version} ..."
    python3 "$sync_script" "$new_version"
  fi

  echo "[Release] 校验发布元数据..."
  python3 "$preflight_script"
  echo "[Release] 完成。请检查 git diff，确认后提交版本变更。"
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
      5) run_python_demo ;;
      6) clean_artifacts ;;
      7) install_python_package ;;
      8) uninstall_python_package ;;
      9) force_reinstall_python_package ;;
      10) run_pre_release_work ;;
      0) exit 0 ;;
      *) echo "未知选项: $choice" ;;
    esac
  done
}

main "$@"
