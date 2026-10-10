# AppImage recipe for Pulsar (linuxdeploy). Run from the repo root:
#   cargo build --release
#   packaging/make-appimage.sh
#
# Produces dist/Pulsar-x86_64.AppImage. Requires `linuxdeploy` and a `pulsar.AppDir` tree.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APPDIR="$ROOT/dist/pulsar.AppDir"
BIN="$ROOT/target/release/pulsar"

[ -x "$BIN" ] || { echo "build first: cargo build --release"; exit 1; }

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps"

install -m755 "$BIN" "$APPDIR/usr/bin/pulsar"
install -m644 "$ROOT/packaging/pulsar.desktop" "$APPDIR/usr/share/applications/pulsar.desktop"
install -m644 "$ROOT/packaging/pulsar.svg" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps/pulsar.svg"
ln -sf usr/share/applications/pulsar.desktop "$APPDIR/pulsar.desktop"
ln -sf usr/share/icons/hicolor/scalable/apps/pulsar.svg "$APPDIR/pulsar.svg"

# Bundle webkit2gtk/gtk runtime libs.
if command -v linuxdeploy-plugin-gtk >/dev/null 2>&1; then
  DEPLOY_PLUGINS=gtk linuxdeploy --appdir "$APPDIR" --desktop-file "$APPDIR/pulsar.desktop" --icon-file "$APPDIR/pulsar.svg" --output appimage
else
  echo "install linuxdeploy + linuxdeploy-plugin-gtk to produce the AppImage" >&2
fi
