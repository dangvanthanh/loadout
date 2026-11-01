# Loadout — Product spec

## One line

**The right skills for the project you're in, one keystroke away.**

_Built with Rust, GPUI and GPUI Kit. macOS 13+._

## Positioning

Other skill managers are libraries: big windows to browse, sync, and edit everything you own (see `COMPETITORS.md`).
Loadout is the opposite: a tiny menu bar tool for one moment, **"I'm starting work in this project; give it the skills it needs."**

- Press **⌃⌥L** anywhere.
- Loadout already knows which project you're in (the Finder window or editor in front) and what it's built with.
- It shows your favorite skills with the right ones pre-selected and ranked for that stack and recent context.
- Press Return. Done. Optionally launch your preferred agent. Later, it tells you calmly when a skill you use really changed, and shows you what changed.

Who it's for: developers who use coding agents (Pi, Claude Code, Cursor, Codex, …) across several projects, keep a personal set of go-to skills, and don't want another dashboard.

## Principles

1. **Where you are.** Act on the project in front of you. Never make the user pick a folder they're already looking at.
2. **Minimal.** One small window, one primary button. Everything else lives in a menu or a right-click.
3. **Calm.** No badges for README edits, no auto-updates, no nagging. Files change only when you press a button.
4. **Honest.** Show what will run, what will change, and what a skill contains before it lands in your project.
5. **Built on the ecosystem.** Installs go through the official `npx skills` CLI; versions come from GitHub.
   No account, no server, no telemetry.
6. **Yours.** Favorites, loadouts, and provenance are plain JSON you can read, back up, and share.

## The unique features (the reason to exist)

