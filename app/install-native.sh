#!/usr/bin/env bash
# Build and install the native Tauri launcher into the user's XDG dirs.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/256x256/apps"
DOC_DIR="${HOME}/.local/share/doc/resistance-mod-launcher"

cd "$HERE"

if [ "${1:-}" != "--no-build" ]; then
    pnpm install
    pnpm tauri build --no-bundle
fi

BIN="$HERE/src-tauri/target/release/xcom2-mod-launcher"
[ -f "$BIN" ] || { echo "Release binary not found: $BIN" >&2; exit 1; }

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR" "$DOC_DIR"

install -m 755 "$BIN" "$BIN_DIR/xcom2-mod-launcher"
install -m 644 "$HERE/../LICENSE" "$DOC_DIR/LICENSE"
install -m 644 "$HERE/../NOTICE" "$DOC_DIR/NOTICE"
install -m 644 "$HERE/../PROVENANCE.md" "$DOC_DIR/PROVENANCE.md"
sed "s|^Exec=xcom2-mod-launcher|Exec=${BIN_DIR}/xcom2-mod-launcher|" \
    "$HERE/xcom2-mod-launcher.desktop" > "$APP_DIR/xcom2-mod-launcher.desktop"
chmod 644 "$APP_DIR/xcom2-mod-launcher.desktop"

ICON_SRC="$HERE/src-tauri/icons/128x128@2x.png"
[ -f "$ICON_SRC" ] || ICON_SRC="$HERE/src-tauri/icons/icon.png"
install -m 644 "$ICON_SRC" "$ICON_DIR/xcom2-mod-launcher.png"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi

echo "Installed native launcher:"
echo "  $BIN_DIR/xcom2-mod-launcher"
echo "  $APP_DIR/xcom2-mod-launcher.desktop"
echo "  $ICON_DIR/xcom2-mod-launcher.png"
echo "  $DOC_DIR/LICENSE, NOTICE, PROVENANCE.md"
echo
echo "Start menu entry: Resistance Mod Launcher"
