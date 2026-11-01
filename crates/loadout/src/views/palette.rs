use std::collections::HashSet;

use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::model::Uuid;
use loadout_core::tags;

use crate::state::{AppState, Scan, Target};

actions!(palette, [Dismiss]);

const CONTEXT: &str = "Palette";
pub const WIDTH: Pixels = px(520.);
const SEARCH_HEIGHT: Pixels = px(44.);
const INTO_HEIGHT: Pixels = px(28.);
const FOOTER_HEIGHT: Pixels = px(28.);
const ROW_HEIGHT: Pixels = px(36.);
const LABEL_HEIGHT: Pixels = px(24.);
const LIST_PADDING: Pixels = px(6.);
const EMPTY_HEIGHT: Pixels = px(40.);
const NOTICE_HEIGHT: Pixels = px(28.);
const MIN_ROWS: usize = 4;
const MAX_ROWS: usize = 7;

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// Window height for `rows` (favorites + loadouts): four to seven (room for a preview), then the
/// list scrolls. Up to three section labels.
pub fn height(rows: usize) -> Pixels {
    let list = if rows == 0 {
        EMPTY_HEIGHT
    } else {
        ROW_HEIGHT * rows.clamp(MIN_ROWS, MAX_ROWS) as f32 + LABEL_HEIGHT * 3. + LIST_PADDING * 2.
    };
    SEARCH_HEIGHT + INTO_HEIGHT + px(1.) + list + px(1.) + FOOTER_HEIGHT
}

struct Row {
    /// A favorite's or a loadout's id.
    id: Uuid,
    loadout: bool,
    name: SharedString,
    source: SharedString,
    suggested: bool,
    preselect: bool,
    installed: bool,
    /// Installed here, with a newer version available.
    update: bool,
    /// Safety findings for a first install, in one line.
    findings: Option<SharedString>,
}

