#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
BUNDLE="target/bundled/Pivot.clap"
STAGE="dist/stage"
ARCHIVE="dist/Pivot-${VERSION}-macOS-universal.zip"

cargo xtask bundle-universal pivot --release

plutil -replace CFBundleIdentifier -string com.iranjin.pivot "$BUNDLE/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$BUNDLE/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$VERSION" "$BUNDLE/Contents/Info.plist"
codesign --force --deep -s - "$BUNDLE"

rm -rf "$STAGE"
mkdir -p "$STAGE"
cp -R "$BUNDLE" "$STAGE/"
cp scripts/install.command "$STAGE/"
chmod +x "$STAGE/install.command"

mkdir -p dist
rm -f "$ARCHIVE"
ditto -c -k --norsrc "$STAGE" "$ARCHIVE"

echo "Created $ARCHIVE"
