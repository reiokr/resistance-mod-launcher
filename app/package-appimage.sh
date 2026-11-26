#!/usr/bin/env bash
# Build the AppImage and assemble a small release directory with legal docs.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
DIST="$ROOT/dist/release"
VERSION="$(python3 -c 'import json; print(json.load(open("'"$HERE"'/src-tauri/tauri.conf.json"))["version"])')"
NAME="Resistance-Mod-Launcher-${VERSION}-x86_64.AppImage"

cd "$HERE"

if [ "${1:-}" != "--no-build" ]; then
    pnpm tauri build
fi

APPIMAGE="$HERE/src-tauri/target/release/bundle/appimage/Resistance Mod Launcher_${VERSION}_amd64.AppImage"
if [ ! -f "$APPIMAGE" ]; then
    APPIMAGE="$(find "$HERE/src-tauri/target/release/bundle/appimage" -maxdepth 1 -name "*_${VERSION}_*.AppImage" -print -quit)"
fi
[ -f "$APPIMAGE" ] || {
    echo "AppImage for version ${VERSION} not found; run \`pnpm tauri build\` first." >&2
    exit 1
}

rm -rf "$DIST"
mkdir -p "$DIST"
cp "$APPIMAGE" "$DIST/$NAME"
cp "$ROOT/LICENSE" "$ROOT/NOTICE" "$ROOT/PROVENANCE.md" "$ROOT/THIRD_PARTY_NOTICES.md" "$DIST/"
cp "$ROOT/README.md" "$DIST/README.md"

(
    cd "$DIST"
    sha256sum "$NAME" LICENSE NOTICE PROVENANCE.md THIRD_PARTY_NOTICES.md README.md > SHA256SUMS
)

echo "AppImage release ready:"
echo "  $DIST/$NAME"
echo "  $DIST/SHA256SUMS"
