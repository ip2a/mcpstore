#!/usr/bin/env bash
set -euo pipefail

TOOL_NAME="${TOOL_NAME:-mcpstore}"
REPOSITORY="${REPOSITORY:-ip2a/mcpstore}"
GITHUB_BASE_URL="${GITHUB_BASE_URL:-https://github.com}"
VERSION_INPUT="${VERSION:-}"

if [ -n "${INSTALL_DIR:-}" ]; then
  TARGET_DIR="${INSTALL_DIR}"
elif [ "${EUID:-$(id -u)}" -eq 0 ]; then
  TARGET_DIR="/usr/local/bin"
else
  TARGET_DIR="${HOME}/.local/bin"
fi

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[error] Required command not found: $1" >&2
    exit 1
  fi
}

compute_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
    return
  fi
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
    return
  fi
  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$1" | awk '{print $NF}'
    return
  fi
  echo "[error] No SHA256 tool found (need sha256sum, shasum, or openssl)" >&2
  exit 1
}

normalize_version() {
  local raw="$1"
  raw="${raw#v}"
  printf '%s' "$raw"
}

resolve_latest_version() {
  local latest_url
  latest_url="$(
    curl -fsSLI -o /dev/null -w '%{url_effective}' \
      "${GITHUB_BASE_URL}/${REPOSITORY}/releases/latest"
  )"
  local tag="${latest_url##*/}"
  if [ -z "${tag}" ] || [ "${tag}" = "latest" ]; then
    echo "[error] Unable to resolve latest GitHub release tag" >&2
    exit 1
  fi
  normalize_version "${tag}"
}

detect_platform_id() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}" in
    Darwin)
      case "${arch}" in
        arm64|aarch64) printf 'darwin-arm64' ;;
        x86_64) printf 'darwin-x64' ;;
        *) echo "[error] Unsupported macOS architecture: ${arch}" >&2; exit 1 ;;
      esac
      ;;
    Linux)
      case "${arch}" in
        x86_64|amd64) printf 'linux-x64-gnu' ;;
        arm64|aarch64) printf 'linux-arm64-gnu' ;;
        *) echo "[error] Unsupported Linux architecture: ${arch}" >&2; exit 1 ;;
      esac
      ;;
    *)
      echo "[error] Unsupported operating system: ${os}" >&2
      echo "[hint] Download the matching binary manually from GitHub Releases." >&2
      exit 1
      ;;
  esac
}

main() {
  require_command curl
  require_command tar
  require_command mktemp
  require_command grep
  require_command awk
  require_command install

  local version tag platform_id archive_name download_base_url
  if [ -n "${VERSION_INPUT}" ]; then
    version="$(normalize_version "${VERSION_INPUT}")"
  else
    version="$(resolve_latest_version)"
  fi
  tag="v${version}"
  platform_id="$(detect_platform_id)"
  archive_name="${TOOL_NAME}-${tag}-${platform_id}.tar.gz"
  download_base_url="${GITHUB_BASE_URL}/${REPOSITORY}/releases/download/${tag}"

  local tmpdir archive_path checksum_path expected actual extract_dir installed_bin
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "'"${tmpdir}"'"' EXIT
  archive_path="${tmpdir}/${archive_name}"
  checksum_path="${tmpdir}/SHA256SUMS.txt"
  extract_dir="${tmpdir}/extract"
  installed_bin="${TARGET_DIR}/${TOOL_NAME}"

  echo "[info] Installing ${TOOL_NAME} ${version} for ${platform_id}"
  mkdir -p "${TARGET_DIR}" "${extract_dir}"

  curl -fsSL "${download_base_url}/SHA256SUMS.txt" -o "${checksum_path}"
  curl -fsSL "${download_base_url}/${archive_name}" -o "${archive_path}"

  expected="$(grep -F "  ${archive_name}" "${checksum_path}" | awk '{print $1}')"
  if [ -z "${expected}" ]; then
    echo "[error] Could not find checksum for ${archive_name}" >&2
    exit 1
  fi

  actual="$(compute_sha256 "${archive_path}")"
  if [ "${expected}" != "${actual}" ]; then
    echo "[error] SHA256 mismatch for ${archive_name}" >&2
    exit 1
  fi
  echo "[ok] Verified SHA256 for ${archive_name}"

  tar -xzf "${archive_path}" -C "${extract_dir}" "${TOOL_NAME}"
  install -m 0755 "${extract_dir}/${TOOL_NAME}" "${installed_bin}"

  local installed_version
  installed_version="$("${installed_bin}" --version 2>/dev/null || "${installed_bin}" version)"
  if [[ "${installed_version}" != *"${version}"* ]]; then
    echo "[error] Installed binary version mismatch: ${installed_version}" >&2
    exit 1
  fi

  echo "[ok] Installed ${installed_version} to ${installed_bin}"
  if [[ ":${PATH}:" != *":${TARGET_DIR}:"* ]]; then
    echo "[hint] ${TARGET_DIR} is not in PATH"
    echo "[hint] Add this line to your shell profile:"
    echo "       export PATH=\"${TARGET_DIR}:\$PATH\""
  fi
}

main "$@"