impl Row {
    /// Installed rows can only be selected to update them.
    fn selectable(&self) -> bool {
        !self.installed || self.update
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Suggested,
    Loadouts,
    Other,
}

impl Row {
    fn section(&self) -> Section {
        match (self.loadout, self.suggested) {
            (true, _) => Section::Loadouts,
            (false, true) => Section::Suggested,
            (false, false) => Section::Other,
        }
    }
}

/// What the list shows, in order: section labels and rows (index into `rows`).
enum Entry {
    Label(Section),
    Row(usize),
}

pub struct Palette {
    state: Entity<AppState>,
    search: Entity<InputState>,
    /// Shown in place of the search while naming a new loadout (⌘S).
    name: Entity<InputState>,
    naming: bool,
    rows: Vec<Row>,
    selected: HashSet<Uuid>,
    /// The user changed the selection; ranking updates no longer reset it.
    touched: bool,
    /// Index into the visible rows.
    highlight: usize,
    /// The favorite whose files the list area shows instead of rows.
    preview: Option<Uuid>,
    /// The loadout the list area compares the project with (⌘D).
    diff: Option<Uuid>,
    /// Missing skills chosen to install from the diff.
    diff_selected: HashSet<Uuid>,
    /// Index into the diff's missing skills.
    diff_highlight: usize,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Palette {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search favorites and loadouts"));
        search.update(cx, |search, cx| search.focus(window, cx));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name this loadout"));
        let _subscriptions = vec![
            cx.subscribe_in(
                &name,
                window,
                |this, name, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let text = name.read(cx).value().to_string();
                        let ids = this.chosen(cx);
                        if this
                            .state
                            .update(cx, |state, cx| state.create_loadout(&text, &ids, cx))
                        {
                            this.stop_naming(window, cx);
                        }
                    }
                },
            ),
            cx.observe(&state, |this, _, cx| {
                this.rebuild(cx);
                cx.notify();
            }),
            cx.subscribe_in(&search, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.highlight = 0;
                    this.scroll.scroll_to_item(0);
                    cx.notify();
                }
            }),
            cx.observe_window_appearance(window, |_, window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
            // Close on blur, like Spotlight.
            cx.observe_window_activation(window, |_, window, _| {
                if !window.is_window_active() {
                    window.remove_window();
                }
            }),
        ];
        let mut palette = Self {
            state,
            search,
            name,
            naming: false,
            rows: Vec::new(),
            selected: HashSet::new(),
            touched: false,
            highlight: 0,
            preview: None,
            diff: None,
            diff_selected: HashSet::new(),
            diff_highlight: 0,
            scroll: ScrollHandle::new(),
            _subscriptions,
        };
        palette.rebuild(cx);
        palette
    }

    /// Re-ranks favorites for the current target; keeps the user's selection once touched.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let favorites = &state.data.favorites;
        let ranked = tags::rank_favorites(favorites, &state.target.stack, &state.target.signals);
        let loadouts = &state.data.loadouts;
        let ranked_loadouts = tags::rank(
            loadouts.iter().map(|l| state.data.loadout_tags(l.id)),
            &state.target.stack,
            &state.target.signals,
        );
        let loadout_rows = ranked_loadouts.iter().map(|s| {
            let loadout = &loadouts[s.index];
            let members = &loadout.favorite_ids;
            Row {
                id: loadout.id,
                loadout: true,
                name: loadout.name.clone().into(),
                source: state.loadout_subtitle(loadout.id).into(),
                suggested: s.score > 0,
                // Loadouts are suggested by rank, never pre-selected: applying one is a choice.
                preselect: false,
                installed: !members.is_empty()
                    && members.iter().all(|f| state.is_installed_here(*f)),
                update: members.iter().any(|f| state.update_here(*f)),
                findings: None,
            }
        });
        let favorite_rows = ranked.iter().map(|s| {
            let favorite = &favorites[s.index];
            Row {
                id: favorite.id,
                loadout: false,
                name: favorite.name.clone().into(),
                source: favorite.source.clone().into(),
                suggested: s.score > 0,
                preselect: s.preselect(),
                installed: state.is_installed_here(favorite.id),
                update: state.update_here(favorite.id),
                findings: match state.scans.get(&favorite.id) {
                    Some(Scan::Ready(scan)) if state.needs_glance(favorite.id) => {
                        scan.report.summary().map(|s| capitalize(&s).into())
                    }
                    _ => None,
                },
            }
        });
        let (suggested, other): (Vec<Row>, Vec<Row>) = favorite_rows.partition(|r| r.suggested);
        self.rows = suggested
            .into_iter()
            .chain(loadout_rows)
            .chain(other)
            .collect();
        if self.touched {
            let ids: HashSet<Uuid> = self.rows.iter().map(|r| r.id).collect();
            self.selected.retain(|id| ids.contains(id));
        } else {
            self.selected = self
                .rows
                .iter()
                .filter(|r| r.preselect)
                .map(|r| r.id)
                .collect();
        }
        // Installed skills are skipped, so they're never selected; updates are never preselected.
        let blocked: HashSet<Uuid> = self
            .rows
            .iter()
            .filter(|r| !r.selectable() || (r.installed && !self.touched))
            .map(|r| r.id)
            .collect();
        self.selected.retain(|id| !blocked.contains(id));
        self.prefetch_scans(cx);
    }

    /// Scans selected first installs early, so Return rarely waits for the glance.
    fn prefetch_scans(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        if state.target.detecting {
            return;
        }
        let ids: Vec<Uuid> = self
            .selected
            .iter()
            .copied()
            .filter(|id| !state.scans.contains_key(id) && state.needs_glance(*id))
            .collect();
        if !ids.is_empty() {
            self.state
                .update(cx, |state, cx| state.ensure_scans(&ids, cx));
        }
    }

    /// Shows `id`'s files and findings in the list area (or closes the preview).
    fn toggle_preview(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        if id.is_some_and(|id| self.rows.iter().any(|r| r.id == id && r.loadout)) {
            return;
        }
        self.preview = if self.preview.is_some() && (id.is_none() || id == self.preview) {
            None
        } else {
            id
        };
        if let Some(id) = self.preview {
            self.state
                .update(cx, |state, cx| state.ensure_scans(&[id], cx));
        }
        self.scroll = ScrollHandle::new();
        cx.notify();
    }

    /// Visible row indexes for the current query.
    fn visible(&self, cx: &App) -> Vec<usize> {
        let query = self.search.read(cx).value().to_lowercase();
        let query = query.trim();
        (0..self.rows.len())
            .filter(|&ix| {
                let row = &self.rows[ix];
                query.is_empty()
                    || row.name.to_lowercase().contains(query)
                    || row.source.to_lowercase().contains(query)
            })
            .collect()
    }

    fn entries(&self, visible: &[usize]) -> Vec<Entry> {
        // Labels explain why something is suggested, or set loadouts apart; otherwise none.
        let labeled = visible
            .iter()
            .any(|&ix| self.rows[ix].section() != Section::Other);
        let mut entries = Vec::new();
        let mut section = None;
        for &ix in visible {
            let current = self.rows[ix].section();
            if labeled && section != Some(current) {
                entries.push(Entry::Label(current));
                section = Some(current);
            }
            entries.push(Entry::Row(ix));
        }
        entries
    }

    /// Selected ids in list order, or the highlighted row's when nothing is selected.
    fn chosen(&self, cx: &App) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = self
            .rows
            .iter()
            .filter(|r| self.selected.contains(&r.id))
            .map(|r| r.id)
            .collect();
        if ids.is_empty()
            && let Some(&ix) = self.visible(cx).get(self.highlight)
        {
            ids.push(self.rows[ix].id);
        }
        ids
    }

    /// Compares the project with `loadout` in the list area, missing skills pre-selected; `None`
    /// goes back to the list.
    fn set_diff(&mut self, loadout: Option<Uuid>, cx: &mut Context<Self>) {
        self.diff = loadout;
        self.preview = None;
        self.diff_highlight = 0;
        self.diff_selected = loadout
            .map(|id| {
                self.state
                    .read(cx)
                    .loadout_diff(id)
                    .missing
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();
        self.scroll = ScrollHandle::new();
        cx.notify();
    }

    /// The highlighted row's id, if it's a loadout.
    fn highlighted_loadout(&self, cx: &App) -> Option<Uuid> {
        let &ix = self.visible(cx).get(self.highlight)?;
        let row = &self.rows[ix];
        row.loadout.then_some(row.id)
    }

    /// Keys while the diff shows: move and toggle among missing skills, install, go back.
    fn diff_key(&mut self, loadout: Uuid, key: &str, cx: &mut Context<Self>) -> bool {
        let missing = self.state.read(cx).loadout_diff(loadout).missing;
        match key {
            "escape" => self.set_diff(None, cx),
            "up" | "down" if !missing.is_empty() => {
                let last = missing.len() - 1;
                self.diff_highlight = if key == "up" {
                    self.diff_highlight.saturating_sub(1)
                } else {
                    (self.diff_highlight + 1).min(last)
                };
                cx.notify();
            }
            "space" | "tab" => {
                if let Some(id) = missing.get(self.diff_highlight)
                    && !self.diff_selected.remove(id)
                {
                    self.diff_selected.insert(*id);
                }
                cx.notify();
            }
            "enter" => {
                let ids: Vec<Uuid> = missing
                    .into_iter()
                    .filter(|id| self.diff_selected.contains(id))
                    .collect();
                self.state.update(cx, |state, cx| state.equip(&ids, cx));
            }
            _ => return false,
        }
        true
    }

    /// ⌘S: name the selection as a loadout, typed in place of the search.
    fn start_naming(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.chosen(cx).is_empty() {
            return;
        }
        self.naming = true;
        self.name.update(cx, |name, cx| {
            name.set_value("", window, cx);
            name.focus(window, cx);
        });
        cx.notify();
    }

    fn stop_naming(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.naming = false;
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
        cx.notify();
    }

    fn move_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        if visible.is_empty() {
            return;
        }
        let last = visible.len() as isize - 1;
        self.highlight = (self.highlight as isize + delta).clamp(0, last) as usize;
        let entry = self
            .entries(&visible)
            .iter()
            .position(|e| matches!(e, Entry::Row(ix) if *ix == visible[self.highlight]))
            .unwrap_or(0);
        self.scroll.scroll_to_item(entry);
        cx.notify();
    }

    fn toggle(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.rows.iter().any(|r| r.id == id && !r.selectable()) {
            return;
        }
        self.touched = true;
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
        cx.notify();
    }

    fn toggle_highlighted(&mut self, cx: &mut Context<Self>) {
        if let Some(&ix) = self.visible(cx).get(self.highlight) {
            self.toggle(self.rows[ix].id, cx);
        }
    }

    /// Keys the palette owns before the search field sees them.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        // While naming, keys belong to the name field; Esc cancels naming, not the palette.
        if self.naming {
            if key == "escape" {
                self.stop_naming(window, cx);
                cx.stop_propagation();
            }
            return;
        }
        if modifiers.platform && key == "s" {
            self.start_naming(window, cx);
            cx.stop_propagation();
            return;
        }
        // ⌘D compares the project with the highlighted loadout (U7); again goes back.
        if modifiers.platform && key == "d" {
            let loadout = if self.diff.is_some() {
                None
            } else {
                self.highlighted_loadout(cx)
            };
            self.set_diff(loadout, cx);
            cx.stop_propagation();
            return;
        }
        if let Some(loadout) = self.diff {
            let plain = !(modifiers.platform || modifiers.control || modifiers.alt);
            if plain && self.diff_key(loadout, key, cx) {
                cx.stop_propagation();
            }
            return;
        }
        // ⌘Y previews, like Quick Look in Finder; Esc leaves a preview before closing.
        if modifiers.platform && key == "y" {
            let highlighted = self
                .visible(cx)
                .get(self.highlight)
                .map(|&ix| self.rows[ix].id);
            self.toggle_preview(highlighted, cx);
            cx.stop_propagation();
            return;
        }
        if key == "escape" && self.preview.is_some() {
            self.toggle_preview(None, cx);
            cx.stop_propagation();
            return;
        }
        if modifiers.platform || modifiers.control || modifiers.alt {
            return;
        }
        let query_empty = self.search.read(cx).value().is_empty();
        match event.keystroke.key.as_str() {
            "up" => self.move_highlight(-1, cx),
            "down" => self.move_highlight(1, cx),
            "tab" => self.toggle_highlighted(cx),
            // With a query, space types; Tab still toggles.
            "space" if query_empty => self.toggle_highlighted(cx),
            "enter" => self.install(cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// Return: install (or update) the selection, or the highlighted row when nothing is selected.
    fn install(&mut self, cx: &mut Context<Self>) {
        let ids = self.chosen(cx);
        self.state.update(cx, |state, cx| state.equip(&ids, cx));
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }

    fn render_row(&self, ix: usize, highlighted: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let row = &self.rows[ix];
        let selected = self.selected.contains(&row.id);
        let id = row.id;
        let hover = theme.list_hover;
        let warning = theme.warning;
        let is_loadout = row.loadout;
        let this = cx.entity().downgrade();
        let icon = if selected {
            Icon::new(IconName::CircleCheck).text_color(theme.primary)
        } else {
            Icon::new(ExtraIcon::Circle).text_color(theme.muted_foreground)
        };
        let element = div()
            .id(("row", ix))
            .h(ROW_HEIGHT)
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(6.))
            .when(highlighted, |row| row.bg(theme.list_active))
            .when(!highlighted, |row| row.hover(move |s| s.bg(hover)))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle(id, cx)))
            .child(icon.small())
            .when(row.loadout, |row| {
                row.child(
                    Icon::new(ExtraIcon::Layers)
                        .xsmall()
                        .text_color(theme.muted_foreground),
                )
            })
            .child(
                div()
                    .id(("name", ix))
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .cursor_pointer()
                    // The name previews (a loadout's name shows its diff); the rest of the row toggles.
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if is_loadout {
                            this.set_diff(Some(id), cx);
                        } else {
                            this.toggle_preview(Some(id), cx);
                        }
                    }))
                    .child(row.name.clone()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .max_w(px(220.))
                    .truncate()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child(row.source.clone()),
            )
            .when_some(row.findings.clone(), |row, findings| {
                row.child(
                    div()
                        .id(("findings", ix))
                        .tooltip(move |window, cx| Tooltip::new(findings.clone()).build(window, cx))
                        .child(
                            Icon::new(ExtraIcon::ShieldAlert)
                                .small()
                                .text_color(warning),
                        ),
                )
            })
            .when(row.update, |row| row.child(update_pill(cx)))
            .when(row.installed && !row.update, |row| {
                row.child(
                    Icon::new(IconName::Check)
                        .small()
                        .text_color(theme.muted_foreground),
                )
            });
        if !is_loadout {
            return element.into_any_element();
        }
        element
            .context_menu(move |menu, _, _| {
                let this = this.clone();
                menu.item(
                    PopupMenuItem::new("Diff against loadout").on_click(move |_, _, cx| {
                        this.update(cx, |this, cx| this.set_diff(Some(id), cx)).ok();
                    }),
                )
            })
            .into_any_element()
    }
}

