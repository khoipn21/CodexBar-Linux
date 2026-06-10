#!/usr/bin/env bash
#
# gui-build-env.sh
#
# Sets up a user-local GTK4 development environment so the Rust GUI can compile
# WITHOUT root. Ubuntu 26.04 already ships the GTK4/libadwaita *runtime* libs
# (GNOME uses them); only the -dev headers/.pc/.so symlinks are missing. We
# `apt-get download` those (no root) and extract them into a private prefix.
#
# Usage:  source scripts/gui-build-env.sh
# Then:   (cd gui && cargo build)   # or run-gui.sh
#
# To (re)provision the prefix from scratch, run:  scripts/gui-build-env.sh --provision

set -euo pipefail

DEVPREFIX="${CODEXBAR_GTK_DEV_PREFIX:-$HOME/.local/gtk-dev}"
ROOT="$DEVPREFIX/root"
PCDIR1="$ROOT/usr/lib/x86_64-linux-gnu/pkgconfig"
PCDIR2="$ROOT/usr/share/pkgconfig"
LIBDIR="$ROOT/usr/lib/x86_64-linux-gnu"

provision() {
    echo "[gui-env] Provisioning user-local GTK4 dev prefix at $DEVPREFIX"
    mkdir -p "$DEVPREFIX/debs"
    local pkgs="libgtk-4-dev libadwaita-1-dev librsvg2-dev libcairo2-dev \
libglib2.0-dev libpango1.0-dev libgdk-pixbuf-2.0-dev libgraphene-1.0-dev \
libdbus-1-dev"

    # Full -dev dependency closure.
    apt-cache depends --recurse --no-recommends --no-suggests --no-conflicts \
        --no-breaks --no-replaces --no-enhances $pkgs 2>/dev/null \
        | grep -E '^[a-z]' | grep -- '-dev$' | sort -u > "$DEVPREFIX/devpkgs.txt"

    ( cd "$DEVPREFIX/debs" && xargs -a "$DEVPREFIX/devpkgs.txt" -n 20 apt-get download )

    rm -rf "$ROOT"; mkdir -p "$ROOT"
    for d in "$DEVPREFIX"/debs/*.deb; do dpkg-deb -x "$d" "$ROOT"; done

    # The .pc files hardcode prefix=/usr; redirect to the extracted root so
    # headers and libs resolve from our private prefix.
    for pc in "$PCDIR1"/*.pc "$PCDIR2"/*.pc; do
        [ -f "$pc" ] || continue
        sed -i "s|^prefix=/usr$|prefix=$ROOT/usr|" "$pc"
    done

    # The dev packages ship `lib*.so` symlinks pointing at versioned runtime
    # files (e.g. libcairo.so -> libcairo.so.2) that live in the SYSTEM libdir,
    # not our prefix. Materialize each versioned target as a symlink into the
    # prefix so the linker can follow `-lcairo` etc. The dynamic loader still
    # uses the real system runtime libs at run time.
    local sys="/usr/lib/x86_64-linux-gnu"
    for sofile in "$LIBDIR"/*.so; do
        [ -L "$sofile" ] || continue
        local tgt; tgt="$(readlink "$sofile")"
        if [ ! -e "$LIBDIR/$tgt" ] && [ -e "$sys/$tgt" ]; then
            ln -sf "$sys/$tgt" "$LIBDIR/$tgt"
        fi
    done
    # gtk4 also links libvulkan at build time.
    [ -e "$sys/libvulkan.so.1" ] && [ ! -e "$LIBDIR/libvulkan.so" ] && \
        ln -sf "$sys/libvulkan.so.1" "$LIBDIR/libvulkan.so"

    echo "[gui-env] Provisioned: $(find "$ROOT" -name '*.pc' | wc -l) pkg-config files."
}

if [ "${1:-}" = "--provision" ]; then
    provision
    exit 0
fi

# If not yet provisioned, do it now.
if [ ! -f "$PCDIR1/gtk4.pc" ]; then
    provision
fi

# Export build env. pkg-config finds headers/libs in the private prefix; the
# linker gets the dev .so symlinks there, while the dynamic loader at RUNTIME
# uses the system runtime libs (already installed).
export PKG_CONFIG_PATH="$PCDIR1:$PCDIR2${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export LIBRARY_PATH="$LIBDIR${LIBRARY_PATH:+:$LIBRARY_PATH}"
export RUSTFLAGS="-L $LIBDIR${RUSTFLAGS:+ $RUSTFLAGS}"

echo "[gui-env] GTK4 dev env ready (prefix: $DEVPREFIX, no root required)."
