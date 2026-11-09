# Loadout — Plan

The original plan (M0–M5, tasks T0.1–T5.4) is complete. Every unique feature in `PRODUCT.md` is built;
`git log` has one commit per task (`T2.1: palette with suggestions`, …) and `DECISIONS.md` records why each piece
works the way it does.

What's left before a public release is in **[`RELEASE.md`](RELEASE.md)**.

## Adding work

New work goes here as a task, one at a time:

```markdown
### [ ] T6.1 Short title
What to build, in two or three lines. Which files it touches.
**Accept:** what the owner checks by hand in the bundled app.
```

Stay inside the positioning in `PRODUCT.md`. Marketplace browsing, skill editing, enable/disable, usage stats, and
sync belong to other apps (`COMPETITORS.md`); if a request drifts there, say so before building it.

## Definition of done

Every task:
1. `cargo build` succeeds; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass.
2. `cargo test -p loadout-core` passes, with new tests for any new core logic.
3. The task's **Accept** line is checked by hand, in the bundled app for anything that needs permissions, the menu
   bar, notifications, or the Dock (`packaging/bundle.sh --install`).
4. Docs updated: tick the task with the date, adjust `PRODUCT.md`, `ARCHITECTURE.md`, `DESIGN.md`, and add a
   `DECISIONS.md` entry for any new dependency or design choice.
5. One commit per task: `T6.1: short title`.
