use std::collections::HashSet;

use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::model::Uuid;
use loadout_core::tags;

use crate::state::{AppState, Target};

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
const MAX_ROWS: usize = 7;

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// Window height for `favorites` rows: at most seven, then the list scrolls.
pub fn height(favorites: usize) -> Pixels {
    let list = if favorites == 0 {
        EMPTY_HEIGHT
    } else {
        ROW_HEIGHT * favorites.min(MAX_ROWS) as f32 + LABEL_HEIGHT * 2. + LIST_PADDING * 2.
    };
    SEARCH_HEIGHT + INTO_HEIGHT + px(1.) + list + px(1.) + FOOTER_HEIGHT
}

struct Row {
    id: Uuid,
    name: SharedString,
    source: SharedString,
    suggested: bool,
    preselect: bool,
    installed: bool,
    /// Installed here, with a newer version available.
    update: bool,
}

impl Row {
    /// Installed rows can only be selected to update them.
    fn selectable(&self) -> bool {
        !self.installed || self.update
    }
}

/// What the list shows, in order: section labels and rows (index into `rows`).
enum Entry {
    Label { suggested: bool },
    Row(usize),
}

pub struct Palette {
    state: Entity<AppState>,
    search: Entity<InputState>,
    rows: Vec<Row>,
    selected: HashSet<Uuid>,
    /// The user changed the selection; ranking updates no longer reset it.
    touched: bool,
    /// Index into the visible rows.
    highlight: usize,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Palette {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search favorites and loadouts"));
        search.update(cx, |search, cx| search.focus(window, cx));
        let _subscriptions = vec![
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
            rows: Vec::new(),
            selected: HashSet::new(),
            touched: false,
            highlight: 0,
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
        self.rows = ranked
            .iter()
            .map(|s| {
                let favorite = &favorites[s.index];
                Row {
                    id: favorite.id,
                    name: favorite.name.clone().into(),
                    source: favorite.source.clone().into(),
                    suggested: s.score > 0,
                    preselect: s.preselect(),
                    installed: state.is_installed_here(favorite.id),
                    update: state.update_here(favorite.id),
                }
            })
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
        let has_suggested = visible.iter().any(|&ix| self.rows[ix].suggested);
        let mut entries = Vec::new();
        let mut section = None;
        for &ix in visible {
            let suggested = self.rows[ix].suggested;
            // Labels only explain why something is suggested; no suggestions, no labels.
            if has_suggested && section != Some(suggested) {
                entries.push(Entry::Label { suggested });
                section = Some(suggested);
            }
            entries.push(Entry::Row(ix));
        }
        entries
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
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
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
        let icon = if selected {
            Icon::new(IconName::CircleCheck).text_color(theme.primary)
        } else {
            Icon::new(ExtraIcon::Circle).text_color(theme.muted_foreground)
        };
        div()
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
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
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
            .when(row.update, |row| row.child(update_pill(cx)))
            .when(row.installed && !row.update, |row| {
                row.child(
                    Icon::new(IconName::Check)
                        .small()
                        .text_color(theme.muted_foreground),
                )
            })
            .into_any_element()
    }
}

impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible(cx);
        self.highlight = self.highlight.min(visible.len().saturating_sub(1));
        let entries = self.entries(&visible);
        let stack_label = self.state.read(cx).stack_summary();
        let rows: Vec<AnyElement> = entries
            .iter()
            .map(|entry| match entry {
                Entry::Label { suggested } => section_label(*suggested, &stack_label, cx),
                Entry::Row(ix) => {
                    let highlighted = visible.get(self.highlight) == Some(ix);
                    self.render_row(*ix, highlighted, cx)
                }
            })
            .collect();
        let status = self.state.read(cx).install_status.clone();
        let theme = cx.theme();
        let updates = self
            .rows
            .iter()
            .filter(|r| r.installed && self.selected.contains(&r.id))
            .count();
        let installs = self.selected.len() - updates;
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
                div().h(SEARCH_HEIGHT).px_3().flex().items_center().child(
                    Input::new(&self.search)
                        .appearance(false)
                        .large()
                        .text_size(px(15.))
                        .prefix(Icon::new(IconName::Search).text_color(theme.muted_foreground)),
                ),
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
                    .when(empty, |list| {
                        list.flex().items_center().px_3().text_size(px(12.)).text_color(theme.muted_foreground).child(
                            "No favorites yet. Paste a link or owner/repo in the menu bar popup.",
                        )
                    })
                    .when(!empty && visible.is_empty(), |list| {
                        list.px_3().text_size(px(12.)).text_color(theme.muted_foreground).child("No matches")
                    })
                    .children(rows),
            )
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
                    .child(status.unwrap_or_else(|| return_hint(installs, updates)))
                    .child(div().flex_1())
                    .child("↑↓ move · space select · esc close"),
            )
    }
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
fn return_hint(installs: usize, updates: usize) -> SharedString {
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

fn section_label(suggested: bool, stack_label: &str, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let text: SharedString = match (suggested, stack_label.is_empty()) {
        (true, false) => format!("Suggested for {stack_label}").into(),
        (true, true) => "Suggested".into(),
        (false, _) => "Other favorites".into(),
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
