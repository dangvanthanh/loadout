#!/bin/bash
# Builds target/Loadout.app from a release build and signs it ad hoc.
#   packaging/bundle.sh             build only
#   packaging/bundle.sh --install   also copy to /Applications and relaunch
set -euo pipefail
cd "$(dirname "$0")/.."

APP="target/Loadout.app"

cargo build --release -p loadout

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp target/release/loadout "$APP/Contents/MacOS/Loadout"
cp packaging/Info.plist "$APP/Contents/Info.plist"
codesign --force --sign - "$APP"

echo "Built $APP"

if [[ "${1:-}" == "--install" ]]; then
  pkill -x Loadout 2>/dev/null || true
  pkill -x loadout 2>/dev/null || true
  rm -rf /Applications/Loadout.app
  cp -R "$APP" /Applications/
  open /Applications/Loadout.app
  echo "Installed to /Applications and launched. Look for the stack icon in the menu bar."
fi
