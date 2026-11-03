# Loadout — Design guide (GPUI + GPUI Kit)

Calm, fast, native-feeling. The palette should feel like Spotlight or Raycast; the menu like a system menu extra; Settings like
a macOS settings window.
When in doubt, remove it.

## Rules

1. **One primary action per view.** Palette: Return = install. Settings → Skills: Update All (only when updates exist).
   Nothing else looks primary.
2. **Explain selections, not chrome.** No decorative titles. The palette's section labels (Suggested, Loadouts, Other)
   exist only because they explain *why* something is pre-selected.
3. **Keyboard first in the palette.** Everything reachable with ↑↓, Space, Return, ⌘↩ (equip + launch), ⌘P, ⌘D (diff), Esc. Show hints in the footer.
4. **Hide until needed.** Remove buttons on hover; update bar only when there are updates; log only when asked;
   safety notice only when there are findings.
5. **Theme colors only.** Read from GPUI Kit's active theme (`cx.theme()`): foreground, muted foreground, border,
   primary, list hover, danger, warning. Follow macOS light/dark. No hex in view code.
6. **GPUI Kit components first** (Input, Button, List, Popover, PopupMenu, Switch, Tooltip). Custom elements only when
   nothing fits, kept in `views/`.
7. **Feel like macOS:** system font, 13 px body, hairline borders, small radius (7 px; 12 px for the palette window),
   no heavy shadows except the palette's window shadow.
8. **Look like Tinycast.** Both windows are frosted glass (blurred window background under a 78% theme tint).
   Rows are soft: a 10 px radius and a faint foreground wash for hover and highlight, no list-active color.
   Every row leads with a 26 px tile; the main action lives in a bottom-right capsule with its keycap, next
   to "Actions ⌘K", which lists everything else for the highlighted row.

## Layout

| Token | Value |
|---|---|
| Palette | 750 × 475 pt, fixed (Tinycast's size), centered on the active screen, top edge 18% down; shrinks to fit small screens |
| Palette search | 56 px tall, 20 px text, 18 px search glyph, no border; the target chip sits on its right |
| Palette target chip | 28 px pill: folder icon, project (or Global), chevron; tooltip says where it came from; ⌘P |
| Palette row | 40 px, radius 10, 26 px tile, 14 px name, 13 px muted source (up to 340 px) on the right |
| Palette bar | 52 px: status or safety notice on the left, capsule (main action ↩ · Actions ⌘K) on the right |
| Actions menu (⌘K) | 320 px, radius 12, popover color, 32 px items with keycaps, above the capsule |
| Menu | 290 px wide: 54 px header (34 px app icon, name, status dot), info rows, 28 px items with shortcuts on the right |
| Settings window | 800 × 600 (min 700 × 480), transparent titlebar; 220 px sidebar (theme sidebar colors), 30 px items with 20 px colored icon squares |
| Settings card | radius 10, faint foreground wash, hairline dividers; 44 px rows: label + 11 px hint left, control right |
| Shortcut key | 160 × 24 gray box, centered keys |
| Settings list row | 44 px, padding 8, radius 10, 26 px tile |
| Paste field (Skills) | 34 px, fully rounded, faint foreground wash |
| Lists | palette: what fits in 475 pt (about 7 rows); Settings pages scroll as a whole |
| Gaps | 6 / 8 / 10 / 12 px |

## Type

| Use | Style |
|---|---|
| Palette search | 20 px regular |
| Section label | 11 px medium, muted |
| Row name | 13 px medium |
| Row source, status, hints | 11–12 px regular, muted |
| Log / provenance | 10 px monospace |

## Icons

GPUI Kit `IconName` where available; otherwise Lucide-style SVGs in `assets/icons/` (1.5 px stroke), 12–16 px.

| Meaning | Icon |
|---|---|
| Search | search |
| Target / project | folder |
| Suggested | sparkles (muted, small) |
| Loadout | layers |
| Selected / unselected | tile filled with primary and a check / the row's own tile |
| Skill tile | source owner's initial, tinted with a theme chart color picked by owner |
| Loadout tile | layers |
| Installed | check |
| Pinned | pin |
| Safety finding | shield-alert (warning color) |
| Diff | git-compare or list-tree |
| Launch | play or external-link |
| Refresh, settings, remove | rotate-cw, settings, x |

**Menu bar icon:** monochrome template image, a simple "backpack/layers" glyph; outline normally, filled when any
project has an update. The octopus from `reference/images-prototype/loadout-menubar.png` as a template image
(`assets/tray/menubar@2x.png`, 36 px at 18 pt). When any project has an update it switches to
`menubar-update@2x.png`: the same glyph with a dot in the top-right.

## Row states

| State | Trailing element |
|---|---|
| Not installed in target | source text only |
| Installed, current | check, muted |
| Update available | "Update" pill: primary text on 12% primary background, fully rounded |
| Pinned | pin, muted |
| Safety findings (first install) | shield-alert, warning color, tooltip "Includes scripts" |
| Loadout with preferred agent | small agent badge or muted text |
| Hover (Settings → Skills, no update) | x remove |

## Wireframes

See `PRODUCT.md` → Screens for the palette, the menu, and Settings.

Safety notice (inline in the palette, above the footer):
```
⚠ deploy-helper includes scripts (scripts/release.sh, curl … | sh).   Review   Install anyway
```

Diff against loadout (simple sheet or inline section):
```
Missing vs Web app (3)
○ frontend-design
○ vercel-react-best-practices
○ tdd
```

## Motion

Palette appears instantly (no zoom). Hover and selection changes are instant. Spinners only beside the working item.
No redraws when idle.

## Copy

- Sentence case: "Save as loadout", "Install only highlighted", "Check for updates automatically".
- Buttons are verbs. Status lines past tense and specific: "Installed 2 skills in shop-dashboard".
- Errors: what happened + what to do. "Couldn't read the Finder window. Allow Automation in System Settings."
- Never: "please", "successfully", "!", "Error:".
- Name: always "Loadout" (singular). A saved set of skills is "a loadout".

## Accessibility

- Every icon-only button has a tooltip.
- Full keyboard coverage in the palette.
- VoiceOver labels on interactive elements.
