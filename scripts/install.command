#!/bin/bash
set -e
cd "$(dirname "$0")"

xattr -dr com.apple.quarantine Pivot.clap 2>/dev/null || true

DEST="$HOME/Library/Audio/Plug-Ins/CLAP"
mkdir -p "$DEST"
rm -rf "$DEST/Pivot.clap"
cp -R Pivot.clap "$DEST/"

echo "Pivot installed to $DEST"
echo "Restart FL Studio and run 'Verify plugins'."
read -n 1 -s -r -p "Press any key to close..."
