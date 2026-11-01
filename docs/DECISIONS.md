# Loadout — Decision log

Short records of choices that shaped the app. Add a new entry when you make a choice someone might later question.

## 001 — Wrap the `skills` CLI instead of copying files ourselves
**Choice:** Every install runs `npx skills add … -y`.
**Why:** The CLI already knows 30+ agents' folder layouts, symlink vs copy, and lock files. Competitors reimplement
this; staying on the official path is part of our positioning (U6).
**Cost:** Needs Node.js. First run is slower while `npx` downloads the CLI.

## 002 — Don't use the skills.sh API
**Choice:** Read versions from GitHub, not skills.sh.
**Why:** The skills.sh API authenticates with a Vercel OIDC token, which a desktop app doesn't have.
**Cost:** Only GitHub-hosted skills get update tracking.

## 003 — Never run `npx skills check` / `npx skills update`
**Choice:** Detect updates ourselves; update by re-running `skills add`.
**Why:** At least one CLI guide notes that `check` updates skills instead of only checking. Files change only on a click.

## 004 — Version = git tree hash of the skill's folder
**Choice:** Use the folder's tree `sha` from the GitHub trees API.
**Why:** Changes only when that folder's files change, so README edits never raise an update (U3). One request per repo.
**Cost:** Skills are matched to folders by folder name first, frontmatter `name:` second.

## 005 — Rust + GPUI + GPUI Kit (replaces SwiftUI)
**Choice:** Rust with GPUI for rendering and GPUI Kit for components.
**Why:** Owner's preference; GPUI is fast and proven in Zed; GPUI Kit provides 75+ components with themes; core logic
becomes a testable library.
**Cost:** No built-in menu bar API (see 008); GPUI's API moves, so pin versions; builds need full Xcode for Metal.
**Kept:** SwiftUI prototype in `reference/swift-prototype/` as a behavior reference.

## 006 — Two crates: pure core + GPUI app
**Why:** Most bugs live in parsing, detection, commands, hashing, and storage. UI-free core = fast tests that the agent
can run without launching the app.

## 007 — No async runtime
**Why:** GPUI has executors; `ureq` + `std::process` on the background executor is enough.

## 008 — Own menu bar item via objc2
**Choice:** `tray.rs` creates an `NSStatusItem` and positions a GPUI popup under it.
**Why:** GPUI has no tray API; community `gpui-tray` crates are early and must match GPUI Kit's exact GPUI flavor.
**Revisit:** at T0.4, and whenever GPUI Kit adds tray support.
**T0.4 result (2026-10-31):** kept our own. `gpui-tray` 0.3.0 exists but is young and doesn't expose the icon's frame,
which we need to place the popup. `tray.rs` is ~90 lines: an `NSStatusItem` whose button target sets a flag that the
50 ms hotkey poll drains. The popup is a `WindowKind::PopUp` panel centered under the icon frame (display-relative).
A click that blurs the open popup is ignored for 400 ms so it doesn't reopen it. On macOS 27 the item is drawn by
`MenuBarAgent`, but `button.window().frame()` still reports the icon's position.

## 009 — Menu bar only, no sandbox, plain JSON
**Why:** A 3-second utility; sandboxing blocks `npx` in project folders; JSON is easy to back up, diff, and share.

## 010 — Narrow positioning: "equip the project you're in"
**Choice:** Lead with project detection, stack + context suggestions, calm updates, safety, agent-aware loadouts,
diff-against-loadout, provenance, and shareable/versioned loadouts. Leave marketplace browsing, editing, enable/disable,
usage stats, and sync to other apps.
**Why:** October 2026 research (`COMPETITORS.md`) found at least five free skill managers. The leader, Skills Manager
(~6.6k ★), already does library management, presets, upstream compare, and sync across 56 agents. Competing there is
a losing race; the "where you are, right now" flow is unclaimed.
**Consequence:** The palette, not the popup, is the main surface. The plan builds U1/U2 first (M0–M2) and moves
popup parity and team features to M5.

## 011 — Rename SkillDock → Loadout; hotkey ⌃⌥L
**Choice:** Product name "Loadout"; a saved set of skills is "a loadout"; URL scheme `loadout://`; files `.loadout.json`;
default hotkey ⌃⌥L.
**Why:** "SkillDock" was one letter from SkillDeck and Skill Deck. "Loadout" (the gear you pick before a mission)
describes the product and no agent-skills tool used it in October 2026 searches. SkillsBar already uses ⌥⇧S.
**Todo before public release:** trademark and App Store/Homebrew name check.

