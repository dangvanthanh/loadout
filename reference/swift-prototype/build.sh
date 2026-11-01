#!/bin/bash
# Builds SkillDock.app without opening Xcode.
# Needs: macOS 13+ and Xcode Command Line Tools (xcode-select --install).
set -euo pipefail
cd "$(dirname "$0")"

APP="build/SkillDock.app"
ARCH="$(uname -m)"   # arm64 on Apple Silicon, x86_64 on Intel

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"

echo "Compiling for $ARCH…"
xcrun swiftc \
  -O \
  -swift-version 5 \
  -sdk "$(xcrun --show-sdk-path --sdk macosx)" \
  -target "${ARCH}-apple-macos13.0" \
  Sources/*.swift \
  -o "$APP/Contents/MacOS/SkillDock"

cp Info.plist "$APP/Contents/Info.plist"
codesign --force --sign - "$APP" >/dev/null 2>&1 || true

echo "Built $APP"

if [[ "${1:-}" == "--install" ]]; then
  pkill -x SkillDock 2>/dev/null || true
  rm -rf /Applications/SkillDock.app
  cp -R "$APP" /Applications/
  open /Applications/SkillDock.app
  echo "Installed to /Applications and launched. Look for the stack icon in your menu bar."
else
  echo "Run it:      open $APP"
  echo "Or install:  ./build.sh --install"
fi
