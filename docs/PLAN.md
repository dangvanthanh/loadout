# Loadout — Implementation plan (Rust + GPUI)

Ordered so the **unique features come first** (see `PRODUCT.md` U1–U11 and `COMPETITORS.md`).
After M2 you have a usable app that no competitor matches: one keystroke equips the project you're in with the right
skills. Agent awareness, diff-against-loadout, provenance, team loadouts, and the digest come in later milestones.

Each task is sized for one Pi Harness session. Mark a task done by changing `[ ]` to `[x]` and adding the date.
Don't start a milestone until the previous one's checklist passes.

Definition of done for every task:
1. `cargo build` succeeds; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass.
2. `cargo test -p loadout-core` passes, with new tests for any new core logic.
3. The task's **Accept** criteria are checked by hand (app tasks: in the bundled app via `packaging/bundle.sh --install`).
4. Docs updated: tick the task, update ✅/⬜ in `PRODUCT.md`, adjust `ARCHITECTURE.md` / `DECISIONS.md` if needed.
5. One commit per task: `T2.1: palette with suggestions`.

`reference/swift-prototype/` (old name SkillDock) already implements the parser, command format, and update logic.
Port its behavior; don't copy its structure.

---

## M0 — Foundation and risk spikes

Answer the questions that could change the architecture before writing features.

### [x] T0.1 Workspace and hello window (2026-10-31)
- Prerequisites: Rust stable via rustup, full Xcode selected (GPUI compiles Metal shaders). If the Metal step fails,
  try GPUI's `runtime_shaders` feature and record the result in `DECISIONS.md`.
- Create the workspace: `Cargo.toml`, `rust-toolchain.toml`, `crates/loadout-core`, `crates/loadout`.
- Add `gpui-kit` at the latest crates.io version; follow its README for `init` and `open_window`.
- Show a 520×360 window with a GPUI Kit `Input` and `Button`, following the system light/dark appearance.
**Accept:** `cargo run -p loadout` opens the window; switching macOS appearance switches the theme.

### [x] T0.2 Global hotkey and palette window (spike for the main surface) (2026-10-31)
- `hotkey.rs` with the `global-hotkey` crate: ⌃⌥L by default. Poll its receiver from a GPUI timer.
- Open/close a borderless floating palette window centered on the active screen; focus its input; Esc and blur close it.
- Activation policy Accessory (no Dock icon).
**Accept:** from any app, ⌃⌥L shows the palette with the cursor in the search field; Esc closes it and focus returns
to the previous app.

### [x] T0.3 "Project you're in" detection (spike for U1) (2026-10-31)
- `macos/frontmost.rs`: get the frontmost app (`NSWorkspace.frontmostApplication`) *before* the palette takes focus.
- Finder: `NSAppleScript` → POSIX path of the front window's target (Automation permission prompt on first use).
- Editors (Zed, VS Code, Cursor, Xcode, JetBrains): read `AXDocument` / `AXTitle` of the focused window via the
  Accessibility API (`AXUIElement…` through `objc2` / `core-foundation`); turn a file URL into its folder. Prompt for
  Accessibility permission with a clear explanation.
- Record which apps work in `DECISIONS.md` (entry 012). Unsupported apps fall back to the last-used project.
**Accept:** with Finder, Zed, and VS Code each in front, the palette's "Into" line shows the right folder.

### [x] T0.4 Menu bar item, bundle, notifications, login item (2026-10-31)
- `tray.rs`: `NSStatusItem` with a template icon (objc2). Click toggles a 340 pt popup window under the icon; closes on blur.
  Check first whether a maintained GPUI tray crate matches GPUI Kit's GPUI version (DECISIONS 008).
- `packaging/Info.plist` (bundle id `local.loadout.app`, `LSUIElement`, `CFBundleURLTypes` for `loadout`,
  `NSAppleEventsUsageDescription`), `packaging/bundle.sh` (release build → `target/Loadout.app`, ad-hoc sign,
  `--install`).
- `macos/notify.rs` test notification; `macos/login_item.rs` with `SMAppService`.
**Accept:** bundled app has no Dock icon; menu bar click shows the popup under the icon on every display; a test
notification appears; open-at-login toggles in System Settings.

---

## M1 — Core library (`loadout-core`, no UI, fully tested)

### [x] T1.1 Model and storage (2026-10-31)
`model.rs`, `storage.rs`, `agents.rs`. Atomic save, `#[serde(default)]` everywhere, camelCase, corrupt-file recovery.
Include `Loadout.preferred_agent`, `ProvenanceEntry`, digest settings.
Tests: `{}`, a prototype `data.json` fixture (imported from `~/Library/Application Support/SkillDock/` if present),
round-trip, corrupt file.
**Accept:** `cargo test -p loadout-core storage` green.

### [x] T1.2 Paste parser (2026-10-31)
`parser.rs` + `ui_skills.rs` (port `SourceParser.swift`). At least 15 cases: owner/repo, `@skill`, GitHub repo/tree/`.git`
URLs, gitlab, skills.sh repo/skill/site and reserved paths, `npx skills add` with quoted names and multiple `--skill`,
ui-skills from a saved HTML fixture.
**Accept:** parser tests green.

### [x] T1.3 Installer (2026-10-31)
`installer.rs`: `shq`, `agent_list`, `install_command`, `remove_command`, `run(cmd, dir, on_line) -> exit code` with
streaming and ANSI stripping. Missing `npx` → typed error.
**Accept:** installer unit tests green; a manual dry-run against a temp dir works.

