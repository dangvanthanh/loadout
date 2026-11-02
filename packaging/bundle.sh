#!/bin/bash
# Builds target/Loadout.app.
#   packaging/bundle.sh               native release build, signed ad hoc
#   packaging/bundle.sh --install     also copy to /Applications and relaunch
#   packaging/bundle.sh --universal   Apple Silicon + Intel in one binary (adds the Rust target once)
#   packaging/bundle.sh --release     universal, Developer ID signed, notarized, stapled, zipped
#     needs LOADOUT_SIGN_ID       e.g. "Developer ID Application: Your Name (TEAMID)"
#           LOADOUT_NOTARY_PROFILE  a profile saved with `xcrun notarytool store-credentials`
#     prints the zip's sha256 for packaging/homebrew/loadout.rb
set -euo pipefail
cd "$(dirname "$0")/.."

APP="target/Loadout.app"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' packaging/Info.plist)"
INSTALL=0 UNIVERSAL=0 RELEASE=0
for arg in "$@"; do
  case "$arg" in
    --install) INSTALL=1 ;;
    --universal) UNIVERSAL=1 ;;
    --release) RELEASE=1 UNIVERSAL=1 ;;
    *) echo "Unknown option: $arg" >&2; exit 2 ;;
  esac
done

if [[ $RELEASE == 1 ]]; then
  : "${LOADOUT_SIGN_ID:?Set LOADOUT_SIGN_ID to your Developer ID Application identity}"
  : "${LOADOUT_NOTARY_PROFILE:?Set LOADOUT_NOTARY_PROFILE (xcrun notarytool store-credentials)}"
fi

if [[ $UNIVERSAL == 1 ]]; then
  TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
  rustup target add "${TARGETS[@]}" >/dev/null
  for target in "${TARGETS[@]}"; do
    cargo build --release -p loadout --target "$target"
  done
  BINARY="target/loadout-universal"
  lipo -create -output "$BINARY" \
    target/aarch64-apple-darwin/release/loadout target/x86_64-apple-darwin/release/loadout
else
  cargo build --release -p loadout
  BINARY="target/release/loadout"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/Loadout"
cp packaging/Info.plist "$APP/Contents/Info.plist"
cp packaging/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

if [[ $RELEASE == 1 ]]; then
  codesign --force --timestamp --options runtime \
    --entitlements packaging/entitlements.plist --sign "$LOADOUT_SIGN_ID" "$APP"
  codesign --verify --strict --verbose=2 "$APP"
  ZIP="target/Loadout-$VERSION.zip"
  rm -f "$ZIP"
  ditto -c -k --keepParent "$APP" "$ZIP"
  xcrun notarytool submit "$ZIP" --keychain-profile "$LOADOUT_NOTARY_PROFILE" --wait
  xcrun stapler staple "$APP"
  # Zip again so the download carries the stapled ticket.
  rm -f "$ZIP"
  ditto -c -k --keepParent "$APP" "$ZIP"
  spctl --assess --type execute --verbose "$APP"
  echo "Built $ZIP"
  echo "version $VERSION"
  echo "sha256  $(shasum -a 256 "$ZIP" | cut -d' ' -f1)"
else
  codesign --force --sign - "$APP"
  echo "Built $APP ($(lipo -archs "$APP/Contents/MacOS/Loadout"))"
fi

if [[ $INSTALL == 1 ]]; then
  pkill -x Loadout 2>/dev/null || true
  pkill -x loadout 2>/dev/null || true
  rm -rf /Applications/Loadout.app
  cp -R "$APP" /Applications/
  open /Applications/Loadout.app
  echo "Installed to /Applications and launched. Look for the octopus in the menu bar."
fi
