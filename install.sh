#!/bin/sh
# install.sh - Install ccsesh
# Usage: curl -fsSL https://raw.githubusercontent.com/ryanlewis/ccsesh/main/install.sh | sh

set -e

REPO="ryanlewis/ccsesh"
BINARY="ccsesh"
INSTALL_DIR="${CCSESH_INSTALL_DIR:-${HOME}/.local/bin}"

# --- Colour output (TTY detection) ---
if [ -t 1 ]; then
    BOLD='\033[1m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    RESET='\033[0m'
else
    BOLD='' GREEN='' YELLOW='' RED='' RESET=''
fi

info()  { printf "  ${GREEN}info${RESET}: %s\n" "$1"; }
warn()  { printf "  ${YELLOW}warn${RESET}: %s\n" "$1" >&2; }
error() { printf "  ${RED}error${RESET}: %s\n" "$1" >&2; exit 1; }

# --- Detect downloader ---
if command -v curl >/dev/null 2>&1; then
    DOWNLOADER="curl"
elif command -v wget >/dev/null 2>&1; then
    DOWNLOADER="wget"
else
    error "curl or wget is required to download ccsesh"
fi

download() {
    if [ "$DOWNLOADER" = "curl" ]; then
        curl -fsSL "$1"
    else
        wget -qO- "$1"
    fi
}

download_file() {
    if [ "$DOWNLOADER" = "curl" ]; then
        curl -fsSL -o "$2" "$1"
    else
        wget -q -O "$2" "$1"
    fi
}

# --- OS detection ---
case "$(uname -s)" in
    Darwin) _os="apple-darwin" ;;
    Linux)  _os="unknown-linux-gnu" ;;
    CYGWIN*|MINGW*|MSYS*)
        error "use install.ps1 for Windows: irm https://raw.githubusercontent.com/ryanlewis/ccsesh/main/install.ps1 | iex" ;;
    *)
        error "unsupported operating system: $(uname -s)" ;;
esac

# --- Arch detection ---
case "$(uname -m)" in
    x86_64|amd64)  _arch="x86_64" ;;
    arm64|aarch64) _arch="aarch64" ;;
    *)             error "unsupported architecture: $(uname -m)" ;;
esac

# --- Rosetta 2 detection (macOS running x86_64 under translation) ---
if [ "$_os" = "apple-darwin" ] && [ "$_arch" = "x86_64" ]; then
    if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = "1" ]; then
        _arch="aarch64"
    fi
fi

# --- Validate target ---
_target="${_arch}-${_os}"

# macOS x86_64 is not a supported target
if [ "$_target" = "x86_64-apple-darwin" ]; then
    error "macOS x86_64 (Intel) is not supported — ccsesh requires Apple Silicon (aarch64)"
fi

info "detected platform ${_target}"

# --- Version resolution ---
if [ -n "${VERSION}" ]; then
    _version="${VERSION}"
    # Ensure version starts with 'v'
    case "${_version}" in
        v*) ;;
        *)  _version="v${_version}" ;;
    esac
else
    _response=$(download "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null) || true
    if [ -n "${_response}" ]; then
        _version=$(printf '%s' "${_response}" | grep '"tag_name"' | sed 's/.*"\([^"]*\)".*/\1/' | head -1)
    fi
fi

if [ -z "${_version}" ]; then
    error "could not determine latest version (check network connectivity or set VERSION env var)"
fi

info "installing ccsesh ${_version}"

# --- Temp directory with cleanup ---
_tmpdir=$(mktemp -d) || error "failed to create temporary directory"
trap 'rm -rf "$_tmpdir"' EXIT

# --- Download archive ---
_archive="${BINARY}-${_target}.tar.gz"
_url="https://github.com/${REPO}/releases/download/${_version}/${_archive}"

info "downloading from github.com/${REPO}"
download_file "${_url}" "${_tmpdir}/${_archive}" \
    || error "download failed — check that release ${_version} exists at github.com/${REPO}/releases"

# --- Checksum verification ---
if [ "${CCSESH_SKIP_CHECKSUM:-}" = "1" ]; then
    warn "checksum verification skipped (CCSESH_SKIP_CHECKSUM=1)"
else
    _checksum_url="https://github.com/${REPO}/releases/download/${_version}/${_archive}.sha256"
    if _checksum_content=$(download "${_checksum_url}" 2>/dev/null); then
        _expected=$(printf '%s' "${_checksum_content}" | awk '{print $1}')
        if [ -n "${_expected}" ]; then
            _actual=""
            if command -v sha256sum >/dev/null 2>&1; then
                _actual=$(sha256sum "${_tmpdir}/${_archive}" | awk '{print $1}')
            elif command -v shasum >/dev/null 2>&1; then
                _actual=$(shasum -a 256 "${_tmpdir}/${_archive}" | awk '{print $1}')
            else
                error "cannot verify checksum: neither sha256sum nor shasum is available (set CCSESH_SKIP_CHECKSUM=1 to bypass)"
            fi
            if [ -n "${_actual}" ]; then
                if [ "${_actual}" != "${_expected}" ]; then
                    error "checksum mismatch (expected ${_expected}, got ${_actual})"
                fi
                info "checksum verified"
            fi
        else
            error "checksum file is empty or malformed — set CCSESH_SKIP_CHECKSUM=1 to bypass"
        fi
    else
        error "could not download checksum file — set CCSESH_SKIP_CHECKSUM=1 to bypass verification"
    fi
fi

# --- Extract ---
tar -xzf "${_tmpdir}/${_archive}" -C "${_tmpdir}" \
    || error "failed to extract archive"

# --- Install ---
mkdir -p "${INSTALL_DIR}" \
    || error "failed to create directory ${INSTALL_DIR} — check permissions"
mv "${_tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}" \
    || error "failed to install to ${INSTALL_DIR} — check permissions"
chmod +x "${INSTALL_DIR}/${BINARY}"

info "installed to ${INSTALL_DIR}/${BINARY}"

# --- PATH hint ---
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
        ;;
    *)
        warn "${INSTALL_DIR} is not in your PATH"
        _shell_name=$(basename "${SHELL:-/bin/sh}")
        case "${_shell_name}" in
            fish)
                printf "\n  Add it with:\n"
                printf "    ${BOLD}fish_add_path %s${RESET}\n\n" "${INSTALL_DIR}"
                ;;
            zsh)
                printf "\n  Add to ${BOLD}~/.zshrc${RESET}:\n"
                printf "    ${BOLD}export PATH=\"%s:\$PATH\"${RESET}\n\n" "${INSTALL_DIR}"
                ;;
            *)
                printf "\n  Add to ${BOLD}~/.bashrc${RESET}:\n"
                printf "    ${BOLD}export PATH=\"%s:\$PATH\"${RESET}\n\n" "${INSTALL_DIR}"
                ;;
        esac
        ;;
esac

info "run 'ccsesh' to get started"
