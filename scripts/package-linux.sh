#!/usr/bin/env bash
set -euo pipefail

PKG_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${CODEXBAR_VERSION:-0.1.0}"
ARCH="${CODEXBAR_ARCH:-amd64}"
APPDIR="$PKG_ROOT/out/appdir"
DEBROOT="$PKG_ROOT/out/debroot"
DIST="$PKG_ROOT/out/dist"
GTK_ENV="$PKG_ROOT/scripts/gui-build-env.sh"

log() { printf '\033[1;36m[pkg]\033[0m %s\n' "$*"; }

log "Building engine bundle"
"$PKG_ROOT/scripts/compile-engine.sh"

log "Building GUI release"
# shellcheck disable=SC1090
. "$GTK_ENV"
( cd "$PKG_ROOT/gui" && cargo build --release )

rm -rf "$APPDIR" "$DEBROOT" "$DIST"
mkdir -p \
  "$APPDIR/usr/bin" \
  "$APPDIR/usr/lib/codexbar/engine" \
  "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
  "$DIST"

cp "$PKG_ROOT/gui/target/release/codexbar-tray" "$APPDIR/usr/bin/codexbar-tray"
cp -a "$PKG_ROOT/out/engine/." "$APPDIR/usr/lib/codexbar/engine/"
cp "$PKG_ROOT/packaging/codexbar-tray.desktop" "$APPDIR/usr/share/applications/codexbar-tray.desktop"
cp "$PKG_ROOT/engine/CodexBar/codexbar.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/codexbar.png"

log "Creating tar bundle"
tar -C "$APPDIR" -czf "$DIST/codexbar-linux-${VERSION}-${ARCH}.tar.gz" .

log "Creating deb"
mkdir -p "$DEBROOT/DEBIAN"
cp -a "$APPDIR/usr" "$DEBROOT/"
installed_size=$(du -sk "$DEBROOT/usr" | awk '{print $1}')
cat > "$DEBROOT/DEBIAN/control" <<CONTROL
Package: codexbar
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: CodexBar Linux Port <noreply@example.invalid>
Installed-Size: $installed_size
Depends: libc6, libgtk-4-1, libadwaita-1-0, libglib2.0-0t64 | libglib2.0-0, libpango-1.0-0, libgdk-pixbuf-2.0-0, libcairo2, libgraphene-1.0-0, libepoxy0, libwayland-client0, libxkbcommon0, libdbus-1-3, libsecret-1-0, libcurl4t64 | libcurl4, libstdc++6, libsqlite3-0, libssl3t64 | libssl3, ca-certificates
Description: AI coding-provider usage tray for Linux
 Native GTK4/libadwaita tray GUI with bundled CodexBar Swift engine.
CONTROL

dpkg-deb --build "$DEBROOT" "$DIST/codexbar_${VERSION}_${ARCH}.deb" >/dev/null

log "Artifacts:"
ls -lh "$DIST"
