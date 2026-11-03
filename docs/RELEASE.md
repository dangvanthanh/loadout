# Loadout — Release

Formerly task T5.5 in `PLAN.md`: icon, signed + notarized universal binary, Homebrew cask, final docs pass.

**Accept:** a downloaded `Loadout-<version>.zip`, or `brew install --cask loadout`, installs and runs cleanly on a
fresh Mac, with no Gatekeeper warning.

## Status

| Item | State |
|---|---|
| App icon (`packaging/AppIcon.icns`) and menu bar icon | ✅ |
| Universal build (`packaging/bundle.sh --universal`, arm64 + x86_64) | ✅ built; Intel half not run (no Rosetta on the build Mac) |
| Release script (`packaging/bundle.sh --release`: sign, notarize, staple, zip, sha256) | ✅ written; never run |
| Hardened-runtime entitlements (`packaging/entitlements.plist`) | ✅ |
| Homebrew cask (`packaging/homebrew/loadout.rb`) | ✅ template; version and sha256 not filled |
| Docs | ✅ refreshed 2026-11-02 |
| Developer ID signature + notarization | ⬜ needs the owner's Apple account |
| GitHub release + published cask | ⬜ owner's call (publishing) |

## Before the first public release (owner)

These are decisions or accounts only the owner has. Do them in this order.

1. **Bundle id.** Replace `local.loadout.app` in `packaging/Info.plist` (and `SERVICE` in
   `crates/loadout/src/macos/keychain.rs`) with a real reverse-DNS id. Changing it later resets macOS permissions
   (Automation, Accessibility), the login item, and the saved GitHub token, so decide once.
2. **Name check.** Trademark, App Store, and Homebrew search for "Loadout" (DECISIONS 011).
3. **Version.** Set `CFBundleShortVersionString` / `CFBundleVersion` in `packaging/Info.plist` and `version` in
   the workspace `Cargo.toml`.
4. **Developer ID.** Create a "Developer ID Application" certificate (the build Mac has only an "Apple
   Development" one, which Gatekeeper rejects on other Macs), then save notarization credentials:
   ```bash
   xcrun notarytool store-credentials loadout-notary --apple-id … --team-id … --password …
   ```
5. **Build the release:**
   ```bash
   LOADOUT_SIGN_ID="Developer ID Application: Your Name (TEAMID)" \
   LOADOUT_NOTARY_PROFILE=loadout-notary \
   packaging/bundle.sh --release
   ```
   It prints `target/Loadout-<version>.zip`, the version, and the sha256.
6. **Publish.** Create a GitHub release `v<version>` and upload the zip. Put the version and sha256 into
   `packaging/homebrew/loadout.rb`, then copy it into a tap (e.g. `dangvanthanh/homebrew-tap/Casks/loadout.rb`).
7. **Clean-Mac test** (the Accept line): `brew install --cask loadout` on a Mac that never had Loadout; open it;
   allow Automation and Accessibility when asked; run through the hand checks below.

## Hand checks not yet done

Built and covered by core tests or rendered to images, but never exercised by hand in the bundled app:

- Recording a new palette shortcut in Settings → Shortcuts (and Reset).
- Settings → Interface → Show app icon: Dock only, Dock and menu bar; the Dock icon image for each app icon;
  clicking the Dock icon opens the palette.
- Saving a GitHub token through Settings → Integrations (Keychain prompt), then an update check with it.
- The digest notification (Settings → Notifications → Daily) and clicking it.
- A real `curl … | sh` skill showing the safety notice (only the scripts notice was seen live).
- ⌘↩ opening a real Terminal (Pi, Claude Code) or editor window.
- The save and open panels: Save file…, Import File…, Add Project….
- Opening a `loadout://` link from another app and a `.loadout.json` from Finder's Open With.
- The Intel slice of the universal build.
