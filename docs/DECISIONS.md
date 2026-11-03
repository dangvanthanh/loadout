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
  Hand check on 2026-10-31 passed with Finder, Zed, and an Xcode or VS Code window in front.
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

## 020 — data.json keeps the prototype's keys
**Choice:** `settings.autoCheck` (not a new `checkUpdates`), `settings.global`, `selectedProjectID`, and `installed`
stay exactly as SkillDock wrote them. New keys (`tags`, `loadouts`, `provenance`, `hotkey`, digest and launch
settings) default when missing. `skillState` waits for pin/ignore (T3.2) so we don't freeze a key before it has a
use. With no `Loadout/data.json` yet, `storage::load` reads `SkillDock/data.json`; the first save writes Loadout's
own file and leaves the prototype's alone. A file that doesn't parse is renamed `data.json.corrupt-<secs>` and
Loadout starts empty, so nothing is overwritten.
**Agents:** `agents.rs` copies 18 common agents' skill folders from `skills` 1.7.4. Unknown ids still pass to `-a`.

## 021 — Parser port: two fixes over the prototype
**Choice:** `parser.rs` follows `SourceParser.swift` case for case, with two fixes. `.git` is stripped only as a
suffix (Swift removed it anywhere, so `acme/my.github-tools` lost letters). ui-skills scraping matches
`npx skills add [^<\n"]+` on the raw HTML and decodes entities per match: the live page carries the command in a
`data-command-row="…"` attribute, where the old pattern ran past the closing quote into the tag. The fixture is a
trimmed excerpt of the real page (2026-10-31). URLs are split by hand; no `url` crate.

## 022 — Installer runs one merged pipe through a login shell
**Choice:** `installer::run` starts `/bin/zsh -lc` with the prototype's PATH/nvm preamble. stdout and stderr share
one `std::io::pipe`, so lines arrive in order. Output is split on `\n` and `\r` and ANSI is stripped; stdin is
null; the env sets `NO_COLOR`, `FORCE_COLOR=0`, `DO_NOT_TRACK`. A missing `npx` prints a marker line and exits 127,
which becomes `InstallError::NpxMissing`, so there's no second shell launch to check for it. `run` refuses any command
containing `skills check` / `skills update` (DECISIONS 003). `remove_command` needs skill names; without them the CLI
would prompt. Agent lists split on commas or whitespace, as in the prototype.
**Verified:** `npx -y skills add 'anthropics/skills' --skill 'frontend-design' -a 'claude-code' -y` in a temp dir
exited 0 and wrote `.claude/skills/frontend-design/` and `skills-lock.json` (`skills` → name → `source`,
`sourceType`, `skillPath`, `computedHash`).

