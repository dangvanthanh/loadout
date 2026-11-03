# Loadout — Product spec

## One line

**The right skills for the project you're in, one keystroke away.**

_Built with Rust, GPUI and GPUI Kit. macOS 13+. Version 0.1.0; see `RELEASE.md` for what's left before release._

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
2. **Minimal.** One palette, one primary action. Everything else lives in ⌘K, a right-click, or Settings.
3. **Calm.** No badges for README edits, no auto-updates, no nagging. Files change only when you press a button.
4. **Honest.** Show what will run, what will change, and what a skill contains before it lands in your project.
5. **Built on the ecosystem.** Installs go through the official `npx skills` CLI; versions come from GitHub.
   No account, no server, no telemetry.
6. **Yours.** Favorites, loadouts, and provenance are plain JSON you can read, back up, and share.

## The unique features (the reason to exist)

| # | Feature | What it does |
|---|---|---|
| U1 | **Project you're in** | The palette targets the frontmost Finder folder, or the project open in your editor (Zed, VS Code, Cursor, Xcode, Terminal via the Accessibility "document" attribute), walking up to the nearest `.git` or manifest (`package.json`, `Cargo.toml`, …). Falls back to the last project; ⌘P picks another or Global. |
| U2 | **Stack-aware + context-weighted suggestions** | Detects the stack (Next.js, React, Vue, Svelte, Expo, Tailwind, SwiftUI, Rust, Python, Go …). Favorites carry stack tags (auto-suggested from the skill's name and description). Matching favorites are pre-selected and ranked higher when they also match recent files, open editors, or git status signals. Label: "Suggested for Next.js + Tailwind". |
| U3 | **Calm, honest updates** | Tracks each skill's own folder on GitHub, so README or other-skill changes never count. Before updating: a file list and a `SKILL.md` diff. Pin a version, or ignore one update. |
| U4 | **Safety + compatibility glance** | Before a skill first lands in any project: flags scripts, `curl … \| sh`, `sudo`, deletes outside the project, encoded commands, credential reads, secrets sent over the network, `chmod 777`, prompt-injection phrases, and invisible characters. Notes compatibility signals (no SKILL.md or description, `allowed-tools`, `.claude/` paths, Python or Node scripts). Advice, never a block; ⌘Y previews any skill. |
| U5 | **Agent-aware loadouts** | Named sets of skills ("Web app", "iOS app") that can declare preferred agents (Pi, Claude Code, Cursor, Codex, …). Apply to a project in one keystroke. When applied, installs into the correct agent folders and can optionally launch the preferred agent. |
| U6 | **Official CLI underneath** | Every install is `npx skills add … -y`, so lock files and 30+ agent folder layouts stay exactly as the ecosystem expects. Paste any `npx skills add` command to save its skills as favorites. |
| U7 | **Diff against loadout** | One-click (or keyboard) view of what the current project is missing versus a named loadout or the suggested one. Makes gaps obvious before you install. |
| U8 | **Skill provenance + audit** | Lightweight local log of what was installed/updated/removed, when, from which commit/tree, into which project, and by which action (palette, loadout apply, update). Plain JSON; useful for personal history and light team trust. |
| U9 | **One-shot equip + launch** | After installing (or applying a loadout), optionally launch the preferred agent for that loadout/project with a single extra key or checkbox. |
| U10 | **Shared team loadouts with versioning** | Beyond one-off `loadout://` links and `.loadout.json` files: a loadout can carry a version and the https URL the team publishes it at; "Check for a newer version" previews the update before importing. Still installs through the official CLI. (Signatures are not built.) |
| U11 | **Idle “what changed” digest** | Optional quiet daily/weekly notification summarizing real skill updates across your projects. Still never noisy; only surfaces changes that would have raised an Update pill. |

## Non-goals (deliberate)

- Marketplace browsing or search (skills.sh does it; paste a link from there).
- Writing or editing skills.
- Enabling/disabling, usage stats, cross-agent sync, Git backup (Skills Manager and SkillDeck do these well).
- Accounts, cloud servers, telemetry.
- Windows / Linux builds in v1 (GPUI supports them, but U1, tray, and notifications are macOS code).
- Mac App Store (sandboxing blocks `npx` in project folders).

## Status

Everything above is built (✅ U1–U11). Remaining work is the public release: `RELEASE.md`.

- **Palette (⌃⌥L):** detected project and stack, ranked and pre-selected favorites and loadouts, Return installs,
  ⌘↩ installs and opens the agent, ⌘K actions, ⌘Y preview, ⌘D diff against a loadout, ⌘S save as loadout,
  ⌘P project or Global, ⌘⌫ uninstall, safety glance before first installs.
- **Menu bar menu:** status, target, version, Open Palette, Update All, Check for Updates…, Settings…, Quit. The
  menu bar icon gets a dot when updates are pending.
- **Settings:** Interface (app icon, menu bar / Dock, show on launch, open at login), Shortcuts (record the palette
  shortcut), Integrations (several coding agents, launch commands, open after installing, GitHub token),
  Notifications (automatic checks, digest), Skills (add, tag, pin, ignore, what changed, update), Loadouts (edit,
  share, import, team versions), Projects (Global, add, reveal, forget), History (provenance, CLI output).
- **Data:** `~/Library/Application Support/Loadout/data.json`, imported once from the SkillDock prototype; the
  GitHub token in the Keychain.

The SwiftUI prototype (`reference/swift-prototype/`, built under the old name SkillDock) is kept as a behavior
reference only.

## Screens

### Palette (⌃⌥L) — the main surface, 750 × 475 pt

```
╭────────────────────────────────────────────────────────────────────────────────╮
│ ⌕  Search favorites and loadouts                        ( 📁 shop-dashboard ⌄ )│
│ Suggested for Next.js, React                                                   │
│▕[✓] vercel-react-best-practices                        vercel-labs/agent-skills▏│
│ [A] frontend-design                            Update  anthropics/skills       │
│ Loadouts                                                                       │
│ [≡] Web app                                            3 skills · prefers Pi   │
│ Other favorites                                                                │
│ [M] tdd                                                mattpocock/skills       │
│ [A] deploy-helper                              ⚠       acme/ops                │
│                                                                                │
│ status / safety notice              ( Install 1 skill ↩ │ Actions ⌘K )         │
╰────────────────────────────────────────────────────────────────────────────────╯
```

- Fixed size, like Tinycast's palette (750 × 475 pt, top edge 18% down the screen), so it never jumps as rows
  change; the list area also holds the preview (⌘Y), diff against loadout (⌘D), and project chooser (⌘P).
- Typing filters. ↑↓ move, Space/Tab toggles, Return installs selected (already-installed ones are skipped).
- ⌘↩ = install + open the preferred agent. ⌘S saves the selection as a loadout. ⌘⌫ twice uninstalls.
- The target chip on the search row: click it or ⌘P to switch project or choose Global.
- Actions ⌘K lists every action for the highlighted row with its shortcut, like Tinycast.
- Section labels here are the one exception to "no section titles": they explain *why* something is selected.

### Menu bar menu — 290 pt, for glancing

```
╭──────────────────────────────────╮
│ [🐙] Loadout                    › │
│      • 1 update available         │
├──────────────────────────────────┤
│ Into     shop-dashboard           │
│ Version  0.1.0                    │
├──────────────────────────────────┤
│ Open Palette               ⌃⌥L   │
│ Update All (1)              ⌘U   │
│ Check for Updates…   4 min ago   │
│ Settings…                   ⌘,   │
├──────────────────────────────────┤
│ Quit Loadout                ⌘Q   │
╰──────────────────────────────────╯
```

### Settings — a window with a sidebar

```
╭───────────────┬──────────────────────────────────────────────╮
│ ● ● ●         │ Interface                                    │
│ [🐙] Loadout  │ App Icon                                     │
│  • Up to date │ ┌──────────────────────────────────────────┐ │
│               │ │ [Default]   [Twilight]   [Graphite]      │ │
│ ▣ Interface   │ │ Show app icon              Menu bar ⌄    │ │
│ ⌘ Shortcuts   │ └──────────────────────────────────────────┘ │
│ ◎ Integrations│ Startup                                      │
│ ♪ Notifications│ ┌──────────────────────────────────────────┐ │
│               │ │ Show window on launch               ( )  │ │
│ ▤ Skills    1 │ │ Open at login                       ( )  │ │
│ ≡ Loadouts    │ └──────────────────────────────────────────┘ │
│ ▭ Projects    │                                              │
│ ◷ History     │                                              │
╰───────────────┴──────────────────────────────────────────────╯
```

- **Interface:** app icon variant, where the icon shows (menu bar / Dock and menu bar / Dock), show window on
  launch, open at login.
- **Shortcuts:** filterable list; click the palette shortcut to record a new one.
- **Integrations:** coding agents (check any number), open the agent after installing, a launch command per
  checked agent, GitHub token.
- **Notifications:** automatic checks, the update digest, a test notification.
- **Skills:** paste to add favorites; the list, what changed, Update All. **Loadouts:** edit, share, import.
  **Projects:** where installs go, add, reveal, forget. **History:** provenance and `skills` output.

### Row interactions
- **Palette:** click toggles; click the name to preview (a loadout's name shows its diff); ⌘K lists every action
  for the highlighted row. Right-click: Uninstall (installed skills), Diff against loadout (loadouts).
- **Settings → Skills:** right-click a skill for What changed · Pin / Unpin · History · Copy install command ·
  Uninstall from this project · Rename… · Tags… · Remove. Hover shows ✕ remove; the Update pill opens what changed.
- **Settings → Loadouts:** click to edit its skills; right-click for Rename… · Preferred agent… · Tags… · Version… ·
  Source URL… · Check for a newer version · Copy link · Save file… · Remove.
- **Settings → Projects:** click to install there; right-click for Reveal in Finder · Forget.

## Copy rules

Sentence case. No "please", "successfully", or "!". Errors say what happened and what to do.
Examples: "Installed 2 skills in shop-dashboard" · "Couldn't install tdd. See log." ·
"GitHub limit reached. Add a token in Settings, or try again in an hour." · "Suggested for Next.js" · "Missing 3 skills vs Web app loadout"

## Success measures (personal use first)

- Equipping a new project with the right skills: **one keystroke + Return, under 5 seconds.**
- Adding a favorite: one paste.
- Every real skill update is noticed within 6 hours; zero alerts for README-only changes.
- Diff against a loadout is readable in under 2 seconds.
- Idle cost: under 80 MB memory, ~0% CPU.
