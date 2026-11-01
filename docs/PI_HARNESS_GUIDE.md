# Building Loadout with Pi Harness (Rust + GPUI)

A step-by-step way to build every feature in `PLAN.md` using Pi Harness on your Mac.

## 1. Set up once

```bash
# Tools
xcode-select --install                               # command line tools
# Install full Xcode from the App Store too — GPUI compiles Metal shaders with it
sudo xcode-select -s /Applications/Xcode.app          # use full Xcode, not just CLT
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
brew install node                                    # the app runs `npx skills`

# Project
unzip Loadout.zip && cd Loadout
git init && git add -A && git commit -m "Loadout: docs, positioning, and plan"
```

Start Pi Harness inside the folder (or open the project in your usual way). Pi reads `AGENTS.md` automatically,
so it knows the positioning, rules, commands, and docs.

## 2. The loop for every task

1. Ask for the next task (or name one: “do T2.1”).
2. Pi plans, writes code, runs fmt, clippy, tests, and build, updates the docs, and stops with a short checklist.
3. You try it: `cargo run -p loadout` for quick UI work, or `packaging/bundle.sh --install` for anything that needs
   the real app (hotkey permissions, Finder/editor detection, tray, notifications, agent launch).
4. If something's off, say so in plain words, or share a screenshot.
5. When it works: ask for a check, then commit.
6. Clear context before the next task so each task starts clean.

## 3. Suggested order

| Sessions | Tasks | Result |
|---|---|---|
| 1–4 | T0.1–T0.4 | Window, ⌃⌥L palette, "project you're in" detection, menu bar + bundle. Use Plan mode for T0.3 and T0.4. |
| 5–11 | T1.1–T1.7 | Core library with tests: storage, parser, installer, stack + context, updates, safety, provenance |
| 12–16 | T2.1–T2.5 | **The loop works:** ⌃⌥L → ranked suggestions → Return. Use it daily for a few days. |
| 17–20 | T3.1–T3.4 | Calm updates, what changed, pin, safety + compat glance, provenance view |
| 21–24 | T4.1–T4.4 | Agent-aware loadouts, diff against loadout, equip + launch, share/import |
| 25+ | T5.x | Team loadouts, digest, popup management, settings, release |

Run a UI review after M2, M3, and M4.

## 4. Prompts that help

- GPUI API errors: "Check the gpui-kit version in Cargo.lock, read its docs.rs page and examples, and fix the calls.
  Don't downgrade versions without asking."
- Detection spike: "Use Plan mode. Show me how you'll get the folder from Finder and from Zed/VS Code, which permissions
  each needs, and the fallback, before writing code."
- Staying on track: "Does this fit the positioning in docs/PRODUCT.md? If not, tell me instead of building it."
- When a feature feels heavy: "Make this more minimal per docs/DESIGN.md. Move secondary actions to a menu or a shortcut."
- If the agent wanders: "Stop. Only do task T2.3. Revert unrelated changes."
- Porting: "Port the behavior of reference/swift-prototype/Sources/SourceParser.swift into parser.rs with tests for every case."
- Context ranking: "Keep ranking deterministic and tested. Context is a boost, never a requirement."

## 5. Tips

- First GPUI builds take a few minutes; later builds are incremental.
- Permissions (Automation, Accessibility) attach to the app's bundle id and signature. If prompts stop appearing or
  detection breaks after a rebuild, remove Loadout in System Settings → Privacy & Security and grant it again.
- Keep each task in its own commit so you can undo one feature (`git revert`).
- If GPUI Kit publishes a new version mid-project, upgrade in its own task.
- Agent launch configuration should live in settings and degrade gracefully when the preferred agent is missing.