## 023 — Project root and stack rules
**Choice:** `project::find_root` walks up from the detected folder. The nearest `.git` (folder or worktree file)
wins, then the nearest manifest (`package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `*.xcodeproj`, …), then
the folder itself. The home folder and its ancestors never count, so a dotfiles repo in `~` doesn't capture
everything. `stack::detect` reads root manifests only: package.json dependency keys, Cargo dependency sections
(line scan, no toml crate), whole-word package names in Python manifests, plus go.mod / Gemfile / composer.json /
pubspec.yaml / Package.swift / Dockerfile. Order is fixed per ecosystem (frameworks and libraries, then the
language), so ranking can rely on it. Each tag carries its evidence ("next in package.json").

## 024 — Ranking formula and context signals
**Choice:** `tags::rank` scores each item 100 per tag in the stack (plus implied tags: React → frontend,
Prisma → database, Vitest → testing, …) and 10 per tag in the context, then sorts stably. Ties keep the user's order,
so the result is deterministic. Only stack matches are pre-selected; context boosts order but never selects. Favorites
with no tags use `suggest_tags` (keyword table over name, skill, and source words) at rank time, so prototype imports
rank well. `context::gather` takes the editor's open file, `git status --porcelain -uall`, and the 20 newest files
from a bounded walk (depth 4, 3000 entries, skipping `node_modules`, `target`, …). Each path maps to
testing / frontend / database / devops / docs / language tags. Every source is optional (DECISIONS 016).

## 025 — Versions, diffs, and the safety scan
**Choice:** `updates.rs` ports `UpdateChecker.swift`: the folder's tree SHA for a named skill or folder URL; a SHA-256
over every skill folder's version for a whole-repo favorite; `HEAD` first, then the default branch. Two changes:
a skill at the repo root hashes the root's files minus README / LICENSE / CHANGELOG / `.github`, because the root
tree SHA moves on README edits; and a `blob/…/SKILL.md` link means its folder. Trees keep blob SHAs, so `diff.rs`
can list added / removed / modified files between two listings of a folder (fetched at each version's tree SHA),
plus a grouped SKILL.md line diff (`similar`). Blob text comes from the blobs API (base64).
**Caveat for T3.2:** synthetic versions (whole repo, root skill) aren't fetchable tree SHAs. Diffing those needs
the listing saved at install time.
**Safety:** `safety::scan` is pure over `(path, text)` pairs. It flags scripts plus eight line rules (pipe to shell,
sudo, deletes outside the project, encoded exec, credential reads, secrets sent over the network, `chmod 777`, prompt
injection) and invisible characters, once per rule per file. Compatibility notes cover a missing SKILL.md or
description, `allowed-tools`, `.claude/` paths, and Python/Node script runtimes. Informs only, never blocks.
**Verified live:** `anthropics/skills` → 20 skills; `skills/pdf` version = its tree SHA; its scripts are flagged.

## 026 — Palette keys, sections, and data ownership
**Choice:** `AppState` loads `data.json` at launch (importing SkillDock's on first run). If it can't be read, it
never saves over it. Each detection upserts the project into `projects` (`lastUsed`, `selectedProjectID`).
Detection, `project::root_for`, and `stack::detect` run in one background task. The palette ranks favorites with
`tags::rank_favorites` (context is empty until T2.5): "Suggested for <stack>" first, then "Other favorites". Labels
appear only when something is suggested, and stack matches are pre-selected until the user toggles. Keys are caught
in the capture phase before the search field: ↑↓ move, Tab toggles, Space toggles only while the search is empty
(otherwise it types), Esc closes. The window height is fixed at open for up to seven rows; longer lists scroll.
**Deferred:** the Loadouts section arrives with T4.1, because no loadout can exist before then. Extra icons
(circle, sparkles) come from GPUI Kit's `icon_assets!`.

## 027 — Favorites are managed in the popup
**Choice:** The popup's paste field (Return adds) accepts every parser format; ui-skills.com links resolve in the
background. `SavedState::add_favorites` skips duplicates (same source, case-insensitive, and skill) and tags
untagged favorites with `suggest_tags`. Right-click a row for Rename… / Tags… (inline field, Return saves, Esc
cancels) / Remove; hovering a row also shows ✕. Removing drops the favorite from loadouts and from `installed`.
Every change saves at once. Popup notices ("Added pdf") clear when the popup closes.

## 028 — Install runs as one background batch with a polled log
**Choice:** Return installs the selection, or the highlighted row when nothing is selected. `AppState::install`
runs `installer::run` for each favorite on the background executor. Lines come back over a std `mpsc` channel that a
100 ms foreground timer drains into `AppState.log` (capped at 2000 lines), so there's no new dependency. Favorites
already recorded for the target (`installed["<project id|global>|<favorite id>"]`) are skipped. A success records
`"?"` as the version (T3.1 fills it in) and appends an `install` provenance entry with the agents. The palette
footer shows "Installing tdd…" → "Installed 2 skills in shop-dashboard" / "Couldn't install tdd. See log.". The
popup shows the same status and a Log toggle.
**Verified:** a real `frontend-design` install into a temp project wrote the skill folder, `skills-lock.json`, the
`installed` record, and the provenance entry.

## 029 — What counts as installed
**Choice:** `installed::read` merges `skills-lock.json` (name → source) with the skill folders of the configured
agents (every known agent's folder when `agents` is empty); a subfolder with a SKILL.md counts. A named favorite is
installed when its normalized name is there and the lock's source, if any, is the same GitHub repo. A whole-source
favorite is installed when the lock attributes any skill to that repo. Loadout's own `installed` records also count.
The palette shows a muted ✓, never pre-selects or toggles those rows, and Return skips them. The set is read with
detection (background) and again after each install batch. Global installs rely on records only for now.

## 030 — Context in the live palette
**Choice:** Detection keeps the editor's open file (`Detection::Folder { folder, document }`), and the same
background task runs `context::gather(root, document)` next to stack and installed detection. The palette ranks
with it: context lifts items into Suggested but never pre-selects them. Hidden folders except `.github` are skipped
when gathering, so installed skills (`.claude/skills/…`) and tool folders don't read as the user's work.
**Verified:** in a git project with a changed `tests/login.spec.ts`, a `nextjs` favorite ranked first (stack,
pre-selected), `tdd` second (context "testing", not pre-selected), and an untagged favorite last.

## 031 — No install while the target is still being detected
**Choice:** `AppState::install` refuses while detection runs and shows "Finding the project…", which clears when
detection lands. A fast ⌃⌥L + Return would otherwise install into the fallback (last used) project shown before
detection finished. An empty selection on an empty list does nothing. Found in the M2 self-review.

## 032 — App icon: the octopus in a bucket hat
**Choice:** `packaging/icon/AppIcon.png` comes from `reference/images-prototype/logo.jpg` with the slate background
removed. The tile is a fitted rounded square (inset 4 px past the source's dark anti-aliased rim); the hat's crown
keeps its overhang above the tile. It follows the macOS icon grid: an 824 px body centered on a 1024 canvas,
transparent around it. `packaging/icon/make-icns.sh` builds `packaging/AppIcon.icns` (16–512 @1x/@2x); `bundle.sh`
copies it to `Contents/Resources` and `Info.plist` names it (`CFBundleIconFile`). The menu bar keeps its SF Symbol:
a full-color mascot doesn't work as a monochrome template at 18 pt. This covers the icon part of T5.5.

## 033 — New logo art; the menu bar gets the octopus too
**Choice:** Supersedes the source and menu bar parts of 032. `packaging/icon/AppIcon.png` is now
`reference/images-prototype/loadout-logo.png` (already a rounded tile), scaled to the 824 px body of the 1024 px
grid; `make-icns.sh` is unchanged. The menu bar uses `loadout-menubar.png` instead of the SF Symbol, as a template
image so AppKit tints it for light, dark, and highlighted menu bars. A template keeps only alpha, which would turn
the light-gray art into a blob, so `packaging/icon/make-menubar.sh` (ImageMagick) cuts the darker detail lines (eyes,
smile, hat band) out of the silhouette at an 85 % gray threshold and writes a 36 px PNG. The binary embeds it
(`include_bytes!`), so `cargo run` shows it too. Generated files are committed; builds don't need ImageMagick.

## 034 — How updates are checked, shown, and applied
**Choice:** A check fetches one tree per repo, only for favorites with an install recorded at a known version, so
"?" installs (and skills other tools installed) cost no requests. It runs 3 s after launch, then every 6 hours, and
from ↻. `SavedState::pending_updates` compares recorded versions with the session's latest versions: pinned skills,
ignored versions, and installs in removed projects don't count. After each successful install or update, the job
fetches that repo's tree once per batch and records the real version, so new installs are tracked from the start.
An update re-runs `skills add` in each project that's behind (`-g` for global records), sets the new version, and
appends an `update` provenance entry with `treeSha`. A project folder that's gone is skipped and noted in the log.
**Calm:** each new version is notified once (`skillState.notifiedVersion` is persisted, so relaunching doesn't
repeat it). A manual check marks updates seen without notifying. Clicking the notification opens the popup. The
menu bar icon shows a dot while anything is pending.
**Surfaces:** the popup shows an Update pill on rows (in place of the hover ✕) and "N updates available · Update all"
only when something is pending. In the palette, an installed row with an update shows the pill and can be selected
(never pre-selected); Return then updates it. The popup footer is now "Checked 4 minutes ago … ↻ ⚙". Output,
automatic checks, open at login, test notification, and Quit moved into the ⚙ menu, per DESIGN rule 4. The popup's
list area is now 4–7 rows high so other views fit in it.
**Verified:** with real GitHub, a stale recorded version surfaced as one pending update; Update all re-ran the CLI and
recorded the current tree SHA and an `update` entry. README-only vs folder changes are covered by core tests (T1.6).

## 035 — What changed, pin, and ignore
**Choice:** The Update pill (and "What changed" in the row's right-click menu) opens a view in the popup's list area:
where the skill is behind, Update / Ignore this update / Pin, then the changed files (+ − ~) and a SKILL.md diff with
2 lines of context. It costs four API requests: the folder listing at each version (both are tree SHAs) and two
SKILL.md blobs, read only when SKILL.md changed. The result is kept while the same update is shown again. Versions
that aren't tree SHAs (whole-repo favorites, root skills; DECISIONS 025) get a sentence instead of a file list; Update,
Ignore, and Pin still work. When several projects are behind at different versions, the diff starts from the first
recorded one. Pin = stay on the installed version (no pill, dot, or notification) until Unpin. Ignore = hide this
version only; a newer one surfaces again. Both persist in `skillState` keyed by source@skill, so they survive
removing and re-adding the favorite. Pinned rows show a muted pin. Esc leaves the view before it closes the popup.
**Verified:** with `frontend-design` recorded at an older tree SHA, the view listed SKILL.md as modified with its
real diff. Pin, unpin, and ignore changed the pending set as expected, and `ignoredVersion` was written to disk.

## 036 — Safety glance and preview
**Choice:** A first install is a GitHub skill Loadout never installed anywhere (no `installed` record, no provenance
entry) that isn't already in the target. Before Return installs one, its folder is scanned: one trees request per
repo (reusing the update check's listing) plus up to 25 text files from `raw.githubusercontent.com`, which doesn't
count against the 60/hour API limit. Scans are cached for the session and prefetched for pre-selected first
installs, so Return is usually instant. If any first install has findings, the palette shows one warning line above
the footer ("pdf includes scripts (…)") with Review, and the footer says "↩ Install anyway". Return again on the same
selection installs. A scan that fails counts as no findings: the glance informs and never blocks. Flagged rows show
a shield with the findings as a tooltip. Clicking a name, or ⌘Y on the highlighted row, previews it in the list
area: findings, compatibility notes, files, and SKILL.md. Esc or ⌘Y closes the preview before Esc closes the
palette. The palette list is now 4–7 rows high so a preview fits.
**Verified:** a real first install of `anthropics/skills@pdf` showed "Checking the skill…", then the scripts notice;
Return again installed it, and it no longer counts as a first install. `curl … | sh` detection is covered by core
tests; no public skill I checked uses it.

## 037 — History in the popup
**Choice:** ⚙ → History opens provenance in the popup's list area, newest first (up to 200 lines, 10 px monospace):
"Updated frontend-design in shop · d79e2a5 · 2 hours ago". A "This project" / "All projects" toggle filters by the
last detected project. A row's right-click → History shows that skill only. `provenance::describe` builds each line
and is tested; short SHAs appear only for real tree SHAs. Remove entries arrive with uninstall (T5.4); the view
already words them ("Removed tdd from blog").
**Verified:** the History view rendered from a real `data.json`, and its line matched the stored entry (action,
skill, project, tree SHA).

## 038 — Loadouts: saved from the palette, managed in the popup
**Choice:** ⌘S in the palette names the selection (favorites, plus the skills of any selected loadouts) as a new
loadout, typed in place of the search; Return saves and Esc cancels. Its tags are the first two stack tags on the
Into line, so it's suggested for projects like the one it came from. Names are unique (case-insensitive) and a
loadout keeps at least one skill. Untagged loadouts are ranked by the union of their skills' tags.
**Palette:** a Loadouts section between Suggested and Other favorites ("Web app · 2 skills · prefers Pi"). It's
ranked like favorites but never pre-selected, because applying a set is a deliberate choice. Section labels now show
when anything is suggested or any loadout is listed. Return applies a loadout through the same equip path: its
skills expand into install/update jobs (skipping installed ones), each provenance entry carries `loadoutId` and
"from Web app", and one `apply_loadout` entry records the apply itself. A loadout whose skills are all installed
shows ✓.
**Popup:** ⚙ → Loadouts lists them. Click one to tick its skills in or out. Right-click for Rename…, Preferred
agent… (an id or a name like "Claude Code", resolved against `agents.rs`; unknown names are refused; empty clears
it), Tags…, Remove.
**Verified:** created "Web app" from a Next.js project, set its agent to "Pi", and applied it. Both skills
installed with loadout-attributed entries and one apply entry.

## 039 — Diff against loadout in the palette
**Choice:** ⌘D on a highlighted loadout row, clicking its name, or right-click → "Diff against loadout" replaces the
list with "Missing vs Web app (n)" (selectable, all pre-selected), then "Installed (n)" and "Extra, not in Web app
(n)", muted. ↑↓ and Space or Tab work on the missing rows; Return installs the chosen ones through the usual equip
path (safety glance included); Esc or ⌘D goes back. Present = Loadout's records or the project's lock file / agent
folders (the same test as the ✓). Extra = names in the lock file or agent folders that no loadout skill accounts for
(`Installed::matching`: a whole-source favorite covers every skill its repo installed). The diff is computed live,
so installed skills move from Missing to Installed as they land. Global targets show no extras: only project folders
are read.
**Verified:** a project with `tdd` and an unrelated `notes` skill, against "Web app" (frontend-design + tdd), showed
missing frontend-design, installed tdd, extra notes. Installing the missing one left nothing missing.

## 040 — Equip + launch
**Choice:** ⌘↩ (or plain Return with ⚙ → "Open agent after installing") installs, then opens an agent in the
project folder (the home folder for global installs). The agent is the first selected loadout's preferred agent,
else the first id in `settings.agents`. It opens unless every job failed; an "Already installed" equip opens it
straight away. The footer names it: "↩ Apply Web app · ⌘↩ + open Pi".
**How it opens (`launch.rs`, run by `macos/launch.rs` off the main thread):**
- CLI agents (pi, claude, codex, gemini, opencode, goose, amp, qwen, kiro-cli): checked with `command -v` in a login
  shell, then a `.command` file in `~/Library/Caches/Loadout/` (`cd '<dir>' && exec pi`) is opened. Terminal, or the
  user's default app for `.command`, runs it: no Automation permission, no AppleScript.
- Editor agents (Cursor, Zed, Windsurf, Trae, Warp; VS Code for Copilot, Cline, Roo): `open -a <App> <dir>`.
- `settings.launchers` (agent id → shell command, `{dir}` replaced by the quoted folder) overrides either, e.g.
  Ghostty for pi. Editing it waits for the settings panel (T5.3); it's plain JSON until then.
**Graceful:** a missing CLI or app, an agent with no launcher, or a failing custom command becomes one footer line
("Goose isn't installed (goose wasn't found).") after the install summary. The install itself is unaffected.
**Verified:** applying "Web app" (preferred agent pi, with a custom launcher) installed its skill and ran the launcher
in a folder whose path has a space; the missing-CLI and missing-app messages showed for goose and Windsurf.

## 041 — Sharing loadouts: one format, previewed before import
**Choice:** `share.rs` defines `.loadout.json` (`{"loadout": 1, "name", "tags", "preferredAgent", "version",
"skills": [{"name", "source", "skill", "tags"}]}`). A link is the same JSON, base64url without padding:
`loadout://import?data=…`. A loadout row's right-click → Copy link / Save file… (save panel, then revealed in
Finder) exports it. Imports arrive by pasting a link or JSON into the popup's paste field, opening a `loadout://`
link, Finder's Open With (JSON, Alternate rank, so never the default), or ⚙ → Import loadout file….
**Trust boundary:** imports come from anyone. `share::parse` rejects anything over 256 KB or 100 skills, newer
formats, control characters, and any source the paste parser doesn't recognise (no local paths, no commands).
Unknown agents are dropped and tags normalised. Nothing changes until the popup's preview ("Import Web app? 2
skills · prefers Pi", with a warning when it replaces an existing loadout of that name) gets Import. Importing adds
missing favorites (with suggested tags) and creates the loadout or updates the same-named one; it never removes
anything and never installs. Installing still goes through the palette, the safety glance, and the official CLI.
**Verified:** on a clean `HOME`, a malformed link only showed a notice; a link and then a file, delivered through
the AppKit open-URL path, previewed, imported (2 new favorites, agent pi), and re-importing updated the same loadout
without duplicating favorites.

