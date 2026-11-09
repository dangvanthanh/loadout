# Loadout — notes for Pi Harness (and other coding agents)

Loadout is a minimal macOS menu bar app written in **Rust** with **GPUI** (Zed's UI framework) and **GPUI Kit**
(Longbridge's component library). Its job: **put the right skills into the project you're in, one keystroke away.**

⌃⌥L (or a Dock click) opens a palette that detects the frontmost project and its stack, ranks and pre-selects matching favorite skills
(and loadouts), and installs them with `npx skills add`. It can optionally launch the preferred agent, show a diff
against a loadout, track calm updates via the GitHub trees API, and keep a local provenance log. The menu bar
icon opens a small menu; everything else is managed in the Settings window.

Stay inside that positioning. Don't add marketplace browsing, skill editing, enable/disable, usage stats, or sync —
competitors do those (see `docs/COMPETITORS.md`). If a request drifts there, say so and ask first.

Read before changing anything:
- `docs/PRODUCT.md` — what we're building and why, and its unique features
- `docs/COMPETITORS.md` — what others already do, and what we leave to them
- `docs/ARCHITECTURE.md` — crates, modules, data model, flows, rules
- `docs/DESIGN.md` — UI rules (minimal, theme colors only, one primary button)
- `docs/PLAN.md` — how to add work (the original M0–M5 plan is done); `docs/RELEASE.md` — release steps
- `docs/DECISIONS.md` — past choices; add an entry for new ones
- `reference/swift-prototype/` — earlier SwiftUI version (old name SkillDock). Port behavior from it
  (parser cases, command format, update logic). Don't edit it.

## Commands

```bash
cargo build                                   # whole workspace
cargo test -p loadout-core                    # fast logic tests (run these constantly)
cargo clippy --all-targets -- -D warnings     # must pass
cargo fmt                                     # format
cargo run -p loadout                      # quick UI run (unbundled: no notifications/login item)
packaging/bundle.sh --install                 # release .app → /Applications, relaunch
pkill -x loadout; pkill -x Loadout        # stop a running copy
```

## How to work

1. Take the change the user names, or the first unchecked task in `docs/PLAN.md` / `docs/RELEASE.md`.
2. Read the files it touches. For GPUI/GPUI Kit APIs, check the pinned version's docs on docs.rs and GPUI Kit's
   examples before writing code — APIs change between releases. Don't guess method names.
3. Implement. Put logic in `loadout-core` with tests; keep `loadout` thin.
4. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p loadout-core`, `cargo build`.
   Fix everything.
5. Update docs: tick the task with the date, update ✅/⬜ in `PRODUCT.md`, adjust `ARCHITECTURE.md` if structure changed,
   add a `DECISIONS.md` entry for any new dependency or design choice.
6. Tell the user exactly what to check by hand (the task's **Accept** line). You can't see the screen.
7. Commit: `T<id>: <short title>`.

## Code rules

- **Core vs app:** `loadout-core` has no GPUI dependency. Parsing, commands, HTTP, hashing, storage, diff, safety,
  stack/context detection, provenance, loadout diff live there and are unit-tested. `loadout` holds UI, the
  `AppState` entity, and macOS glue.
- **State:** one `AppState` GPUI entity. Views call its methods; views never spawn processes, do HTTP, or touch files.
- **Threads:** blocking work (HTTP via `ureq`, `std::process`) runs on GPUI's background executor; results are applied
  on the foreground and followed by `cx.notify()`. No tokio.
- **Persistence:** never rename or remove a JSON key. Every field has `#[serde(default)]`. camelCase keys
  (compatible with the prototype's `data.json`). Atomic writes. Provenance is append-only.
- **Shell:** every user value goes through `installer::shq`. Always pass `-y`. Stdin null.
  **Never run `npx skills check` or `npx skills update`** — they change files.
- **Errors:** `thiserror` in core, `anyhow` in app. No `unwrap()`/`expect()` outside tests and startup invariants.
- **Dependencies:** only those listed in `ARCHITECTURE.md`. Adding one needs a `DECISIONS.md` entry and the user's OK.
- **macOS glue:** `objc2` family crates (plus the small Accessibility `extern "C"` block over `core-foundation`), only in `loadout/src/macos/`
  and `tray.rs`. Keep `unsafe` blocks small, each with a `// SAFETY:` comment.
- **UI:** follow `docs/DESIGN.md`. GPUI Kit components first, theme colors only (no hex), sentence case,
  no "please"/"successfully"/"!". The palette is keyboard-first. If a change adds a visible button, ask whether it
  could live in a menu or a key shortcut instead.

## Gotchas

- GPUI needs full Xcode for Metal shader compilation (DECISIONS 017). If the build fails at a `metal` step, run
  `xcode-select -p` and check Xcode is selected, not just Command Line Tools.
- GPUI Kit pins its GPUI release; don't add a separate `gpui` dependency that could mismatch.
- Notifications and `SMAppService` only work from the bundled `.app` (`packaging/bundle.sh`), not `cargo run`.
- Read the frontmost app **before** showing the palette, or you'll detect Loadout itself.
- Finder detection needs Automation permission; editor detection needs Accessibility. Both prompts only appear in the
  bundled app. Always have a fallback (most recent project).
- Context signals are best-effort. Never block the palette if they are unavailable.
- Agent launch (equip + launch) is optional and must degrade gracefully when the preferred agent is not installed.
