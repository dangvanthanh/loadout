use std::collections::HashSet;

use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::contrast;
use loadout_core::model::Uuid;
use loadout_core::tags;

use crate::state::{AppState, Scan};

const CONTEXT: &str = "Palette";
/// Tinycast's palette size (DECISIONS 047): fixed, so the window never jumps as rows change.
pub const WIDTH: Pixels = px(750.);
pub const HEIGHT: Pixels = px(475.);
/// Where the palette's top edge sits, as a fraction of the screen's visible height.
pub const TOP_FRACTION: f32 = 0.18;
const SEARCH_HEIGHT: Pixels = px(56.);
const BAR_HEIGHT: Pixels = px(52.);
const ROW_HEIGHT: Pixels = px(40.);
/// Loadout rows have a second line: their skills.
const LOADOUT_ROW_HEIGHT: Pixels = px(48.);
/// The theme tint over the blurred desktop.
const GLASS: f32 = 0.78;
/// The foreground wash behind the highlighted row.
const HIGHLIGHT: f32 = 0.1;
const LABEL_HEIGHT: Pixels = px(28.);
const LIST_PADDING: Pixels = px(8.);
/// Row glyph slot, like Tinycast's result icons.
const TILE: Pixels = px(26.);
const ACTIONS_WIDTH: Pixels = px(320.);
/// Room for a skill's source on the right of a row.
const SOURCE_WIDTH: Pixels = px(340.);