## 042 — Team loadouts: version + https source, no signatures yet
**Choice:** A team publishes a `.loadout.json` somewhere reachable over https (a raw GitHub URL works) with
`version` and `source` set. A loadout's right-click menu gets Version…, Source URL… (https only), and, once a
source is set, "Check for a newer version". That fetches the file (≤ 256 KB, the same validation as any import)
and opens the import preview only if its version is newer. Versions compare by numeric parts ("1.10" > "1.9",
"v2" = "2"). The preview names the change ("Updates your Web app from version 1.1", "Older than yours (version
1.3); importing goes back to it", or a replace warning for unversioned files). Every import appends an
`import_loadout` provenance entry ("Imported Web app · version 1.2"), and the loadout keeps `version` and `source`
in `data.json`. Installing still goes through the palette and the official CLI.
**Not done: signatures.** Signing needs a crypto dependency and a way to distribute and trust keys, both of which
need the owner's OK (AGENTS.md). The trust model today: you choose the source URL; every import is previewed and
validated; nothing installs without the palette and the safety glance. Checks are manual, not part of the 6-hour
update check.
**Verified:** a v1.2 file imported with its version and source and appeared in History; a v1.1 file previewed as
older; an http source was refused; checking a real https URL that isn't a loadout ended in a clear message.