impl Palette {
    /// "Missing vs Web app (3)" rows to select, then installed ones and extras, muted.
    fn render_diff(&self, loadout: Uuid, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.state.read(cx);
        let Some(name) = state.data.loadout(loadout).map(|l| l.name.clone()) else {
            return Vec::new();
        };
        let diff = state.loadout_diff(loadout);
        let favorite = |id: &Uuid| {
            state
                .data
                .favorites
                .iter()
                .find(|f| f.id == *id)
                .map(|f| (f.name.clone(), f.source.clone()))
                .unwrap_or_default()
        };
        let missing: Vec<(Uuid, (String, String))> =
            diff.missing.iter().map(|id| (*id, favorite(id))).collect();
        let present: Vec<(String, String)> = diff.present.iter().map(favorite).collect();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let label = |text: String| {
            div()
                .h(LABEL_HEIGHT)
                .px_2()
                .flex()
                .items_center()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(muted)
                .child(text)
                .into_any_element()
        };
        let line = |name: String, trailing: String, icon: Option<Icon>| {
            div()
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .text_color(muted)
                .children(icon)
                .child(div().text_size(px(13.)).truncate().child(name))
                .child(div().flex_1())
                .child(
                    div()
                        .max_w(px(220.))
                        .truncate()
                        .text_size(px(11.))
                        .child(trailing),
                )
                .into_any_element()
        };
        let mut items = Vec::new();
        if missing.is_empty() {
            items.push(label(format!(
                "Nothing missing: this project has everything in {name}"
            )));
        } else {
            items.push(label(format!("Missing vs {name} ({})", missing.len())));
        }
        for (ix, (id, (skill, source))) in missing.into_iter().enumerate() {
            let selected = self.diff_selected.contains(&id);
            let highlighted = ix == self.diff_highlight;
            let theme = cx.theme();
            let hover = theme.list_hover;
            let icon = if selected {
                Icon::new(IconName::CircleCheck).text_color(theme.primary)
            } else {
                Icon::new(ExtraIcon::Circle).text_color(theme.muted_foreground)
            };
            items.push(
                div()
                    .id(("missing", ix))
                    .h(ROW_HEIGHT)
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(6.))
                    .when(highlighted, |row| row.bg(theme.list_active))
                    .when(!highlighted, |row| row.hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.diff_selected.remove(&id) {
                            this.diff_selected.insert(id);
                        }
                        cx.notify();
                    }))
                    .child(icon.small())
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(skill),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .max_w(px(220.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child(source),
                    )
                    .into_any_element(),
            );
        }
        if !present.is_empty() {
            items.push(label(format!("Installed ({})", present.len())));
            items.extend(present.into_iter().map(|(skill, source)| {
                line(skill, source, Some(Icon::new(IconName::Check).small()))
            }));
        }
        if !diff.extra.is_empty() {
            items.push(label(format!(
                "Extra, not in {name} ({})",
                diff.extra.len()
            )));
            items.extend(
                diff.extra
                    .into_iter()
                    .map(|skill| line(skill, String::new(), None)),
            );
        }
        items
    }

    /// A favorite's source, findings, compatibility notes, files, and SKILL.md.
    fn render_preview(&self, id: Uuid, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let Some(favorite) = state.data.favorites.iter().find(|f| f.id == id) else {
            return Vec::new();
        };
        let muted = theme.muted_foreground;
        let small = |text: SharedString, color: Hsla| {
            div()
                .px_2()
                .text_size(px(11.))
                .text_color(color)
                .child(text)
                .into_any_element()
        };
        let mut items = vec![
            div()
                .px_2()
                .pb_1()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(favorite.name.clone()),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(muted)
                        .child(favorite.source.clone()),
                )
                .into_any_element(),
        ];
        let scan = match state.scans.get(&id) {
            None | Some(Scan::Loading) => {
                items.push(small("Loading…".into(), muted));
                return items;
            }
            Some(Scan::Unavailable(why)) => {
                items.push(small(why.clone(), muted));
                return items;
            }
            Some(Scan::Ready(scan)) => scan.clone(),
        };
        for finding in &scan.report.findings {
            let place = match finding.line {
                0 => finding.file.clone(),
                line => format!("{}:{line} {}", finding.file, finding.excerpt),
            };
            items.push(small(
                format!("⚠ {} · {place}", finding.message).into(),
                theme.warning,
            ));
        }
        for note in &scan.report.notes {
            items.push(small(note.clone().into(), muted));
        }
        let files: Vec<&str> = scan.files.iter().map(|(path, _)| path.as_str()).collect();
        items.push(small(format!("Files: {}", files.join(", ")).into(), muted));
        if let Some((_, text)) = scan.files.iter().find(|(path, _)| path == "SKILL.md") {
            items.extend(text.lines().take(400).map(|line| {
                div()
                    .px_2()
                    .font_family("Menlo")
                    .text_size(px(10.))
                    .min_h(px(13.))
                    .child(line.to_string())
                    .into_any_element()
            }));
        }
        items
    }
}

impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible(cx);
        self.highlight = self.highlight.min(visible.len().saturating_sub(1));
        let entries = self.entries(&visible);
        let stack_label = self.state.read(cx).stack_summary();
        let rows: Vec<AnyElement> = match (self.diff, self.preview) {
            (Some(loadout), _) => self.render_diff(loadout, cx),
            (None, Some(id)) => self.render_preview(id, cx),
            (None, None) => entries
                .iter()
                .map(|entry| match entry {
                    Entry::Label(section) => section_label(*section, &stack_label, cx),
                    Entry::Row(ix) => {
                        let highlighted = visible.get(self.highlight) == Some(ix);
                        self.render_row(*ix, highlighted, cx)
                    }
                })
                .collect(),
        };
        let status = self.state.read(cx).install_status.clone();
        let notice = self
            .state
            .read(cx)
            .safety_notice
            .as_ref()
            .map(|n| (n.favorite, n.text.clone()));
        let theme = cx.theme();
        let chosen = |r: &&Row| self.selected.contains(&r.id);
        let loadouts: Vec<SharedString> = self
            .rows
            .iter()
            .filter(chosen)
            .filter(|r| r.loadout)
            .map(|r| r.name.clone())
            .collect();
        let updates = self
            .rows
            .iter()
            .filter(chosen)
            .filter(|r| !r.loadout && r.installed)
            .count();
        let installs = self.selected.len() - updates - loadouts.len();
        let diff_count = self.diff.map(|loadout| {
            let missing = self.state.read(cx).loadout_diff(loadout).missing;
            missing
                .iter()
                .filter(|id| self.diff_selected.contains(id))
                .count()
        });
        let loadout_highlighted = self.highlighted_loadout(cx).is_some();
        let empty = self.rows.is_empty();

        div()
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::dismiss))
            .capture_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .h(SEARCH_HEIGHT)
                    .px_3()
                    .flex()
                    .items_center()
                    .when(!self.naming, |bar| {
                        bar.child(
                            Input::new(&self.search)
                                .appearance(false)
                                .large()
                                .text_size(px(15.))
                                .prefix(Icon::new(IconName::Search).text_color(theme.muted_foreground)),
                        )
                    })
                    .when(self.naming, |bar| {
                        bar.child(
                            Input::new(&self.name)
                                .appearance(false)
                                .large()
                                .text_size(px(15.))
                                .prefix(Icon::new(ExtraIcon::Layers).text_color(theme.muted_foreground)),
                        )
                    }),
            )
            .child(into_line(&self.state.read(cx).target, &stack_label, cx))
            .child(div().h(px(1.)).bg(theme.border))
            .child(
                div()
                    .id("favorites")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .p(LIST_PADDING)
                    .when(empty && self.preview.is_none() && self.diff.is_none(), |list| {
                        list.flex().items_center().px_3().text_size(px(12.)).text_color(theme.muted_foreground).child(
                            "No favorites yet. Paste a link or owner/repo in the menu bar popup.",
                        )
                    })
                    .when(!empty && visible.is_empty() && self.preview.is_none() && self.diff.is_none(), |list| {
                        list.px_3().text_size(px(12.)).text_color(theme.muted_foreground).child("No matches")
                    })
                    .children(rows),
            )
            .when_some(notice.clone(), |palette, (favorite, text)| {
                palette.child(
                    div()
                        .h(NOTICE_HEIGHT)
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .text_size(px(12.))
                        .text_color(theme.warning)
                        .child(Icon::new(ExtraIcon::ShieldAlert).small())
                        .child(div().flex_1().min_w_0().truncate().child(text))
                        .child(
                            div()
                                .id("review")
                                .cursor_pointer()
                                .text_color(theme.foreground)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.preview = None;
                                    this.toggle_preview(Some(favorite), cx)
                                }))
                                .child("Review"),
                        ),
                )
            })
            .child(div().h(px(1.)).bg(theme.border))
            .child(
                div()
                    .h(FOOTER_HEIGHT)
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child(match (self.naming, status, notice.is_some(), diff_count) {
                        (true, ..) => "↩ Save as loadout".into(),
                        (false, Some(status), ..) => status,
                        (false, None, true, _) => "↩ Install anyway".into(),
                        (false, None, false, Some(0)) => "Nothing selected".into(),
                        (false, None, false, Some(n)) => format!("↩ Install {n} missing").into(),
                        (false, None, false, None) => return_hint(installs, updates, &loadouts),
                    })
                    .child(div().flex_1())
                    .child(if self.naming {
                        "esc cancel"
                    } else if self.diff.is_some() {
                        "↑↓ move · space select · esc back"
                    } else if self.preview.is_some() {
                        "⌘Y or esc closes preview"
                    } else if loadout_highlighted {
                        "↑↓ move · space select · ⌘D diff · esc close"
                    } else {
                        "↑↓ move · space select · ⌘S save as loadout · esc close"
                    }),
            )
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// "Update": primary text on a 12% primary background, fully rounded (DESIGN row states).
pub fn update_pill(cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .px_2()
        .py(px(1.))
        .rounded_full()
        .bg(theme.primary.opacity(0.12))
        .text_color(theme.primary)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .child("Update")
}