// No key bindings: GPUI runs bindings before key listeners, so an Esc binding would close the
// palette before the handler below could go back from a preview, diff, or menu first.
pub fn init(_: &mut App) {}

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
    /// A loadout's skills, "tdd, pdf, frontend-design"; empty for skills.
    members: SharedString,
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
    /// The ⌘K actions menu is open; index of its highlighted item.
    actions: Option<usize>,
    /// The first ⌘⌫ on an installed row; a second one removes it.
    confirm_uninstall: Option<Uuid>,
    /// The project chooser (⌘P) shows in the list area; index into `target_choices`.
    choosing: Option<usize>,
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
            choosing: None,
            confirm_uninstall: None,
            actions: None,
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
                members: state
                    .data
                    .loadout_favorites(loadout.id)
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
                    .into(),
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
                members: SharedString::default(),
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
    fn diff_key(&mut self, loadout: Uuid, key: &str, launch: bool, cx: &mut Context<Self>) -> bool {
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
                let launch = launch || self.launch_by_default(cx);
                self.state
                    .update(cx, |state, cx| state.equip(&ids, launch, cx));
            }
            _ => return false,
        }
        true
    }

    /// Recent projects, then Global (`None`).
    fn target_choices(&self, cx: &App) -> Vec<Option<std::path::PathBuf>> {
        let mut choices: Vec<_> = self
            .state
            .read(cx)
            .recent_projects()
            .into_iter()
            .take(9)
            .map(Some)
            .collect();
        choices.push(None);
        choices
    }

    /// Keys while the chooser shows: move, pick, close.
    fn choose_key(&mut self, ix: usize, key: &str, cx: &mut Context<Self>) -> bool {
        let choices = self.target_choices(cx);
        match key {
            "escape" => self.choosing = None,
            "up" => self.choosing = Some(ix.saturating_sub(1)),
            "down" => self.choosing = Some((ix + 1).min(choices.len() - 1)),
            "enter" => {
                self.choosing = None;
                let choice = choices.get(ix).cloned().flatten();
                self.state
                    .update(cx, |state, cx| state.choose_target(choice, cx));
            }
            _ => return false,
        }
        self.scroll = ScrollHandle::new();
        cx.notify();
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
        if modifiers.platform && key == "k" {
            self.actions = match self.actions {
                Some(_) => None,
                None => Some(0),
            };
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(ix) = self.actions {
            let count = self.action_items(cx).len();
            match key {
                "escape" => self.actions = None,
                "up" => self.actions = Some(ix.saturating_sub(1)),
                "down" => self.actions = Some((ix + 1).min(count.saturating_sub(1))),
                "enter" => {
                    if let Some(kind) = self.action_items(cx).get(ix).map(|item| item.kind) {
                        self.actions = None;
                        self.run_action(kind, window, cx);
                    }
                }
                // Any other key closes the menu and does its usual job.
                _ => {
                    self.actions = None;
                    cx.notify();
                    return self.on_key_down(event, window, cx);
                }
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if modifiers.platform && key == "s" {
            self.start_naming(window, cx);
            cx.stop_propagation();
            return;
        }
        // ⌘⌫ removes the highlighted installed skill from the target; it asks once first.
        let confirming = self.confirm_uninstall.take();
        if modifiers.platform && key == "backspace" {
            let highlighted = self
                .visible(cx)
                .get(self.highlight)
                .map(|&ix| &self.rows[ix]);
            if let Some(row) = highlighted.filter(|r| r.installed && !r.loadout) {
                let id = row.id;
                if confirming == Some(id) {
                    self.state.update(cx, |state, cx| state.uninstall(id, cx));
                } else {
                    self.confirm_uninstall = Some(id);
                }
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if confirming.is_some() {
            cx.notify();
        }
        // ⌘P picks where installs go; again closes the chooser.
        if modifiers.platform && key == "p" {
            self.choosing = match self.choosing {
                Some(_) => None,
                None => Some(0),
            };
            self.preview = None;
            self.diff = None;
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(ix) = self.choosing {
            if !modifiers.platform && self.choose_key(ix, key, cx) {
                cx.stop_propagation();
            }
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
        // ⌘↩ installs and then opens the agent (U9).
        let launch = modifiers.platform && key == "enter";
        if let Some(loadout) = self.diff {
            let plain = !(modifiers.platform || modifiers.control || modifiers.alt);
            if (plain || launch) && self.diff_key(loadout, key, launch, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if launch {
            self.install(true, cx);
            cx.stop_propagation();
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
            "enter" => {
                let launch = self.launch_by_default(cx);
                self.install(launch, cx)
            }
            // Like Tinycast: Esc clears the search first, then closes.
            "escape" if !query_empty => {
                self.search
                    .update(cx, |search, cx| search.set_value("", window, cx));
            }
            "escape" => window.remove_window(),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// What ⌘K offers for the highlighted row and the selection, in order.
    fn action_items(&self, cx: &App) -> Vec<ActionItem> {
        let highlighted = self
            .visible(cx)
            .get(self.highlight)
            .map(|&ix| &self.rows[ix]);
        let state = self.state.read(cx);
        let mut items = vec![ActionItem {
            label: self.primary_label(cx),
            keys: "enter",
            kind: ActionKind::Install,
        }];
        if let Some(agent) = state.agent_for(&self.chosen(cx)) {
            items.push(ActionItem {
                label: format!(
                    "Install and open {}",
                    loadout_core::launch::agent_name(&agent)
                )
                .into(),
                keys: "cmd-enter",
                kind: ActionKind::InstallAndOpen,
            });
        }
        if let Some(row) = highlighted {
            if row.selectable() {
                let label = if self.selected.contains(&row.id) {
                    "Deselect"
                } else {
                    "Select"
                };
                items.push(ActionItem {
                    label: label.into(),
                    keys: "space",
                    kind: ActionKind::Toggle,
                });
            }
            if row.loadout {
                items.push(ActionItem {
                    label: "Diff against loadout".into(),
                    keys: "cmd-d",
                    kind: ActionKind::Diff,
                });
            } else {
                items.push(ActionItem {
                    label: "Preview".into(),
                    keys: "cmd-y",
                    kind: ActionKind::Preview,
                });
            }
            if row.installed && !row.loadout {
                items.push(ActionItem {
                    label: "Uninstall".into(),
                    keys: "cmd-backspace",
                    kind: ActionKind::Uninstall,
                });
            }
        }
        items.push(ActionItem {
            label: "Save selection as loadout".into(),
            keys: "cmd-s",
            kind: ActionKind::SaveLoadout,
        });
        items.push(ActionItem {
            label: "Choose project".into(),
            keys: "cmd-p",
            kind: ActionKind::ChooseProject,
        });
        items
    }

    fn run_action(&mut self, kind: ActionKind, window: &mut Window, cx: &mut Context<Self>) {
        let highlighted = self
            .visible(cx)
            .get(self.highlight)
            .map(|&ix| self.rows[ix].id);
        match kind {
            ActionKind::Install => {
                let launch = self.launch_by_default(cx);
                self.install(launch, cx)
            }
            ActionKind::InstallAndOpen => self.install(true, cx),
            ActionKind::Toggle => self.toggle_highlighted(cx),
            ActionKind::Preview => self.toggle_preview(highlighted, cx),
            ActionKind::Diff => self.set_diff(highlighted, cx),
            ActionKind::Uninstall => {
                if let Some(id) = highlighted {
                    self.state.update(cx, |state, cx| state.uninstall(id, cx));
                }
            }
            ActionKind::SaveLoadout => self.start_naming(window, cx),
            ActionKind::ChooseProject => {
                self.choosing = Some(0);
                self.preview = None;
                self.diff = None;
            }
        }
        cx.notify();
    }

    /// The capsule's main action for the current mode ("Install 2 skills", "Save loadout", …).
    fn primary_label(&self, cx: &App) -> SharedString {
        if self.naming {
            return "Save loadout".into();
        }
        if self.choosing.is_some() {
            return "Choose".into();
        }
        if self.confirm_uninstall.is_some() {
            return "Uninstall".into();
        }
        if self.state.read(cx).safety_notice.is_some() {
            return "Install anyway".into();
        }
        if let Some(loadout) = self.diff {
            let missing = self.state.read(cx).loadout_diff(loadout).missing;
            let n = missing
                .iter()
                .filter(|id| self.diff_selected.contains(id))
                .count();
            return match n {
                0 => "Nothing selected".into(),
                n => format!("Install {n} missing").into(),
            };
        }
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
        return_hint(installs, updates, &loadouts)
    }

    /// Return: install (or update) the selection, or the highlighted row when nothing is selected.
    fn install(&mut self, launch: bool, cx: &mut Context<Self>) {
        let ids = self.chosen(cx);
        self.state
            .update(cx, |state, cx| state.equip(&ids, launch, cx));
    }

    /// Plain Return opens the agent too when the user chose that in the ⚙ menu.
    fn launch_by_default(&self, cx: &App) -> bool {
        self.state.read(cx).data.settings.launch_after_install
    }

    fn render_row(&self, ix: usize, highlighted: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let row = &self.rows[ix];
        let selected = self.selected.contains(&row.id);
        let id = row.id;
        let warning = readable(theme.warning, glass_floor(theme), contrast::NON_TEXT);
        let muted = muted(theme);
        let is_loadout = row.loadout;
        let this = cx.entity().downgrade();
        let glyph = if is_loadout {
            Glyph::Loadout(row.name.to_string())
        } else {
            Glyph::for_source(&row.source)
        };
        let status = match (&row.findings, row.update, row.installed) {
            (Some(findings), ..) => format!(", {findings}"),
            (_, true, _) => ", update available".into(),
            (_, _, true) => ", installed".into(),
            _ => String::new(),
        };
        let detail = if is_loadout {
            &row.members
        } else {
            &row.source
        };
        let label = format!("{}, {detail}{status}", row.name);
        let element = list_row(("row", ix), highlighted, cx)
            .role(Role::ListBoxOption)
            .aria_label(label)
            .aria_selected(selected)
            .when(is_loadout, |row| row.h(LOADOUT_ROW_HEIGHT))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle(id, cx)))
            .child(tile(selected, glyph, cx))
            .child(
                div()
                    .id(("name", ix))
                    .min_w_0()
                    .flex()
                    .flex_col()
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
                    .child(row_name(row.name.clone()))
                    .when(is_loadout && !row.members.is_empty(), |name| {
                        name.child(
                            div()
                                .truncate()
                                .text_size(px(12.))
                                .text_color(muted)
                                .child(row.members.clone()),
                        )
                    }),
            )
            .child(div().flex_1())
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
                row.child(Icon::new(IconName::Check).small().text_color(muted))
            })
            .child(
                div()
                    .max_w(SOURCE_WIDTH)
                    .truncate()
                    .text_size(px(13.))
                    .text_color(muted)
                    .child(row.source.clone()),
            );
        if !is_loadout {
            if !row.installed {
                return element.into_any_element();
            }
            let state = self.state.clone();
            return element
                .context_menu(move |menu, _, _| {
                    let state = state.clone();
                    menu.item(
                        PopupMenuItem::new("Uninstall").on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| s.uninstall(id, cx))
                        }),
                    )
                })
                .into_any_element();
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

    /// The bottom-right capsule: the main action with ↩, then Actions with ⌘K.
    fn render_capsule(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let fill = theme.foreground.opacity(0.06);
        let label = self.primary_label(cx);
        let (main_keys, aria_keys) = if self.confirm_uninstall.is_some() {
            ("cmd-backspace", "Meta+Backspace")
        } else {
            ("enter", "Enter")
        };
        let on_primary = theme.primary_foreground;
        let part = |id: &'static str, hover: Hsla| {
            div()
                .id(id)
                .role(Role::Button)
                .h(px(30.))
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .rounded_full()
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
        };
        div()
            .h(px(36.))
            .px_1()
            .flex()
            .items_center()
            .gap_1()
            .rounded_full()
            .bg(fill)
            .border_1()
            .border_color(theme.border.opacity(0.6))
            .child(
                part("primary", theme.primary_hover)
                    .aria_label(label.clone())
                    .aria_keyshortcuts(aria_keys)
                    .bg(theme.primary)
                    .text_color(on_primary)
                    .font_weight(FontWeight::SEMIBOLD)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.run_action(ActionKind::Install, window, cx)
                    }))
                    .child(label)
                    .children(
                        keycap(main_keys, theme)
                            .map(|k| k.bg(on_primary.opacity(0.18)).text_color(on_primary)),
                    ),
            )
            .child(
                part("actions", theme.foreground.opacity(0.08))
                    .aria_label("Actions")
                    .aria_keyshortcuts("Meta+K")
                    .text_color(muted(theme))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.actions = Some(0);
                        cx.notify();
                    }))
                    .child("Actions")
                    .children(keycap("cmd-k", theme)),
            )
            .into_any_element()
    }

    /// The ⌘K menu, floating above the capsule.
    fn render_actions(&self, highlight: usize, cx: &mut Context<Self>) -> AnyElement {
        let items = self.action_items(cx);
        let theme = cx.theme();
        let hover = theme.foreground.opacity(0.05);
        let active = theme.foreground.opacity(HIGHLIGHT);
        div()
            .id("actions-menu")
            .role(Role::Menu)
            .aria_label("Actions")
            .absolute()
            .right(px(10.))
            .bottom(BAR_HEIGHT)
            .w(ACTIONS_WIDTH)
            .p_1()
            .flex()
            .flex_col()
            .rounded(px(12.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .children(items.into_iter().enumerate().map(|(ix, item)| {
                div()
                    .id(("action", ix))
                    .role(Role::MenuItem)
                    .aria_label(item.label.clone())
                    .h(px(32.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .text_size(px(13.))
                    .when(ix == highlight, |row| row.bg(active))
                    .when(ix != highlight, |row| row.hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.actions = None;
                        this.run_action(item.kind, window, cx);
                    }))
                    .child(item.label)
                    .child(div().flex_1())
                    .children(keycap(item.keys, theme))
            }))
            .into_any_element()
    }
}

impl Palette {
    /// "Install into": recent projects and Global.
    fn render_chooser(&self, highlight: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.state.read(cx);
        let global = state.data.settings.global;
        let current = state.target.root.clone();
        let choices = self.target_choices(cx);
        let mut items = vec![list_label("Install into".into(), cx)];
        for (ix, choice) in choices.into_iter().enumerate() {
            let is_current = match &choice {
                None => global,
                Some(path) => !global && current.as_ref() == Some(path),
            };
            let (name, detail, icon): (String, String, Icon) = match &choice {
                None => (
                    "Global".into(),
                    "home folder, every project".into(),
                    Icon::new(IconName::Globe),
                ),
                Some(path) => (
                    path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    ),
                    path.display().to_string(),
                    Icon::new(IconName::Folder),
                ),
            };
            let muted = muted(cx.theme());
            items.push(
                list_row(("choice", ix), ix == highlight, cx)
                    .role(Role::ListBoxOption)
                    .aria_label(format!("{name}, {detail}"))
                    .aria_selected(is_current)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_key(ix, "enter", cx);
                    }))
                    .child(tile(false, Glyph::icon(icon), cx))
                    .child(row_name(name))
                    .child(div().flex_1())
                    .when(is_current, |row| {
                        row.child(Icon::new(IconName::Check).small().text_color(muted))
                    })
                    .child(row_detail(detail, muted))
                    .into_any_element(),
            );
        }
        items
    }

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
        let extra = diff.extra;
        let mut items = vec![list_label(
            if missing.is_empty() {
                format!("Nothing missing: this project has everything in {name}").into()
            } else {
                format!("Missing vs {name} ({})", missing.len()).into()
            },
            cx,
        )];
        for (ix, (id, (skill, source))) in missing.into_iter().enumerate() {
            let selected = self.diff_selected.contains(&id);
            let muted = muted(cx.theme());
            items.push(
                list_row(("missing", ix), ix == self.diff_highlight, cx)
                    .role(Role::ListBoxOption)
                    .aria_label(format!("{skill}, {source}, missing"))
                    .aria_selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.diff_selected.remove(&id) {
                            this.diff_selected.insert(id);
                        }
                        cx.notify();
                    }))
                    .child(tile(selected, Glyph::for_source(&source), cx))
                    .child(row_name(skill))
                    .child(div().flex_1())
                    .child(row_detail(source, muted))
                    .into_any_element(),
            );
        }
        // Installed and extra skills are context, not choices: muted names, no highlight. Muted
        // rather than faded, so the text keeps its contrast.
        let quiet =
            |ix: usize, id: &'static str, skill: String, detail: String, glyph: Glyph, cx: &App| {
                let muted = muted(cx.theme());
                div()
                    .id((id, ix))
                    .h(ROW_HEIGHT)
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(tile(false, glyph, cx))
                    .child(row_name(skill).text_color(muted))
                    .child(div().flex_1())
                    .child(row_detail(detail, muted))
                    .into_any_element()
            };
        if !present.is_empty() {
            items.push(list_label(
                format!("Installed ({})", present.len()).into(),
                cx,
            ));
            for (ix, (skill, source)) in present.into_iter().enumerate() {
                let glyph = Glyph::for_source(&source);
                items.push(quiet(ix, "present", skill, source, glyph, cx));
            }
        }
        if !extra.is_empty() {
            items.push(list_label(
                format!("Extra, not in {name} ({})", extra.len()).into(),
                cx,
            ));
            for (ix, skill) in extra.into_iter().enumerate() {
                let glyph = Glyph::icon(IconName::BookOpen);
                items.push(quiet(ix, "extra", skill, String::new(), glyph, cx));
            }
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
        let muted = muted(theme);
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
            _ if self.choosing.is_some() => self.render_chooser(self.choosing.unwrap_or(0), cx),
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
        let capsule = self.render_capsule(cx);
        let actions = self.actions.map(|ix| self.render_actions(ix, cx));
        let target = target_text(self.state.read(cx));
        let chip = target_chip(target, cx);
        let state = self.state.read(cx);
        let detection_notice = state.target.notice.clone();
        let safety = state
            .safety_notice
            .as_ref()
            .map(|n| (n.favorite, n.text.clone()));
        let confirm: Option<SharedString> = self.confirm_uninstall.and_then(|id| {
            let name = self.rows.iter().find(|r| r.id == id)?.name.clone();
            let place = if state.data.settings.global {
                "Global".to_string()
            } else {
                state
                    .target
                    .root
                    .as_ref()?
                    .file_name()?
                    .to_string_lossy()
                    .into_owned()
            };
            Some(format!("Press ⌘⌫ again to remove {name} from {place}").into())
        });
        let status: Option<SharedString> = confirm
            .or_else(|| state.install_status.clone())
            .or_else(
                || match (self.naming, self.choosing, self.diff, self.preview) {
                    (true, ..) => Some("Name the loadout; esc cancels".into()),
                    (_, Some(_), ..) => Some("Where should skills go?".into()),
                    (_, _, Some(_), _) | (_, _, _, Some(_)) => Some("esc goes back".into()),
                    _ => None,
                },
            );
        let theme = cx.theme();
        let muted = muted(theme);
        let warning = readable(theme.warning, glass_floor(theme), contrast::TEXT);
        let empty = self.rows.is_empty();
        let list_mode = self.preview.is_none() && self.diff.is_none() && self.choosing.is_none();
        let search_icon = if self.naming {
            ExtraIcon::Layers.into()
        } else {
            Icon::new(IconName::Search)
        };
        let field = if self.naming {
            &self.name
        } else {
            &self.search
        };

        div()
            .key_context(CONTEXT)
            .capture_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            // Glass: the window blurs what's behind it; this tint keeps text readable.
            .bg(theme.background.opacity(GLASS))
            .text_color(theme.foreground)
            .child(
                div()
                    .h(SEARCH_HEIGHT)
                    .flex_none()
                    .pl_4()
                    .pr_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(search_icon.size(px(18.)).text_color(muted))
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(field)
                                .appearance(false)
                                .large()
                                .text_size(px(20.)),
                        ),
                    )
                    .when(!self.naming, |bar| bar.child(chip)),
            )
            .when_some(detection_notice, |palette, notice| {
                palette.child(
                    div()
                        .px_4()
                        .pb_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(12.))
                        .text_color(warning)
                        .child(Icon::new(ExtraIcon::ShieldAlert).xsmall())
                        .child(div().truncate().child(notice)),
                )
            })
            .child(
                div()
                    .id("favorites")
                    .role(Role::ListBox)
                    .aria_label("Skills and loadouts")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px(LIST_PADDING)
                    .when(empty && list_mode, |list| {
                        list.flex().items_center().justify_center().text_size(px(13.)).text_color(muted).child(
                            "No favorites yet. Add skills in Settings → Skills (⌘, from the menu bar).",
                        )
                    })
                    .when(!empty && visible.is_empty() && list_mode, |list| {
                        list.flex().items_center().justify_center().text_size(px(13.)).text_color(muted).child("No matches")
                    })
                    .children(rows),
            )
            .child(
                div()
                    .h(BAR_HEIGHT)
                    .flex_none()
                    .pl_4()
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(match safety {
                        Some((favorite, text)) => div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(12.))
                            .text_color(warning)
                            .child(Icon::new(ExtraIcon::ShieldAlert).small())
                            .child(div().min_w_0().truncate().child(text))
                            .child(
                                div()
                                    .id("review")
                                    .role(Role::Button)
                                    .flex_none()
                                    .cursor_pointer()
                                    .text_color(theme.foreground)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.preview = None;
                                        this.toggle_preview(Some(favorite), cx)
                                    }))
                                    .child("Review"),
                            ),
                        None => div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(muted)
                            .children(status),
                    })
                    .child(capsule),
            )
            .children(actions)
    }
}

