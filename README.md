# Loadout

**The right skills for the project you're in, one keystroke away.**

Press ⌃⌥L anywhere. Loadout sees which project you're in (Finder or your editor), detects its stack, ranks and
pre-selects the favorite agent skills that fit, and installs them with the official `npx skills` CLI when you press Return.
It can optionally launch your preferred agent, show you what a loadout is missing, keep a local provenance log, tell you
calmly when a skill you use really changed, and check new skills for risky scripts and simple compatibility notes before
they land in your project.

A minimal macOS menu bar app built with **Rust**, **GPUI**, and **GPUI Kit**.

## Status

M0–M4 and most of M5 are built: the palette, favorites, calm updates, safety glances, loadouts with diff and
launch, sharing, team versions, the digest, settings, and uninstall. What's left for a public release is a
Developer ID-signed, notarized build and a published Homebrew cask ([DECISIONS 046](docs/DECISIONS.md)).

| Doc | What's in it |
|---|---|
| [Product](docs/PRODUCT.md) | Positioning, unique features U1–U11, screens, copy rules |
| [Competitors](docs/COMPETITORS.md) | Similar apps, feature overlap, what we leave to them |
| [Architecture](docs/ARCHITECTURE.md) | Crates, modules, data model, flows, dependencies |
| [Design](docs/DESIGN.md) | Palette, menu, and Settings UI rules for GPUI Kit |
| [Plan](docs/PLAN.md) | Milestones M0–M5, unique features first |
| [Decisions](docs/DECISIONS.md) | Why Rust + GPUI, the narrow positioning, the name, Pi Harness |
| [AGENTS.md](AGENTS.md) | Rules Pi Harness (and other agents) follow in this repo |

`reference/swift-prototype/` holds the earlier SwiftUI version (named SkillDock), used as a behavior reference.

## How it's different

Most skill managers are libraries for browsing, syncing, and editing everything you own. Loadout does one thing:
equip the project in front of you — with stack + context awareness, agent-aware loadouts, a clear diff against a
loadout, calm updates, safety/compatibility glances, and optional one-shot launch. It works well alongside a library
app like Skills Manager or SkillDeck.

## Unique features (summary)

- **U1** Project you're in (Finder / editor)
- **U2** Stack-aware + context-weighted suggestions
- **U3** Calm, honest updates (skill-folder tree SHA only)
- **U4** Safety + compatibility glance
- **U5** Agent-aware loadouts (preferred agent)
- **U6** Official `skills` CLI underneath
- **U7** Diff against loadout
- **U8** Skill provenance + audit
- **U9** One-shot equip + launch
- **U10** Shared team loadouts with versioning
- **U11** Idle “what changed” digest

## Using it

- **⌃⌥L** opens the palette for the project in front (Finder or your editor). Suggested skills are pre-selected.
  ↑↓ move · Space or Tab select · **Return** installs · **⌘↩** installs and opens your agent · ⌘Y previews a
  skill · ⌘S saves the selection as a loadout · ⌘D diffs against the highlighted loadout · ⌘P picks the project
  or Global · ⌘⌫ (twice) uninstalls · Esc closes.
- **The menu bar octopus** opens a small menu: status, Open Palette, Update All, Check for Updates…, Settings… (⌘,).
- **Settings** manages everything: add skills (paste a link, `owner/repo`, or `npx skills add …`), loadouts,
  projects, history, coding agents, shortcuts, notifications, and the app icon (menu bar, Dock, or both).
- Share a loadout from its right-click menu in Settings → Loadouts (link or `.loadout.json`); opening one shows a preview before anything
  changes.

The first time, macOS asks to let Loadout read Finder (Automation) and editor windows (Accessibility). Without
them Loadout falls back to the project you used last.

## Requirements

- macOS 13 or newer, Apple Silicon or Intel
- Rust (stable, via rustup) and full Xcode (GPUI compiles Metal shaders)
- Node.js (`brew install node`) — Loadout runs `npx skills add`

## Build

```bash
cargo run -p loadout                    # quick run for UI work (no notifications or login item)
packaging/bundle.sh --install           # build Loadout.app, copy to /Applications, launch
packaging/bundle.sh --universal         # Apple Silicon + Intel
cargo test -p loadout-core              # logic tests
```

Releases: `LOADOUT_SIGN_ID="Developer ID Application: …" LOADOUT_NOTARY_PROFILE=… packaging/bundle.sh --release`
signs, notarizes, staples, and zips, then prints the sha256 for `packaging/homebrew/loadout.rb`.

Your data lives in `~/Library/Application Support/Loadout/data.json`.
