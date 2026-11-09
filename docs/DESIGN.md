# Loadout — Design guide (GPUI + GPUI Kit)

Calm, fast, native-feeling. The palette should feel like Tinycast or Raycast; the menu like a system menu extra
(Rex's); Settings like a macOS settings window (Rex's: a sidebar of colored icons and grouped cards).
When in doubt, remove it.

Wireframes for all three surfaces are in `PRODUCT.md` → Screens. Reference images are in
`reference/images-prototype/` (`sample.png` menu, `interface.png` and `shortcuts.png` Settings).

## Rules

1. **One primary action per view.** Palette: the capsule's main action (Return). Settings → Skills: Update All, only
   when updates exist. Nothing else looks primary.
2. **Explain selections, not chrome.** The palette's section labels (Suggested for …, Loadouts, Other favorites)
   exist only because they explain why something is pre-selected.
3. **Keyboard first in the palette.** Everything is reachable from the keyboard; ⌘K lists every action for the
   highlighted row with its shortcut, so the palette needs no hint footer.
4. **Hide until needed.** Remove buttons on hover; Update All only when something is pending; the safety notice only
   when there are findings; output only when asked.
5. **Theme colors only.** Read from GPUI Kit's theme (`cx.theme()`): foreground, muted foreground, border, primary,
   danger, warning, sidebar colors, and the named colors (blue, green, …) for icon squares and status dots. Follow
   macOS light/dark. No hex in view code.
6. **GPUI Kit components first** (Input, Button, Switch, Checkbox, PopupMenu, Tooltip, Kbd). Custom elements only
   when nothing fits, kept in `views/`.
7. **Feel like macOS:** system font, 13 px body, hairline borders, soft radii (6–12 px), no heavy shadows except the
   ⌘K menu.
8. **Glass and soft rows (Tinycast).** The palette and the menu are frosted glass (blurred window background under a
   78% theme tint); Settings is opaque. Rows use a 10 px radius and a faint foreground wash for hover and highlight;
   every list row leads with a 26 px tile.
9. **Esc goes back before it closes.** Palette: clear the search, else leave the preview / diff / chooser / naming /
   ⌘K menu, else close. Settings: cancel an edit, else leave a detail view, else nothing (⌘W closes).

## Layout

| Token | Value |
|---|---|
| Palette | 750 × 475 pt, fixed, centered on the active screen with its top edge 18% down; shrinks on small screens |
| Palette search | 56 px tall, 20 px text, 18 px search glyph, no border; the two-line target chip on its right |
| Target chip | 40 px, radius 10: 26 px blue folder (Global: globe) tile, project in 13 px medium over its stack (or where it came from) in 11 px muted, chevron; tooltip says where it came from; click or ⌘P |
| Palette row | 40 px, radius 10, 26 px tile, 14 px name, 13 px muted source (up to 340 px) on the right; highlighted: 10% foreground wash and a 1 px outline at 3:1 |
| Loadout row | 48 px: a layers tile tinted per loadout, the name over its skills (12 px muted), subtitle on the right |
| Palette bar | 52 px: status or safety notice on the left, capsule on the right: the main action ↩ filled with primary, then Actions ⌘K |
| Actions menu (⌘K) | 320 px, radius 12, popover color, 32 px items with keycaps, above the capsule |
| Menu | 290 px: 54 px header (34 px app icon, name, status dot), 28 px items, shortcuts on the right |
| Settings window | 800 × 600 (min 700 × 480), transparent titlebar, 52 px title bar |
| Settings sidebar | 220 px, theme sidebar colors; 30 px items with 20 px colored icon squares; a 28 px search field on top that filters pages (⌘F, Esc clears, Return opens the first) |
| Settings card | radius 10, faint foreground wash, hairline dividers; 44 px rows: label + 11 px hint left, control right |
| Settings list row | 44 px, radius 10, 26 px tile, name + 11 px muted detail |
| Segmented control | pill with 24 px options; the chosen one raised on the background color |
| Shortcut key | 160 × 24 gray box, centered keys |
| Paste field (Skills) | 34 px, fully rounded, faint foreground wash |
| Gaps | 6 / 8 / 10 / 12 px |

## Type

| Use | Style |
|---|---|
| Palette search | 20 px regular |
| Settings page title | 15 px semibold; section titles 13 px semibold |
| Palette section label | 12 px medium, muted |
| Row name | 14 px (palette), 13 px medium (Settings) |
| Sources, status, hints | 11–13 px regular, muted |
| History, output, diffs | 10–11 px Menlo |

## Icons

GPUI Kit `IconName`; icons outside its default set are added with `icon_assets!` in `assets.rs`.

| Meaning | Icon |
|---|---|
| Search | search |
| Target / project | folder; Global: globe |
| Skill tile | the source owner's initial on a theme chart color picked by owner |
| Loadout tile | layers |
| Selected | the tile filled with primary and a check |
| Installed | check, muted |
| Pinned | pin, muted |
| Safety finding | shield-alert, warning color |
| Settings pages | settings (gear), command, bot, bell, book-open, layers, folder, clock, info |

**App icon:** the octopus in a bucket hat (`reference/images-prototype/loadout-logo.png`), with Twilight and Graphite
variants for Settings → General. **Menu bar icon:** the octopus as a template image (`assets/tray/menubar@2x.png`,
36 px at 18 pt); `menubar-update@2x.png` adds a dot while updates are pending.

## Row states

| State | Trailing element |
|---|---|
| Not installed in target | source text only |
| Installed, current | check, muted |
| Update available | "Update" pill: primary text on 12% primary, fully rounded |
| Pinned | pin, muted |
| Safety findings (first install) | shield-alert, warning color, findings in the tooltip |
| Loadout | "3 skills · prefers Pi" as its source text |
| Hover (Settings → Skills, no update) | ✕ remove |

## Motion

Windows appear instantly (no zoom). Hover and selection changes are instant. No redraws when idle.

## Copy

- Sentence case for messages and hints: "Installed 2 skills in shop-dashboard". Menu items and Settings section
  titles use title case, like macOS: "Check for Updates…", "App Icon".
- Buttons are verbs. Status lines are past tense and specific.
- Errors: what happened + what to do. "Couldn't read the Finder window. Allow Automation in System Settings."
- Never: "please", "successfully", "!", "Error:".
- Name: always "Loadout" (singular). A saved set of skills is "a loadout".

## Accessibility

Target: WCAG 2.2 AA.

- Every icon-only button has a tooltip and an accessibility label.
- Full keyboard coverage in the palette; ⌘K lists every action.
- Status never relies on color alone (the dot always has text next to it).
- Text keeps 4.5:1 and icons and the highlight outline 3:1 (`loadout_core::contrast`). On glass, colors are
  checked against `palette::glass_floor`, the worst case: the tint over a desktop of the opposite brightness.
  `palette::readable` darkens (or lightens) a theme color until it passes; `palette::muted` is muted text.
- Never fade text with `opacity`; use muted color instead.
- Palette semantics: the list is a list box, rows are options with their selected state and a label that
  includes the source and installed/update status; the capsule halves are buttons with their shortcuts; ⌘K is
  a menu of menu items.
