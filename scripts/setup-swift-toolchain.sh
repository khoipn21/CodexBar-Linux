#!/usr/bin/env bash
#
# setup-swift-toolchain.sh
#
# Idempotently provisions the Swift toolchain needed to build the CodexBar
# engine (CodexBarCore + CodexBarCLI) on Ubuntu 26.04 LTS (x86_64).
#
# Why this is non-trivial:
#   - There is no official Swift toolchain built for Ubuntu 26.04 yet.
#   - We install the ubuntu24.04 build of Swift; it RUNS on 26.04 because glibc
#     is backward compatible. But the 24.04 toolchain links against OLD library
#     sonames that 26.04 no longer ships (libxml2.so.2, ICU 74, libsqlite3.so),
#     so we provide those via a private compat-libs/ shim consumed through
#     LD_LIBRARY_PATH / LIBRARY_PATH (we never touch system libraries).
#
# This was verified empirically: with this setup, `swift build` compiles the
# entire engine and the resulting CLI runs and fetches real data on 26.04.
#
# Safe to re-run: every step checks for existing state before acting.

set -euo pipefail

# --- Pinned versions (bump deliberately) -------------------------------------
SWIFT_PLATFORM="ubuntu24.04"          # closest official build to 26.04
SWIFTLY_HOME_DIR="${SWIFTLY_HOME_DIR:-$HOME/.local/share/swiftly}"
COMPAT_LIBS_DIR="$SWIFTLY_HOME_DIR/compat-libs"

# Runtime sonames the 24.04 toolchain needs but 26.04 does not ship.
# We source these from snap packages (they bundle the 24.04-era libs).
declare -a COMPAT_SONAMES=(
  "libxml2.so.2"
  "libicuuc.so.74"
  "libicui18n.so.74"
  "libicudata.so.74"
)

# apt build dependencies recommended by swiftly for this toolchain.
declare -a APT_DEPS=(
  libcurl4-openssl-dev
  libgcc-13-dev
  libpython3-dev
  libstdc++-13-dev
  libz3-dev
)

log()  { printf '\033[1;36m[setup]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[setup] WARN:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31m[setup] ERROR:\033[0m %s\n' "$*" >&2; exit 1; }

# --- 1. apt dependencies -----------------------------------------------------
install_apt_deps() {
  local missing=()
  for pkg in "${APT_DEPS[@]}"; do
    dpkg -s "$pkg" >/dev/null 2>&1 || missing+=("$pkg")
  done
  if [ ${#missing[@]} -eq 0 ]; then
    log "apt dependencies already installed."
    return
  fi
  log "Installing apt dependencies: ${missing[*]}"
  if [ "$(id -u)" -eq 0 ]; then
    apt-get update -y && apt-get install -y "${missing[@]}"
  else
    warn "Need root to install: ${missing[*]}"
    warn "Run: sudo apt-get install -y ${missing[*]}"
    die  "Re-run this script after installing the apt dependencies."
  fi
}

# --- 2. swiftly + toolchain --------------------------------------------------
install_swiftly() {
  if [ -x "$SWIFTLY_HOME_DIR/bin/swiftly" ] && \
     [ -n "$(ls -d "$SWIFTLY_HOME_DIR"/toolchains/* 2>/dev/null || true)" ]; then
    log "swiftly + a toolchain already present at $SWIFTLY_HOME_DIR."
    return
  fi

  local arch tmp
  arch="$(uname -m)"
  tmp="$(mktemp -d)"
  log "Downloading swiftly for $arch ..."
  curl -fsSL "https://download.swift.org/swiftly/linux/swiftly-${arch}.tar.gz" \
    -o "$tmp/swiftly.tar.gz" || die "swiftly download failed."
  tar -xzf "$tmp/swiftly.tar.gz" -C "$tmp"

  # swiftly rejects 26.04 in /etc/os-release, so force the platform.
  log "Initializing swiftly (platform=$SWIFT_PLATFORM). Downloads ~1GB toolchain."
  SWIFTLY_HOME_DIR="$SWIFTLY_HOME_DIR" "$tmp/swiftly" init \
    --assume-yes --no-modify-profile --platform "$SWIFT_PLATFORM" \
    || die "swiftly init failed."
  rm -rf "$tmp"
}

# --- 3. compat-libs shim -----------------------------------------------------
provision_compat_libs() {
  mkdir -p "$COMPAT_LIBS_DIR"

  for soname in "${COMPAT_SONAMES[@]}"; do
    if [ -f "$COMPAT_LIBS_DIR/$soname" ]; then
      continue
    fi
    # Find a real copy of the soname (versioned) under /snap or /usr.
    local src
    src="$(find /snap /usr /opt -name "${soname}*" 2>/dev/null \
             | grep -E 'x86_64-linux-gnu' | grep -v "$COMPAT_LIBS_DIR" \
             | head -1 || true)"
    if [ -z "$src" ]; then
      die "Cannot locate a copy of $soname on this system. \
Install a snap that bundles it (e.g. mesa-2404) or supply it manually in $COMPAT_LIBS_DIR."
    fi
    log "Providing $soname  <-  $src"
    cp "$src" "$COMPAT_LIBS_DIR/$soname"
  done

  # libsqlite3.so dev symlink (26.04 ships only the runtime .so.0).
  if [ ! -e "$COMPAT_LIBS_DIR/libsqlite3.so" ]; then
    local sqlite
    sqlite="$(find /usr/lib -name 'libsqlite3.so.0' 2>/dev/null | head -1 || true)"
    [ -n "$sqlite" ] || die "libsqlite3.so.0 not found; install libsqlite3-0."
    ln -sf "$sqlite" "$COMPAT_LIBS_DIR/libsqlite3.so"
    log "Created libsqlite3.so -> $sqlite"
  fi
}

# --- 4. verify ---------------------------------------------------------------
verify() {
  # shellcheck disable=SC1091
  . "$SWIFTLY_HOME_DIR/env.sh"
  hash -r
  export LD_LIBRARY_PATH="$COMPAT_LIBS_DIR:${LD_LIBRARY_PATH:-}"
  export LIBRARY_PATH="$COMPAT_LIBS_DIR:${LIBRARY_PATH:-}"

  log "swift --version:"
  swift --version || die "swift does not run."
  log "swift-package --version:"
  swift-package --version || die "swift-package does not run (compat shim incomplete)."
  log "Toolchain ready. Source the env before building:"
  printf '  . "%s/env.sh"\n' "$SWIFTLY_HOME_DIR"
  printf '  export LD_LIBRARY_PATH="%s:$LD_LIBRARY_PATH"\n' "$COMPAT_LIBS_DIR"
  printf '  export LIBRARY_PATH="%s:$LIBRARY_PATH"\n' "$COMPAT_LIBS_DIR"
}

main() {
  log "Ubuntu: $(. /etc/os-release; echo "$PRETTY_NAME")  arch: $(uname -m)"
  install_apt_deps
  install_swiftly
  provision_compat_libs
  verify
  log "Done."
}

main "$@"
