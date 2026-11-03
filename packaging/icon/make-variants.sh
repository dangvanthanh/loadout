#!/bin/bash
# Rebuilds the app icon variants in crates/loadout/assets/app-icon/ from
# reference/images-prototype/loadout-logo.png. Needs ImageMagick (brew install imagemagick).
#   <variant>.png       192 px tile for in-app headers (menu, Settings)
#   <variant>-dock.png  256 px with the macOS icon grid's padding, for the Dock
set -euo pipefail
cd "$(dirname "$0")/../.."
LOGO=reference/images-prototype/loadout-logo.png
OUT=crates/loadout/assets/app-icon
mkdir -p "$OUT"
variant() { # name, ImageMagick color operations
  local name="$1"; shift
  magick "$LOGO" "$@" -resize 192x192 -strip "$OUT/$name.png"
  magick "$LOGO" "$@" -resize 206x206 -background none -gravity center -extent 256x256 -strip "$OUT/$name-dock.png"
}
variant default
variant twilight -modulate 100,105,140
variant graphite -colorspace gray -level 5%,95% -colorspace sRGB
echo "Wrote $OUT"