/// A row in the list area: full width, rounded, a soft highlight like Tinycast's. The highlight
/// also gets a 3:1 outline, so it reads as the keyboard focus (WCAG 1.4.11, 2.4.7).
fn list_row(id: impl Into<ElementId>, highlighted: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.foreground.opacity(0.05);
    let ring = readable(theme.border, glass_floor(theme), contrast::NON_TEXT);
    div()
        .id(id)
        .h(ROW_HEIGHT)
        .px(px(10.))
        .flex()
        .items_center()
        .gap_3()
        .rounded(px(10.))
        .border_1()
        .border_color(transparent_black())
        .when(highlighted, |row| {
            row.bg(theme.foreground.opacity(HIGHLIGHT))
                .border_color(ring)
        })
        .when(!highlighted, |row| row.hover(move |s| s.bg(hover)))
}

/// What a row's 26 pt tile shows when it isn't selected.
pub enum Glyph {
    /// The source owner's initial, tinted per owner (skills).
    Monogram(String),
    /// Layers, tinted per loadout name.
    Loadout(String),
    Icon(Box<Icon>),
}

impl Glyph {
    pub fn icon(icon: impl Into<Icon>) -> Self {
        Glyph::Icon(Box::new(icon.into()))
    }

    /// "anthropics/skills" → A, "https://github.com/acme/kit" → A, "https://docs.x.dev" → D.
    pub fn for_source(source: &str) -> Self {
        let source = source.trim();
        let rest = source
            .strip_prefix("https://")
            .or_else(|| source.strip_prefix("http://"))
            .unwrap_or(source);
        let rest = rest
            .strip_prefix("github.com/")
            .or_else(|| rest.strip_prefix("www.github.com/"))
            .or_else(|| rest.strip_prefix("gitlab.com/"))
            .unwrap_or(rest);
        let rest = rest.strip_prefix("git@github.com:").unwrap_or(rest);
        let owner = rest.split(['/', '.']).next().unwrap_or(rest);
        Glyph::Monogram(owner.to_string())
    }
}