## 012 — How we find "the project you're in"
**Choice:** Finder via `NSAppleScript`; editors via the Accessibility `AXDocument` attribute of the focused window;
fallback to the most recent project.
**Why:** No permissions-free API exposes another app's open folder. These two cover Finder and most editors with
standard macOS permission prompts.
**Open:** T0.3 records which editors expose `AXDocument` (Zed, VS Code, Cursor, Xcode, JetBrains) and any per-app
fallbacks (e.g. parsing the window title).
**T0.3 result (2026-10-31):**
- Finder runs through `/usr/bin/osascript` on the background executor instead of `NSAppleScript`, which is
  main-thread only and would stall the palette. TCC attributes the Automation prompt to Loadout (the responsible
  process). `-1743` in stderr = not allowed → the Into line says what to do.
- Every other app goes through one generic path: `AXFocusedWindow` (or `AXMainWindow`) → `AXDocument` → file URL →
  its folder. No per-app code. A small `extern "C"` block plus `core-foundation` 0.10 (already in GPUI's tree) is
  the AX FFI; no `accessibility-sys`. The system prompt shows once per launch.
- Expected to work: Zed (GPUI sets `representedFilename`), VS Code / Cursor (Electron sets it for the active
  file), Xcode, Terminal and Ghostty (proxy icon = working directory). JetBrains IDEs are unlikely to expose it.
  Not yet confirmed on a machine with the permissions granted; correct this list during the T0.3 hand check.
- The folder is the document's folder; walking up to the project root is T1.4.
- Fallback: the last detected folder for this session, labelled "last used" (persisted projects arrive with T1.1).
  No window-title parsing: titles give a project name, not a path.

## 013 — Primary development agent is Pi Harness
**Choice:** Docs, commands, and workflow are written for Pi Harness (`AGENTS.md`, `docs/PI_HARNESS_GUIDE.md`). Other agents
(Claude Code, Cursor, Codex) remain first-class *targets* of the product.
**Why:** Owner preference and existing skill/harness investment. The product itself stays agent-agnostic.

## 014 — Expand unique features beyond the original six
**Choice:** Add agent-aware loadouts (preferred agent), context-weighted ranking, diff against loadout, provenance/audit,
equip+launch, versioned team loadouts, and a calm idle digest.
**Why:** These deepen the "equip this project right now" job without turning Loadout into a library app. They create
clear differentiation and natural moments (diff, launch, team share) while staying minimal.
**Cost:** Slightly larger core model and a few extra modules; milestones M4–M5 absorb the work.

## 015 — Provenance is local, append-only, plain JSON
**Choice:** No server, no sync. Provenance lives in `data.json` (or a sibling file if it grows large).
**Why:** Matches the "yours, no account" principle. Enough for personal history and light team trust via shared files.

## 016 — Context signals are best-effort and optional
**Choice:** Ranking may use recent files, open editors, or simple git signals when available; otherwise fall back to
stack tags only. Never block the palette on missing context.
**Why:** Keeps the core loop fast and permission-light. Context is a ranking boost, not a requirement.

## 017 — Pin GPUI Kit 0.8.1; build Metal shaders ahead of time
**Choice:** `gpui-kit = "=0.8.1"` (brings `gpui-pre` 0.3.10), Rust edition 2024, stable toolchain. No separate `gpui`
line. GPUI's `runtime_shaders` feature isn't needed: the Metal step builds with full Xcode selected.
**Why:** GPUI Kit is the single facade for GPUI and its components (`gpui_kit::*`, `gpui_kit::component`). An exact
pin keeps the GPUI flavor stable for the objc2 glue and any future tray crate.
**Note:** GPUI Kit doesn't follow system appearance by itself; each window calls `Theme::sync_system_appearance`
from `observe_window_appearance`.

## 018 — The palette is a non-activating panel
**Choice:** The palette opens as GPUI `WindowKind::PopUp` (an `NSPanel` with `NonactivatingPanel`, popup level),
titlebar `None`, on the screen AppKit calls `mainScreen` (the one with the frontmost app's key window). Esc and blur
remove the window; the app policy is `Accessory`. ⌘Q quits while the palette has focus (no menu bar).
**Why:** Like Spotlight, the panel takes keyboard focus without activating Loadout, so the previous app stays
frontmost: focus returns by itself on close, and T0.3 can read the frontmost app without racing our own activation.
The hotkey receiver is polled every 50 ms from a GPUI timer (no redraws).

## 019 — Notifications through GPUI; SF Symbol menu bar icon
**Choice:** Post notifications with GPUI's `cx.show_system_notification` (it wraps `UNUserNotificationCenter` and
skips posting when unbundled). No `objc2-user-notifications` dependency; `macos/notify.rs` waits for the digest
(T5.2). The menu bar icon is the SF Symbol `square.stack.3d.up` (filled variant for updates), like the prototype.
**Why:** Same behavior with less code and no PNG assets to keep in sync; SF Symbols are template images that follow
the menu bar's appearance.
**Note:** the T0.4 popup holds only the spike controls (open at login, test notification, quit). M5 (T5.3) replaces
it with the real popup.