| # | Feature | What it does |
|---|---|---|
| U1 | **Project you're in** | The palette targets the frontmost Finder folder, or the project open in your editor (Zed, VS Code, Cursor, Xcode via the Accessibility "document" attribute), walking up to the nearest `.git` / `package.json` / `Cargo.toml`. Falls back to the last project. |
| U2 | **Stack-aware + context-weighted suggestions** | Detects the stack (Next.js, React, Vue, Svelte, Expo, Tailwind, SwiftUI, Rust, Python, Go …). Favorites carry stack tags (auto-suggested from the skill's name and description). Matching favorites are pre-selected and ranked higher when they also match recent files, open editors, or git status signals. Label: "Suggested for Next.js + Tailwind". |
| U3 | **Calm, honest updates** | Tracks each skill's own folder on GitHub, so README or other-skill changes never count. Before updating: a file list and a `SKILL.md` diff. Pin a version, or ignore one update. |
| U4 | **Safety + compatibility glance** | Before a skill first lands in any project: flags scripts, `curl … \| sh`, `rm -rf`, `sudo`, big encoded blobs, unknown hosts. Also notes simple compatibility signals (skills CLI version, Node version hints, agent folder requirements). Advice, never a block. |
| U5 | **Agent-aware loadouts** | Named sets of skills ("Web app", "iOS app") that can declare preferred agents (Pi, Claude Code, Cursor, Codex, …). Apply to a project in one keystroke. When applied, installs into the correct agent folders and can optionally launch the preferred agent. |
| U6 | **Official CLI underneath** | Every install is `npx skills add … -y`, so lock files and 30+ agent folder layouts stay exactly as the ecosystem expects. Paste any `npx skills add` command to save its skills as favorites. |
| U7 | **Diff against loadout** | One-click (or keyboard) view of what the current project is missing versus a named loadout or the suggested one. Makes gaps obvious before you install. |
| U8 | **Skill provenance + audit** | Lightweight local log of what was installed/updated/removed, when, from which commit/tree, into which project, and by which action (palette, loadout apply, update). Plain JSON; useful for personal history and light team trust. |
| U9 | **One-shot equip + launch** | After installing (or applying a loadout), optionally launch the preferred agent for that loadout/project with a single extra key or checkbox. |
| U10 | **Shared team loadouts with versioning** | Beyond one-off `loadout://` links and `.loadout.json` files: versioned team loadouts (simple git-backed or signed snapshot) that still install through the official CLI. Import stays one click. |
| U11 | **Idle “what changed” digest** | Optional quiet daily/weekly notification summarizing real skill updates across your projects. Still never noisy; only surfaces changes that would have raised an Update pill. |

## Non-goals (deliberate)

- Marketplace browsing or search (skills.sh does it; paste a link from there).
- Writing or editing skills.
- Enabling/disabling, usage stats, cross-agent sync, Git backup (Skills Manager and SkillDeck do these well).
- Accounts, cloud servers, telemetry.
- Windows / Linux builds in v1 (GPUI supports them, but U1, tray, and notifications are macOS code).
- Mac App Store (sandboxing blocks `npx` in project folders).

## Feature list and status

✅ done in the Rust app · ⬜ to build. Milestones refer to `PLAN.md`.
Basic flows exist in the SwiftUI prototype (`reference/swift-prototype/`, built under the old name SkillDock).

### M0 — Foundation
- ✅ GPUI + GPUI Kit app, menu bar icon, popup under the icon, no Dock icon
- ✅ Global hotkey ⌃⌥L opens a palette window
- ✅ Frontmost Finder/editor folder detection (spike for U1)
- ✅ App bundle, notifications, open at login

### M1 — Core library (no UI, fully tested)
- ✅ Model + storage, paste parser, installer, update checker
- ✅ Project root finder + stack detector (U1, U2)
- ✅ Diff, safety + compatibility scanner (U3, U4)
- ✅ Provenance log model (U8)

### M2 — The loop: "equip this project" (U1, U2, U6)
- ⬜ Palette: detected project + stack, favorites with suggestions pre-selected and ranked, Return installs
- ⬜ Favorites: paste to add (any format), tags, rename, remove
- ⬜ Installed state per project (incl. skills installed before Loadout)
- ⬜ Context signals for ranking (recent files / open editors when available)

### M3 — Calm updates and trust (U3, U4, U8)
- ⬜ Update check, Update pill, notification, filled menu bar icon
- ⬜ What changed (files + SKILL.md diff), pin, ignore
- ⬜ Safety + compatibility glance on first install; skill preview
- ⬜ Provenance entries written on install/update/remove

### M4 — Loadouts and agent awareness (U5, U7, U9)
- ⬜ Create, apply, manage loadouts (including preferred agent)
- ⬜ Diff against loadout
- ⬜ One-shot equip + launch
- ⬜ Share and import via link or file

### M5 — Team, digest, polish and release
- ⬜ Shared team loadouts with versioning (U10)
- ⬜ Idle “what changed” digest (U11)
- ⬜ Popup window parity (projects menu, Global target, settings)
- ⬜ Uninstall from a project, GitHub token in Keychain, keyboard navigation
- ⬜ Icon, signed + notarized universal release, Homebrew cask

## Screens

### Palette (⌃⌥L) — the main surface, 520 pt wide

```
┌──────────────────────────────────────────────────────┐
│ ⌕ Search favorites and loadouts                      │
│ Into: shop-dashboard · Next.js, Tailwind   from Zed ⌄│
├──────────────────────────────────────────────────────┤
│ Suggested                                            │
│ ◉ frontend-design      anthropics/skills             │
│ ◉ vercel-react-best-practices   vercel-labs/…     ✓  │
│ Loadouts                                             │
│ ○ Web app · 4 skills · prefers Pi                    │
│ Other favorites                                      │
│ ○ tdd                  mattpocock/skills             │
│ ○ swiftui-patterns     …                             │
├──────────────────────────────────────────────────────┤
│ ↩ Install 1 skill   ⌘↩ Install + launch   esc        │
└──────────────────────────────────────────────────────┘
```

- Typing filters. ↑↓ move, Space toggles, Return installs selected (already-installed ones are skipped).
- ⌘↩ (or a small checkbox) = Install + launch preferred agent when a loadout declares one.
- "Into" line: click (or ⌘P) to switch project or choose Global.
- Section labels here are the one exception to "no section titles": they explain *why* something is selected.
- Diff against loadout is available from the loadout row (right-click or ⌘D).

### Menu bar popup — 340 pt, for glancing and managing

```
┌────────────────────────────────────┐
│ shop-dashboard ⌄            ↻   ⚙ │
│ ┌────────────────────────────────┐ │
│ │ + Paste a link or owner/repo   │ │
│ └────────────────────────────────┘ │
│ ◉ frontend-design         Update   │
│   anthropics/skills                │
│ ○ skill-creator               ✓    │
│ • 1 update available   Update all  │
│ [        Install 1 skill         ] │
│ Checked 4 minutes ago          Log │
└────────────────────────────────────┘
```

### Row interactions (both surfaces)
Click: toggle. Click name: preview. Right-click: Install only this / Update · What changed · Diff against loadout · Pin · Tags… · Copy install command · Rename… · Remove · View provenance.

## Copy rules

Sentence case. No "please", "successfully", or "!". Errors say what happened and what to do.
Examples: "Installed 2 skills in shop-dashboard" · "Couldn't install tdd. See log." ·
"GitHub limit reached. Try again in an hour." · "Suggested for Next.js" · "Missing 3 skills vs Web app loadout"

## Success measures (personal use first)

- Equipping a new project with the right skills: **one keystroke + Return, under 5 seconds.**
- Adding a favorite: one paste.
- Every real skill update is noticed within 6 hours; zero alerts for README-only changes.
- Diff against a loadout is readable in under 2 seconds.
- Idle cost: under 80 MB memory, ~0% CPU.