/// The 26 pt tile: the item's glyph, or a filled check when selected.
pub fn tile(selected: bool, glyph: Glyph, cx: &App) -> Div {
    let theme = cx.theme();
    let base = div()
        .size(TILE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.));
    if selected {
        return base.bg(theme.primary).child(
            Icon::new(IconName::Check)
                .small()
                .text_color(theme.primary_foreground),
        );
    }
    match glyph {
        Glyph::Icon(icon) => base
            .bg(theme.foreground.opacity(0.07))
            .child(icon.small().text_color(muted(theme))),
        Glyph::Monogram(owner) => {
            let color = tint(&owner, theme);
            let fill = color.opacity(0.18);
            let letter: String = owner
                .chars()
                .next()
                .map_or_else(String::new, |c| c.to_uppercase().collect());
            base.bg(fill)
                .text_color(readable(
                    color,
                    glass_floor(theme).blend(fill),
                    contrast::TEXT,
                ))
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(letter)
        }
        Glyph::Loadout(name) => {
            let color = tint(&name, theme);
            let fill = color.opacity(0.18);
            base.bg(fill)
                .child(Icon::new(ExtraIcon::Layers).small().text_color(readable(
                    color,
                    glass_floor(theme).blend(fill),
                    contrast::NON_TEXT,
                )))
        }
    }
}

