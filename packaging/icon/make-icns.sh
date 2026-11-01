#!/bin/bash
# Rebuilds packaging/AppIcon.icns from packaging/icon/AppIcon.png (1024×1024, transparent).
set -euo pipefail
cd "$(dirname "$0")"
SET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$SET"
for size in 16 32 128 256 512; do
  sips -z $size $size AppIcon.png --out "$SET/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) AppIcon.png --out "$SET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o ../AppIcon.icns
echo "Wrote packaging/AppIcon.icns"
