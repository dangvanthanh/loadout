#!/bin/bash
# Rebuilds crates/loadout/assets/tray/menubar@2x.png and menubar-update@2x.png (36 px, drawn at 18 pt) from
# reference/images-prototype/loadout-menubar.png. Needs ImageMagick (brew install imagemagick).
# The menu bar icon is a template image, where only alpha counts: keep the light body opaque and
# cut the darker detail lines (eyes, smile, hat band) out, so the octopus stays readable.
set -euo pipefail
cd "$(dirname "$0")/../.."
magick reference/images-prototype/loadout-menubar.png -trim +repage \
  \( +clone -alpha off -colorspace gray -threshold 85% \) \
  \( -clone 0 -alpha extract \) -delete 0 -compose multiply -composite \
  -background black -alpha shape \
  -resize 34x34 -background none -gravity center -extent 36x36 \
  crates/loadout/assets/tray/menubar@2x.png
# Updates available: the same glyph with a dot in the top-right, cut free of the hat by a ring.
magick crates/loadout/assets/tray/menubar@2x.png \
  \( -size 36x36 xc:none -fill white -draw "circle 29,7 29,0" \) -compose dst-out -composite \
  \( -size 36x36 xc:none -fill black -draw "circle 29,7 29,2" \) -compose over -composite \
  crates/loadout/assets/tray/menubar-update@2x.png
echo "Wrote crates/loadout/assets/tray/menubar@2x.png and menubar-update@2x.png"