/// A theme chart color picked by name, so one author's skills (or one loadout) keep a color.
fn tint(name: &str, theme: &Theme) -> Hsla {
    let colors = [
        theme.chart_1,
        theme.chart_2,
        theme.chart_3,
        theme.chart_4,
        theme.chart_5,
    ];
    let hash = name.bytes().fold(0usize, |h, b| {
        h.wrapping_mul(31)
            .wrapping_add(b.to_ascii_lowercase() as usize)
    });
    colors[hash % colors.len()]
}

/// A keycap for a shortcut like "cmd-k"; `None` if it doesn't parse.
fn keycap(keys: &'static str, theme: &Theme) -> Option<Kbd> {
    let text = readable(
        theme.muted_foreground,
        glass_floor(theme).blend(theme.muted),
        contrast::TEXT,
    );
    Keystroke::parse(keys)
        .ok()
        .map(|k| Kbd::new(k).text_color(text))
}

/// The darkest the glass gets in light mode (the lightest in dark mode): the tint over a desktop
/// of the opposite brightness, under the highlight wash. Colors readable on it are readable
/// anywhere in the palette.
pub fn glass_floor(theme: &Theme) -> Hsla {
    let behind = if theme.is_dark() { white() } else { black() };
    behind
        .blend(theme.background.opacity(GLASS))
        .blend(theme.foreground.opacity(HIGHLIGHT))
}

