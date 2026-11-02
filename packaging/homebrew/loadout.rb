# Homebrew cask for Loadout. Fill in version and sha256 from `packaging/bundle.sh --release`,
# upload target/Loadout-<version>.zip to the GitHub release, then copy this into a tap
# (e.g. dangvanthanh/homebrew-tap/Casks/loadout.rb).
cask "loadout" do
  version "0.1.0"
  sha256 "REPLACE_WITH_SHA256_FROM_BUNDLE_SH"

  url "https://github.com/dangvanthanh/loadout/releases/download/v#{version}/Loadout-#{version}.zip"
  name "Loadout"
  desc "Menu bar app that puts the right agent skills into the project you're in"
  homepage "https://github.com/dangvanthanh/loadout"

  depends_on macos: ">= :ventura"

  app "Loadout.app"

  uninstall quit: "local.loadout.app"

  zap trash: [
    "~/Library/Application Support/Loadout",
    "~/Library/Caches/Loadout",
  ]

  caveats <<~EOS
    Loadout installs skills with `npx skills add`, so it needs Node.js:
      brew install node
  EOS
end