### [x] T1.4 Project root + stack detector (U1, U2) (2026-10-31)
`project.rs`, `stack.rs`. Walk up for `.git` / `package.json` / `Cargo.toml` / etc. Detect common stacks with evidence.
**Accept:** fixture projects produce the expected root and stack tags.

### [x] T1.5 Context signals + ranking (U2) (2026-10-31)
`context.rs`, extend `tags.rs`. Lightweight signals (recent files when available, simple git status presence). Rank
matching favorites higher; keep the ranking deterministic and tested.
**Accept:** ranking tests show stack matches first, then context boosts, then others.

### [x] T1.6 Updates, diff, safety + compatibility (U3, U4) (2026-10-31)
`updates.rs`, `diff.rs`, `safety.rs`. Tree SHA of the skill folder only. SKILL.md + file list diff. Heuristic safety
scan plus simple compatibility notes.
**Accept:** tests cover README-only (no update), real folder change (update), and several safety patterns.

### [x] T1.7 Provenance (U8) (2026-10-31)
`provenance.rs`. Append-only entries on install/update/remove/apply. Persist in `SavedState`. Simple query helpers.
**Accept:** provenance tests green; entries survive round-trip.

---

## M2 — The loop: "equip this project" (U1, U2, U6)

### [x] T2.1 Palette with project + stack + ranked suggestions (2026-10-31)
Palette shows Into line, Suggested / Loadouts / Other sections, keyboard navigation. Pre-select and rank by stack + context.
**Accept:** ⌃⌥L on a Next.js project pre-selects the right favorites; ranking looks sensible.

### [x] T2.2 Favorites management (2026-10-31)
Paste to add (any supported format), tags, rename, remove. Persist immediately.
**Accept:** paste several formats; they appear and survive relaunch.

### [x] T2.3 Install from palette (U6) (2026-10-31)
Return installs selected skills via official CLI into the target project. Skip already-installed. Stream output to log.
Write provenance.
**Accept:** installing a known skill creates the expected agent folders / lock entries; provenance has an entry.

### [x] T2.4 Installed state (2026-10-31)
Detect skills already present (lock file + agent folders) so the palette shows checkmarks and skips them.
**Accept:** previously installed skills show as installed without Loadout having done the install.

### [x] T2.5 Context signals in the live app (2026-10-31)
Wire the context module into the palette ranking path (best-effort; graceful when signals are unavailable).
**Accept:** with an editor open on relevant files, ranking improves vs stack-only.

---

## M3 — Calm updates and trust (U3, U4, U8)

### [x] T3.1 Update check + UI (2026-11-01)
Background check, Update pill, filled menu bar icon, notification when real updates exist.
**Accept:** changing a skill folder on GitHub surfaces an update; README-only does not.

### [x] T3.2 What changed + pin / ignore (2026-11-01)
Show file list + SKILL.md diff before updating. Pin version or ignore this update.
**Accept:** user can see the diff, pin, and ignore; behavior is persisted.

### [x] T3.3 Safety + compatibility glance + preview (2026-11-01)
On first install of a skill, show findings. Preview skill content.
**Accept:** a skill with a `curl | sh` pattern shows a warning; install is still allowed.

### [x] T3.4 Provenance visible (2026-11-01)
Simple log view in the popup (or a dedicated sheet). Filter by project optional.
**Accept:** recent installs appear; data matches what was written.

---

## M4 — Loadouts and agent awareness (U5, U7, U9)

### [x] T4.1 Create / apply / manage loadouts (2026-11-01)
Named sets, preferred agent field, apply in one action. Suggest loadouts by stack tags.
**Accept:** create "Web app", apply it; preferred agent is stored.

### [x] T4.2 Diff against loadout (U7) (2026-11-01)
From a loadout row or keyboard: show missing / extra / present skills for the current project.
**Accept:** gaps are clear; user can select missing ones and install.

### [x] T4.3 One-shot equip + launch (U9) (2026-11-01)
⌘↩ or checkbox: after install/apply, launch the preferred agent (Pi, Cursor, …) via configured command / URL scheme.
**Accept:** applying a loadout with preferred agent Pi opens Pi (or the configured launcher).

### [x] T4.4 Share and import (2026-11-01)
`loadout://` links and `.loadout.json` files. Import applies through the same path.
**Accept:** export → import on a clean machine brings the loadout across.

---

## M5 — Team, digest, polish and release

### [ ] T5.1 Shared team loadouts with versioning (U10)
Version field, optional source/signature, import of versioned snapshots. Still installs via official CLI.
**Accept:** a versioned loadout file imports cleanly and records version in provenance/settings.

### [ ] T5.2 Idle “what changed” digest (U11)
Quiet daily/weekly summary notification when real updates exist. Setting to enable/disable and set interval.
**Accept:** with updates pending, a single calm digest appears; no spam.

### [ ] T5.3 Popup parity + settings
Projects menu, Global target, settings panel (hotkey, agents, digest, launch default, GitHub token).
**Accept:** all settings persist and affect behavior.

### [ ] T5.4 Uninstall, token, keyboard polish
Uninstall from a project, Keychain token, full keyboard coverage, accessibility labels.
**Accept:** uninstall works; token is used for higher GitHub rate limits.

### [ ] T5.5 Release
Icon, signed + notarized universal binary, Homebrew cask formula, final docs pass.
**Accept:** downloadable `.app` / cask installs cleanly on a fresh Mac.

---

## Working with Pi Harness

See `AGENTS.md` and `docs/PI_HARNESS_GUIDE.md`.

Typical loop:
1. `/next-task` (or name the task).
2. Implement, test, update docs.
3. You verify the Accept checklist.
4. Commit, then clear context before the next task.
