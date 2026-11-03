# Working on Loadout with Pi Harness (or another coding agent)

The original build (M0–M5) is done; see `PLAN.md`. This guide covers setting up and making changes from here.

## 1. Set up once

```bash
xcode-select --install                               # command line tools
# Install full Xcode from the App Store too — GPUI compiles Metal shaders with it
sudo xcode-select -s /Applications/Xcode.app          # use full Xcode, not just CLT
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
brew install node                                    # the app runs `npx skills`
brew install imagemagick                             # only to regenerate icons (packaging/icon/*.sh)
```

Start Pi Harness in the repo. It reads `AGENTS.md`, so it knows the positioning, rules, commands, and docs.

## 2. The loop

1. Name the change, or add it to `PLAN.md` as a task (format there). Release work is in `RELEASE.md`.
2. The agent reads the files it touches, writes failing core tests first, implements, then runs fmt, clippy, tests,
   and the build, and updates the docs (`PRODUCT.md`, `ARCHITECTURE.md`, `DESIGN.md`, a `DECISIONS.md` entry).
3. You try it: `cargo run -p loadout` for quick UI work, `packaging/bundle.sh --install` for anything that needs
   the real app (permissions, Finder/editor detection, menu bar, Dock, notifications, Keychain, agent launch).
4. Say what's off in plain words, or share a screenshot.
5. When it works, commit (one commit per change) and clear the context before the next one.

## 3. Seeing the UI without a screen

The agent can't take screenshots on a Mac without Screen Recording permission. It can render GPUI windows to PNGs
in a throwaway build: enable `test-support` on `gpui-pre` and `gpui-pre-macos` (not on `gpui-kit`, which pulls a
conflicting `gpui-fast`), add `image`, call `window.render_to_image()`, and revert everything afterwards. The glass
background doesn't show in those images (DECISIONS 048).

## 4. Prompts that help

- GPUI API errors: "Check the gpui-kit version in Cargo.lock, read the crate source under ~/.cargo/registry, and fix
  the calls. Don't change versions without asking."
- Staying on track: "Does this fit the positioning in docs/PRODUCT.md? If not, tell me instead of building it."
- When something feels heavy: "Make this more minimal per docs/DESIGN.md. Move secondary actions to ⌘K, a
  right-click, or Settings."
- If the agent wanders: "Stop. Only do this change. Revert unrelated edits."
- Ranking: "Keep ranking deterministic and tested. Context is a boost, never a requirement."

## 5. Tips

- First GPUI builds take a few minutes; later builds are incremental.
- Permissions (Automation, Accessibility) and the Keychain item attach to the bundle id and signature. If prompts
  stop appearing after a rebuild, remove Loadout in System Settings → Privacy & Security and allow it again.
- Each change in its own commit, so one feature can be undone with `git revert`.
- Upgrade GPUI Kit in a change of its own.