/// The footer's Return hint for what's selected.
fn return_hint(installs: usize, updates: usize, loadouts: &[SharedString]) -> SharedString {
    let skills_hint = skills_hint(installs, updates);
    match loadouts {
        [] => skills_hint,
        [one] if installs + updates == 0 => format!("↩ Apply {one}").into(),
        [one] => format!(
            "↩ Apply {one} + {}",
            skills_hint.trim_start_matches("↩ ").to_lowercase()
        )
        .into(),
        many => format!("↩ Apply {} loadouts", many.len()).into(),
    }
}

fn skills_hint(installs: usize, updates: usize) -> SharedString {
    let skills = |n: usize| {
        if n == 1 {
            "1 skill".to_string()
        } else {
            format!("{n} skills")
        }
    };
    match (installs, updates) {
        (0, 0) => "↩ Install highlighted".into(),
        (n, 0) => format!("↩ Install {}", skills(n)).into(),
        (0, n) => format!("↩ Update {}", skills(n)).into(),
        (a, b) => format!("↩ Install {a}, update {b}").into(),
    }
}

fn section_label(section: Section, stack_label: &str, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let suggested = section == Section::Suggested;
    let text: SharedString = match (section, stack_label.is_empty()) {
        (Section::Suggested, false) => format!("Suggested for {stack_label}").into(),
        (Section::Suggested, true) => "Suggested".into(),
        (Section::Loadouts, _) => "Loadouts".into(),
        (Section::Other, _) => "Other favorites".into(),
    };
    div()
        .h(LABEL_HEIGHT)
        .px_2()
        .flex()
        .items_center()
        .gap_1()
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.muted_foreground)
        .when(suggested, |label| {
            label.child(Icon::new(ExtraIcon::Sparkles).xsmall())
        })
        .child(text)
        .into_any_element()
}

/// "Into: shop-dashboard · Next.js, Tailwind        from Zed" — where Return will install.
fn into_line(target: &Target, stack_label: &str, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let name = target
        .root
        .as_ref()
        .and_then(|root| root.file_name())
        .map(|name| name.to_string_lossy().into_owned());
    let trailing: Option<SharedString> = if target.detecting {
        None
    } else if let Some(notice) = &target.notice {
        Some(notice.clone())
    } else if let Some(app) = &target.from_app {
        Some(format!("from {app}").into())
    } else {
        name.as_ref().map(|_| "last used".into())
    };
    div()
        .h(INTO_HEIGHT)
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(12.))
        .text_color(theme.muted_foreground)
        .child("Into:")
        .child(match name {
            Some(name) => div().text_color(theme.foreground).child(name),
            None => div().child("no project yet"),
        })
        .when(!stack_label.is_empty(), |line| {
            line.child(format!("· {stack_label}"))
        })
        .child(div().flex_1())
        .children(trailing.map(|text| div().min_w_0().truncate().child(text)))
}