/// `color`, darkened (or lightened on dark backgrounds) until it has `min` contrast on `bg`.
pub fn readable(color: Hsla, bg: Hsla, min: f32) -> Hsla {
    let rgb = |c: Hsla| {
        let c = c.to_rgb();
        [c.r, c.g, c.b]
    };
    let bg = rgb(bg);
    // Below this luminance white has more room than black.
    let step = if contrast::luminance(bg) > 0.18 {
        -0.02
    } else {
        0.02
    };
    let mut color = color;
    while contrast::ratio(rgb(color), bg) < min && (0.0..=1.0).contains(&(color.l + step)) {
        color.l += step;
    }
    color
}

/// Muted text that keeps 4.5:1 on the glass.
pub fn muted(theme: &Theme) -> Hsla {
    readable(theme.muted_foreground, glass_floor(theme), contrast::TEXT)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    Install,
    InstallAndOpen,
    Toggle,
    Preview,
    Diff,
    Uninstall,
    SaveLoadout,
    ChooseProject,
}

#[derive(Clone)]
struct ActionItem {
    label: SharedString,
    keys: &'static str,
    kind: ActionKind,
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
        .text_color(readable(
            theme.primary,
            glass_floor(theme).blend(theme.primary.opacity(0.12)),
            contrast::TEXT,
        ))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .child("Update")
}