## 043 — The digest replaces per-version notifications
**Choice:** ⚙ → Update digest: Off / Daily / Weekly (`digestEnabled`, `digestIntervalHours`). One hourly tick
replaces the 6-hour loop: if the digest is on and due (`lastDigest` + interval), it checks GitHub and posts one
notification naming what's pending ("frontend-design, tdd, and 2 more can be updated."), or nothing if nothing is.
Either way it records `lastDigest`, so there's at most one per interval. Otherwise the plain 6-hourly check runs
when automatic checks are on. With the digest on, those checks stay silent: the digest is the only notification.
Turning it on starts counting from that moment rather than firing at once. No `macos/notify.rs`: GPUI posts.
**Verified:** with a stale install and the digest due, one check ran, `lastDigest` was saved, and the summary read
"frontend-design can be updated."; an immediate second tick did nothing. The notification itself only appears in
the bundled app.

## 044 — Where installs go, and the settings view
**Choice:** The popup's top row ("Into: shop-dashboard ⌄") lists recent projects, Global (home folder), Add
project… (folder picker), and Forget <project>. ⌘P in the palette shows the same choice in the list area. Picking
a project targets it at once (its stack, installed skills, and context are re-read) and marks it "chosen" on the
Into line. Global is the prototype's persistent `global` setting: installs use `-g` until a project is picked
again, and the Into line says "Global · home folder". Return doesn't wait for detection while Global is on.
**Settings** (⚙ → Settings…, `views/settings.rs`): palette shortcut (global-hotkey format such as `ctrl+alt+L`;
applied live: the new one is registered before the old one is released, so a bad or taken shortcut leaves the old
one working and shows why), agents (`-a` list; a Select since DECISIONS 049), update digest (Off /
Daily / Weekly), automatic checks, open agent after installing, open at login, and a test notification. Text
fields save on Return or blur. The ⚙ menu is now Loadouts, Import loadout file…, History, Output, Settings…, Quit.
**Fixed:** startup ignored the saved `settings.hotkey` and always registered ⌃⌥L. It now uses the saved one and
falls back to ⌃⌥L with a notice.
**Verified:** a saved custom shortcut registered at startup; a malformed one was refused and a new one applied and
saved; agents normalised to "codex, cursor"; picking a Vue project targeted it as "chosen"; Global persisted; the
Settings view and the ⌘P chooser rendered.

