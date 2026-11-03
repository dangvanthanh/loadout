# Loadout — Competitors

Researched October 2026. Star counts are approximate and change quickly; re-check before any public launch
(`RELEASE.md`).

## Summary

Agent-skill managers are a crowded, young space. Almost all are free and open source. Most of them are **libraries**:
they show every skill you have, let you browse marketplaces, sync folders between agents, and edit skills.

Loadout should not compete on that. Its job is narrower: **put the right skills into the project you're working on,
right now, keep them safely up to date, and (optionally) launch the agent that will use them.**

## The field

| App | Stack | Strengths | Gaps relative to Loadout |
|---|---|---|---|
| [Skills Manager](https://github.com/xingkongliang/skills-manager) (xingkongliang) | Tauri 2 + React + Rust, SQLite. Mac/Win/Linux. ~6.6k ★, MIT | The leader. Installs from Git, folders, archives, and skills.sh search. Upstream update checks with content compare. Project and global workspaces. Presets applied in one click. Symlink/copy sync to 56 agents. Git-repo backup. CLI. | Full window app, not a quick action. No stack detection. Presets are a one-time copy. Doesn't use the official `skills` CLI. |
| [SkillDeck](https://github.com/crossoverJie/SkillDeck) (crossoverJie) | SwiftUI, macOS 14+. ~479 ★, MIT | Native. Installs from GitHub or folders, writes symlinks and the lock file. One-click updates. skills.sh / ClawHub registry browser. SKILL.md editor. 12 agents. Homebrew cask. | Three-pane dashboard; no per-project focus mentioned; no menu bar. |
| [Skill Deck](https://github.com/Hccake/skill-deck) (Hccake) | Tauri + React. Mac/Win/Linux. ~30 ★, Apache-2.0 | Many sources (GitHub, Git, well-known URLs, raw SKILL.md, archives). Parses pasted `skills add` commands. Update check/apply. Global + project skills, copy between projects. | Unsigned macOS builds. Full window app. |
| [SkillsBar](https://github.com/amandeepmittal/skillsbar) | SwiftUI menu bar. ~27 ★, Apache-2.0 | Menu bar + ⌥⇧S popover + ⌘K palette. Watches skill folders (FSEvents). Trusted project folders. Usage stats, health check, pinning, collections. | Browses what's installed; doesn't install from remote sources or track upstream updates. |
| [Skill Cabinet](https://flaviocopes.com/skill-cabinet/) / Skillscout (Flavio Copes) | SwiftUI, macOS 15+. Free, MIT, notarized | Clean native app for the skills your agents load; usage counts; links into `~/.agents/skills`; CLI. | Global links, not per project. |
| [Skill Manager](https://github.com/dees91/agent-skill-manager) (dees91) | Go + Wails, TUI, CLI. ~1 ★, MIT | Turn skills on/off without deleting. Skill sets. Install from Git/folders. | No project-level skills; no update detection. |
| [AgentHarnessManager](https://github.com/mabyko/AgentHarnessManager), [SkillsManager](https://github.com/MC-and-his-Agents/SkillsManager), [skillmanager](https://github.com/bangadam/skillmanager), [LazySkills](https://lazyskills.sh/) | Various | Library views, TUI, MCP/CLI integration. | Same library-first model. |

Also relevant: the official [`skills` CLI](https://github.com/vercel-labs/skills) and [skills.sh](https://www.skills.sh/)
directory, and [ui-skills.com](https://www.ui-skills.com/). Loadout builds on these rather than replacing them.

## Feature overlap

✅ has it · ◐ partly · — no / not mentioned

| Feature | Skills Manager | SkillDeck | Skill Deck | SkillsBar | **Loadout** |
|---|---|---|---|---|---|
| Install from GitHub / skills.sh | ✅ | ✅ | ✅ | — | ✅ |
| Per-project installs | ✅ | — | ✅ | ◐ | ✅ |
| Upstream update check | ✅ | ✅ | ✅ | — | ✅ |
| Compare before update | ✅ | — | — | — | ✅ |
| Presets / sets | ✅ | — | — | ◐ collections | ✅ loadouts |
| Menu bar + hotkey | — | — | — | ✅ | ✅ |
| Marketplace browser | ✅ | ✅ | ✅ | — | **— on purpose** |
| Skill editor | — | ✅ | — | — | **— on purpose** |
| Sync between agents / Git backup | ✅ | ◐ | — | — | **— on purpose** |
| **Stack detection → suggested skills** | — | — | — | — | ✅ |
| **Context-weighted ranking** | — | — | — | — | ✅ |
| **Targets the project you're in (Finder / editor)** | — | — | — | — | ✅ |
| **Ignores non-skill changes when flagging updates** | — | — | — | — | ✅ |
| **Safety + compatibility glance** | — | — | — | — | ✅ |
| **Agent-aware loadouts (preferred agent)** | — | — | — | — | ✅ |
| **Diff against loadout** | — | — | — | — | ✅ |
| **Skill provenance / audit log** | — | — | — | — | ✅ |
| **One-shot equip + launch** | — | — | — | — | ✅ |
| **Versioned team loadouts** | — | — | — | — | ✅ |
| **Idle “what changed” digest** | — | — | — | — | ✅ |
| **Installs via the official `skills` CLI** | — | — | ◐ parses commands | — | ✅ |

The bold rows are where Loadout leads; all of them are built (`PRODUCT.md` → Status).

## Risks

- **Skills Manager could add any of these features.** Mitigation: move fast on the narrow flow, and keep the app tiny
  and native-feeling so it complements the big apps instead of replacing them. A user can run Skills Manager as their
  library and Loadout as their quick "equip this project" tool.
- **The `skills` CLI could grow a GUI or a stack-aware `skills suggest`.** Mitigation: Loadout already wraps the CLI, so
  it would benefit; the palette and project targeting stay unique.
- **Name collisions.** "SkillDock" was too close to SkillDeck / Skill Deck, so the app is now **Loadout**.
  Re-check trademarks and Homebrew before public release.
- **Agent launch surface is fragmented.** Different agents use different URL schemes / CLIs. Keep launch configuration
  simple and optional; never make it required for the core equip loop.