/// The main action's words for what's selected.
fn return_hint(installs: usize, updates: usize, loadouts: &[SharedString]) -> SharedString {
    let skills_hint = skills_hint(installs, updates);
    match loadouts {
        [] => skills_hint,
        [one] if installs + updates == 0 => format!("Apply {one}").into(),
        [one] => format!("Apply {one} + {}", skills_hint.to_lowercase()).into(),
        many => format!("Apply {} loadouts", many.len()).into(),
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
        (0, 0) => "Install".into(),
        (n, 0) => format!("Install {}", skills(n)).into(),
        (0, n) => format!("Update {}", skills(n)).into(),
        (a, b) => format!("Install {a}, update {b}").into(),
    }
}

fn section_label(section: Section, stack_label: &str, cx: &App) -> AnyElement {
    let text: SharedString = match (section, stack_label.is_empty()) {
        (Section::Suggested, false) => format!("Suggested for {stack_label}").into(),
        (Section::Suggested, true) => "Suggested".into(),
        (Section::Loadouts, _) => "Loadouts".into(),
        (Section::Other, _) => "Other favorites".into(),
    };
    list_label(text, cx)
}

/// A section label in the list area ("Suggested for Next.js", "Install into").
fn list_label(text: SharedString, cx: &App) -> AnyElement {
    div()
        .h(LABEL_HEIGHT)
        .px(px(10.))
        .pt_1()
        .flex()
        .items_center()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(muted(cx.theme()))
        .child(text)
        .into_any_element()
}