## 045 — Uninstall, the GitHub token, and keyboard coverage
**Uninstall:** in the palette, ⌘⌫ on an installed row asks once ("⌘⌫ again removes tdd from shop") and the second
press runs `npx -y skills remove '<names>' -a <agents> [-g] -y`. Right-click → Uninstall in the palette, or
"Uninstall from this project" in the popup, runs it directly. The names come from what the project actually has (lock
file / agent folders, so a whole-source favorite removes each of its skills), else the favorite's own skill name.
A success drops the install record and appends a `remove` provenance entry ("Removed tdd from shop"). The skill
stays a favorite, and having been installed once means no new safety glance.
**Token:** `security-framework` (listed in ARCHITECTURE's dependency table for this, already in Cargo.lock) keeps it
in the login Keychain, service `local.loadout.app`, never in data.json. It's read in the background at launch (the
Keychain may ask) and sent as `Authorization: Bearer` on every GitHub API call: update checks, version reads,
what-changed, and safety-scan listings. Raw file reads don't need it. Settings has a masked field: Return saves,
empty removes. GitHub's 401 becomes "GitHub didn't accept your token. Check it in Settings." The rate-limit message
now suggests adding one. Rebuilt ad-hoc-signed apps may be asked again for Keychain access.
**Keyboard:** the popup adds ⌘, Settings, ⌘L Loadouts, ⌘R check, ⌘U update all (other ⌘ keys still reach the text
fields). The palette adds ⌘⌫ and ⌘P (T5.3) on top of ↑↓ / Space / Tab / Return / ⌘↩ / ⌘S / ⌘D / ⌘Y / Esc.
Icon-only buttons have tooltips and accessibility labels. Clickable rows are plain elements without labels yet;
VoiceOver reads their text.
**Verified:** a real install then uninstall emptied the project (`skills-lock.json` → no skills), and History
showed both. A bogus token produced GitHub's rejection message, which proves the header is sent. Saving to the
Keychain was not exercised here, to keep test runs out of your login Keychain.

## 046 — Release tooling, and what only the owner can do
**Choice:** `packaging/bundle.sh` gains `--universal` (adds the `x86_64-apple-darwin` Rust target once, builds both
architectures, `lipo`s them) and `--release`. The release path runs universal, then a Developer ID signature with
hardened runtime and `entitlements.plist` (Apple events, for reading Finder), `ditto` zip, `notarytool submit
--wait`, staple, re-zip, and `spctl` assessment, and prints the version and sha256. It refuses to start without
`LOADOUT_SIGN_ID` and `LOADOUT_NOTARY_PROFILE`. `packaging/homebrew/loadout.rb` is the cask (Node.js caveat, zap
of Loadout's support and cache folders, quit on uninstall) pointing at
`github.com/dangvanthanh/loadout/releases/download/v<version>/Loadout-<version>.zip`.
**Verified here:** a universal build (x86_64 + arm64, 58 MB) that verifies and launches on Apple Silicon. The Intel
slice is a valid x86_64 executable but wasn't run: this Mac has no Rosetta.
**Left for the owner (T5.5 stays unchecked):**
- A "Developer ID Application" certificate: this Mac only has an "Apple Development" one, which Gatekeeper rejects
  on other Macs. Then `xcrun notarytool store-credentials` and one `--release` run.
- Uploading the zip to a GitHub release and copying the cask into a tap: publishing, so it's the owner's call.
- Before going public: a real bundle id instead of `local.loadout.app`. Changing it later resets macOS permissions,
  the login item, and the Keychain token, so decide once. The trademark/name check from 011 is still open too.
- A clean-Mac install test of the cask.

## 047 — Window sizes: Tinycast's palette, a wider popup
**Choice:** The palette is 750 × 475 pt (Tinycast's `panelWidth` / `panelHeight`), at a fixed height instead of
growing with the row count, centered with its top edge 18% down the visible screen (Tinycast's
`paletteTopMarginFraction`), and shrunk to fit screens smaller than that. Rows and bars grew with it: search 56 pt
(17 pt text), Into line 32, rows 40, labels 26, footer 40. A row's source column can use 340 pt, so
`vercel-labs/agent-skills` no longer truncates. The popup is 400 pt wide (was 340). Tinycast's own menu bar item is a
native menu, so there's no size to copy for it; 400 fits a name, its source, and the Update pill on one row, and gives
the diff and history views room.
**Why:** At 520 pt the palette cut off sources and crowded previews and diffs; the owner asked for widths like
Tinycast's.
**Verified:** the window server reported the palette at 750 × 476 (a 1 pt border rounding) and the popup at 400 wide.

## 048 — Tinycast's look, and Esc that goes back
**Choice:** Both windows are frosted glass (`WindowBackgroundAppearance::Blurred` under a 78% theme tint). The
palette drops its divider lines and footer hints for Tinycast's layout: a 56 pt search row (20 pt text) with the
target as a pill on its right ("📁 shop-dashboard ⌄", tooltip says where it came from, click or ⌘P), soft rows
(10 pt radius, faint foreground wash) led by a 26 pt tile, and a bottom bar with status or the safety notice on
the left and a capsule on the right: the main action with its keycap ("Install 1 skill ↩", "Apply Web app ↩",
"Install anyway ↩", "Choose ↩"), then "Actions ⌘K". ⌘K opens a 320 pt menu above it listing everything for the
highlighted row with keycaps (install, install and open the agent, select, preview or diff, uninstall, save as
loadout, choose project); ↑↓ and Return work in it. A skill's tile is its source owner's initial on a theme chart
color picked by owner (so one author's skills match), a loadout's is layers, and a selected row's tile fills with
the primary color and a check. The diff and project chooser use the same rows. The popup gets the same tiles,
rounded rows, a pill paste field, a folder chip with a caret, and its notices move into the footer (no empty
line under the paste field). The Into line is gone; its information lives in the chip.
**Fixed:** GPUI runs key bindings before key listeners, so the `escape → Dismiss` bindings closed the palette even
from a preview, diff, chooser, loadout name, or the ⌘K menu, and closed the popup while renaming. Esc is now
handled last in each window's key handler: palette = clear the search, else go back, else close (Tinycast's
order); popup = cancel the edit or import, else go back, else close.
**Verified:** palette list, ⌘K menu, diff, chooser, and popup were rendered to images in light and dark through
GPUI's Metal `render_to_image` in a throwaway build (test-support features enabled, close-on-blur off; not
committed). The glass itself only shows on screen.

## 049 — Agents is a Select
**Choice:** Settings → Agents is a searchable GPUI Kit `Select` instead of a text field: "Detect automatically"
(empty `agents`, so the CLI picks), then every agent in `agents.rs`. GPUI Kit's Select picks one item, so choosing
from it sets a single agent. A list configured some other way (several agents, or an id Loadout doesn't know,
e.g. the prototype's `claude-code,cursor`) appears as its own entry ("Claude Code, Cursor") and stays selected,
so opening Settings never drops it. Choosing anything else replaces it. Several agents at once would need GPUI
Kit's multi-select Combobox; not done, since nobody asked for it.
**Verified:** rendered the Settings view with `claude-code,cursor` (shown as one entry) and after choosing Pi
(`agents` saved as `pi`, hint "Installs for Pi.").

## 050 — A small menu and a Settings window
**Choice:** The menu bar popup is replaced by two surfaces, after `reference/images-prototype/sample.png`,
`interface.png`, and `shortcuts.png`.
- **Menu** (`views/menu.rs`, 290 pt): app icon, name, and a status dot ("1 update available", "Up to date",
  "Installing tdd…"); clicking it opens Settings. Then Into / Version rows, Open Palette (with the shortcut),
  Update All (only when pending), Check for Updates… (with "4 minutes ago"), Settings… ⌘,, Quit ⌘Q.
- **Settings** (`views/settings.rs`, a normal 800 × 600 window, transparent titlebar): a sidebar with colored
  icon squares (theme named colors) in two groups, the selected page's title, and grouped cards.
  - **Interface:** app icon variant (Default, Twilight, Graphite, generated by `packaging/icon/make-variants.sh`;
    shown in the menu, the sidebar, and the Dock), "Show app icon" (Menu bar / Dock and menu bar / Dock: switches
    the activation policy and hides the status item), Show window on launch, Open at login.
  - **Shortcuts:** filter field, then Global / Palette / Menu / Settings cards. Clicking the
    palette shortcut records the next keystroke (`shortcut::from_keystroke`; needs ⌃, ⌥, or ⌘; Esc cancels);
    Reset goes back to ⌃⌥L.
  - **Integrations:** coding agents as checkboxes, so several agents can be on at once (supersedes 049's
    single-choice Select), with ids Loadout doesn't know kept; "Open the agent after installing"; a launch
    command per checked agent (`settings.launchers`, placeholder shows the default; first UI for it); the GitHub
    token.
  - **Notifications:** automatic checks, the digest (a segmented control), a test notification.
  - **Skills, Loadouts, Projects, History:** what the popup managed, moved (`settings/library.rs`). Projects gains
    a page of its own (Global, each project with path and last use, click to install there, right-click Reveal in
    Finder / Forget, Add Project…). History has History / Output tabs and All / This project / This skill filters.
**Gaps filled while moving:** "Copy install command" on skill rows (PRODUCT's row interactions listed it; never
built), the launch-command editor (DECISIONS 040 deferred it), a reachable Output view, a Loadouts page. With
the Dock icon shown, a minimal app menu (Settings…, Quit) appears; clicking the Dock icon opens the palette (051). Notifications
and imports open Settings → Skills / Loadouts. Settings data.json keys: `appIcon`, `iconPlace`,
`showWindowOnLaunch`; unknown values fall back to defaults instead of failing the file.
**Not done:** "Search Settings" field and back/forward buttons; the Dock icon variant only applies while the
Dock icon shows, and Finder and notifications keep AppIcon.icns.
**Verified:** the menu (light and dark) and all eight pages were rendered to images through GPUI's Metal
`render_to_image` in a throwaway build. Recording a shortcut, the Dock modes, and the save/reveal panels were not
exercised here.

## 051 — Clicking the Dock icon opens the palette
**Choice:** With Loadout in the Dock, clicking its icon (or opening the app again) opens the palette, the same
as the shortcut; if the palette is already open it stays. Settings stays one step away in the app menu (⌘,) and
the menu bar menu.
**Cost:** a Dock click makes Loadout the frontmost app, so the palette can't see which project you were in. It
targets the last used project (shown as "Last used" on its chip), and ⌘P picks another.