/// A row's title: 14 pt, truncated.
fn row_name(text: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .text_size(px(14.))
        .truncate()
        .child(text.into())
}

/// A row's right-hand detail: 13 pt muted, truncated.
fn row_detail(text: impl Into<SharedString>, muted: Hsla) -> Div {
    div()
        .max_w(SOURCE_WIDTH)
        .truncate()
        .text_size(px(13.))
        .text_color(muted)
        .child(text.into())
}

/// The target's two lines and tooltip.
struct TargetText {
    global: bool,
    name: SharedString,
    /// The stack when known, else where the target came from.
    detail: SharedString,
    tip: SharedString,
}

/// Where Return installs, and where that came from.
fn target_text(state: &AppState) -> TargetText {
    let target = &state.target;
    let global = state.data.settings.global;
    let name: SharedString = if global {
        "Global".into()
    } else {
        target
            .root
            .as_ref()
            .and_then(|root| root.file_name())
            .map_or_else(
                || "No project".into(),
                |n| n.to_string_lossy().into_owned().into(),
            )
    };
    let origin = if global {
        "Installs to your home folder".to_string()
    } else if target.chosen {
        "Chosen".to_string()
    } else if target.detecting {
        "Finding the project…".to_string()
    } else if let Some(app) = &target.from_app {
        format!("From {app}")
    } else {
        "Last used".to_string()
    };
    let stack = state.stack_summary();
    let tip: SharedString = if stack.is_empty() || global {
        format!("{origin} · ⌘P to change").into()
    } else {
        format!("{origin} · {stack} · ⌘P to change").into()
    };
    let detail: SharedString = if global {
        "Home folder".into()
    } else if stack.is_empty() || target.detecting {
        origin.into()
    } else {
        stack.into()
    };
    TargetText {
        global,
        name,
        detail,
        tip,
    }
}

/// Where Return installs, on the search row: a folder tile, the project over its stack, and a
/// caret; ⌘P or a click changes it.
fn target_chip(target: TargetText, cx: &mut Context<Palette>) -> AnyElement {
    let theme = cx.theme();
    let hover = theme.foreground.opacity(0.08);
    let icon = if target.global {
        IconName::Globe
    } else {
        IconName::Folder
    };
    let label = format!("Install into {}, {}", target.name, target.detail);
    let tip = target.tip;
    div()
        .id("target")
        .role(Role::Button)
        .aria_label(label)
        .aria_keyshortcuts("Meta+P")
        .h(px(40.))
        .pl(px(7.))
        .pr_3()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .rounded(px(10.))
        .bg(theme.foreground.opacity(0.05))
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .text_color(muted(theme))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .on_click(cx.listener(|this, _, _, cx| {
            this.choosing = Some(0);
            this.preview = None;
            this.diff = None;
            cx.notify();
        }))
        .child(
            div()
                .size(TILE)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.))
                .bg(theme.blue.opacity(0.15))
                .child(Icon::new(icon).small().text_color(theme.blue)),
        )
        .child(
            div()
                .max_w(px(180.))
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(target.name),
                )
                .child(div().truncate().text_size(px(11.)).child(target.detail)),
        )
        .child(Icon::new(IconName::ChevronDown).xsmall())
        .into_any_element()
}
