//! The Settings window: a sidebar of pages, like Rex's settings (DECISIONS 050). Everything the
//! old menu bar popup managed lives here; the menu bar keeps a small menu.

mod library;

use std::path::PathBuf;

use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, IconName, Sizable, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::launch::{self, Launch};
use loadout_core::model::{AppIcon, IconPlace, ProvenanceEntry, Uuid};
use loadout_core::shortcut::{self, Mods};
use loadout_core::{agents, digest, provenance};

use self::library::{Field, ROW_HEIGHT, folder_name};
use crate::assets::app_icon_path;
use crate::hotkey;
use crate::macos::login_item::LoginItem;
use crate::state::{AppState, CheckReason};
use crate::views::palette::{Glyph, tile};

const SIDEBAR_WIDTH: Pixels = px(220.);
/// Room for the traffic lights above the sidebar and beside the page title.
const TITLE_BAR: Pixels = px(52.);
const SETTING_ROW: Pixels = px(44.);
/// Most history lines shown.
const HISTORY_LINES: usize = 300;

/// The sidebar's pages, in groups.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Interface,
    Shortcuts,
    Integrations,
    Notifications,
    Skills,
    Loadouts,
    Projects,
    History,
}

impl Page {
    const GROUPS: [&'static [Page]; 2] = [
        &[
            Page::Interface,
            Page::Shortcuts,
            Page::Integrations,
            Page::Notifications,
        ],
        &[Page::Skills, Page::Loadouts, Page::Projects, Page::History],
    ];

    fn title(self) -> &'static str {
        match self {
            Page::Interface => "Interface",
            Page::Shortcuts => "Shortcuts",
            Page::Integrations => "Integrations",
            Page::Notifications => "Notifications",
            Page::Skills => "Skills",
            Page::Loadouts => "Loadouts",
            Page::Projects => "Projects",
            Page::History => "History",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::Interface => Icon::new(IconName::LayoutDashboard),
            Page::Shortcuts => ExtraIcon::Command.into(),
            Page::Integrations => Icon::new(IconName::Bot),
            Page::Notifications => Icon::new(IconName::Bell),
            Page::Skills => Icon::new(IconName::BookOpen),
            Page::Loadouts => ExtraIcon::Layers.into(),
            Page::Projects => Icon::new(IconName::Folder),
            Page::History => ExtraIcon::Clock.into(),
        }
    }

    /// The colored square behind the sidebar icon, from the theme's named colors.
    fn color(self, theme: &Theme) -> Hsla {
        match self {
            Page::Interface => theme.blue,
            Page::Shortcuts => theme.muted_foreground,
            Page::Integrations => theme.magenta,
            Page::Notifications => theme.red,
            Page::Skills => theme.green,
            Page::Loadouts => theme.cyan,
            Page::Projects => theme.yellow,
            Page::History => theme.muted_foreground,
        }
    }
}

/// A view inside a page that replaces its list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Detail {
    None,
    /// A pending update's files and SKILL.md diff (Skills).
    Changes(Uuid),
    /// One loadout's skills (Loadouts).
    Loadout(Uuid),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HistoryFilter {
    All,
    /// The project last detected or chosen.
    Project,
    Skill(Uuid),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HistoryTab {
    History,
    /// Output of `skills` runs.
    Output,
}

pub struct SettingsWindow {
    state: Entity<AppState>,
    page: Page,
    detail: Detail,
    history: HistoryFilter,
    history_tab: HistoryTab,
    focus: FocusHandle,
    /// Skills: paste a link or owner/repo.
    paste: Entity<InputState>,
    /// The inline rename / tags / loadout field being edited.
    edit: Entity<InputState>,
    editing: Option<(Uuid, Field)>,
    /// Shortcuts: the palette shortcut waits for the next keystroke.
    recording: bool,
    hotkey_error: Option<SharedString>,
    shortcut_filter: Entity<InputState>,
    token: Entity<InputState>,
    /// Integrations: a launch command per known agent.
    launchers: Vec<(&'static str, Entity<InputState>)>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl SettingsWindow {
    pub fn new(
        state: Entity<AppState>,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let paste = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Paste a link, owner/repo, or an npx skills add command")
        });
        let edit = cx.new(|cx| InputState::new(window, cx));
        let shortcut_filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter shortcuts…"));
        let (saved_token, saved_launchers) = {
            let state = state.read(cx);
            (
                state.token.clone().unwrap_or_default(),
                state.data.settings.launchers.clone(),
            )
        };
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("ghp_… (optional)")
                .masked(true)
                .default_value(saved_token)
        });
        let launchers: Vec<(&'static str, Entity<InputState>)> = agents::AGENTS
            .iter()
            .map(|agent| {
                let saved = saved_launchers.get(agent.id).cloned().unwrap_or_default();
                let placeholder = default_launch(agent.id);
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(placeholder)
                        .default_value(saved)
                });
                (agent.id, input)
            })
            .collect();

        let mut subscriptions = vec![
            cx.observe(&state, |this, state, cx| {
                // A detail whose subject went away (updated, removed) closes.
                let state = state.read(cx);
                match this.detail {
                    Detail::Changes(id) if !state.pending().iter().any(|p| p.favorite == id) => {
                        this.detail = Detail::None
                    }
                    Detail::Loadout(id) if state.data.loadout(id).is_none() => {
                        this.detail = Detail::None
                    }
                    _ => {}
                }
                cx.notify();
            }),
            cx.observe_window_appearance(window, |_, window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
            cx.subscribe_in(
                &paste,
                window,
                |this, paste, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let text = paste.read(cx).value().to_string();
                        this.state
                            .update(cx, |state, cx| state.add_from_text(&text, cx));
                        paste.update(cx, |paste, cx| paste.set_value("", window, cx));
                    }
                },
            ),
            cx.subscribe_in(
                &edit,
                window,
                |this, edit, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => {
                        let text = edit.read(cx).value().to_string();
                        if let Some((id, field)) = this.editing.take() {
                            this.state.update(cx, |state, cx| match field {
                                Field::Name => state.rename_favorite(id, &text, cx),
                                Field::Tags => state.set_favorite_tags(id, &text, cx),
                                Field::LoadoutName => state.rename_loadout(id, &text, cx),
                                Field::LoadoutTags => state.set_loadout_tags(id, &text, cx),
                                Field::LoadoutAgent => state.set_loadout_agent(id, &text, cx),
                                Field::LoadoutVersion => state.set_loadout_version(id, &text, cx),
                                Field::LoadoutSource => state.set_loadout_source(id, &text, cx),
                            });
                        }
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }
                    InputEvent::Blur => {
                        this.editing = None;
                        cx.notify();
                    }
                    _ => {}
                },
            ),
            cx.subscribe(&shortcut_filter, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&token, |this, input, event: &InputEvent, cx| {
                // Only Return saves: a half-pasted token shouldn't reach the Keychain on blur.
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().to_string();
                    this.state
                        .update(cx, |state, cx| state.set_token(&text, cx));
                }
            }),
        ];
        for (agent, input) in &launchers {
            let agent = *agent;
            subscriptions.push(
                cx.subscribe(input, move |this, input, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        let text = input.read(cx).value().to_string();
                        let saved = this
                            .state
                            .read(cx)
                            .data
                            .settings
                            .launchers
                            .get(agent)
                            .cloned();
                        if saved.unwrap_or_default() != text.trim() {
                            this.state
                                .update(cx, |state, cx| state.set_launcher(agent, &text, cx));
                        }
                    }
                }),
            );
        }
        Self {
            state,
            page,
            detail: Detail::None,
            history: HistoryFilter::All,
            history_tab: HistoryTab::History,
            focus,
            paste,
            edit,
            editing: None,
            recording: false,
            hotkey_error: None,
            shortcut_filter,
            token,
            launchers,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// From outside (the menu, a notification, an import).
    pub fn show_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.go(page, Detail::None, cx);
    }

    fn go(&mut self, page: Page, detail: Detail, cx: &mut Context<Self>) {
        if page != self.page {
            self.state.update(cx, |state, _| state.notice = None);
        }
        if let Detail::Changes(id) = detail {
            self.state
                .update(cx, |state, cx| state.show_changes(id, cx));
        }
        self.page = page;
        self.detail = detail;
        self.editing = None;
        self.recording = false;
        self.scroll = ScrollHandle::new();
        if page == Page::History && self.history_tab == HistoryTab::Output {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let m = keystroke.modifiers;
        if self.recording {
            if key == "escape" {
                self.recording = false;
            } else {
                let mods = Mods {
                    ctrl: m.control,
                    alt: m.alt,
                    shift: m.shift,
                    cmd: m.platform,
                };
                match shortcut::from_keystroke(mods, key) {
                    Some(spec) => self.set_hotkey(&spec, cx),
                    None => {
                        self.hotkey_error =
                            Some("Use ⌃, ⌥, or ⌘ with a letter, number, or key.".into())
                    }
                }
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }
        match (m.platform, key) {
            (true, "w") => window.remove_window(),
            (false, "escape") => {
                if self.editing.take().is_some() {
                    window.focus(&self.focus, cx);
                } else if self.detail != Detail::None {
                    self.detail = Detail::None;
                } else if self.state.read(cx).pending_import.is_some() {
                    self.state.update(cx, |state, cx| state.cancel_import(cx));
                } else {
                    return;
                }
            }
            _ => return,
        }
        cx.notify();
        cx.stop_propagation();
    }

    fn set_hotkey(&mut self, spec: &str, cx: &mut Context<Self>) {
        let ok = self
            .state
            .update(cx, |state, cx| state.set_hotkey(spec, cx));
        self.recording = false;
        self.hotkey_error = (!ok).then(|| {
            format!(
                "Couldn't use {}. Another app may own it.",
                shortcut::display(spec)
            )
            .into()
        });
    }
}

/// What Integrations shows as an agent's default launch, in its field's placeholder.
fn default_launch(agent: &str) -> SharedString {
    match launch::launcher(agent, std::path::Path::new("{dir}"), &Default::default()) {
        Some(Launch::Terminal(command)) => format!("Default: {command} in Terminal").into(),
        Some(Launch::App(app)) => format!("Default: open in {app}").into(),
        _ => "A shell command; {dir} is the project folder".into(),
    }
}

// MARK: building blocks

/// "Section" title above a card.
fn section_title(text: impl Into<SharedString>) -> Div {
    div()
        .pt_2()
        .pb(px(6.))
        .px_1()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}

/// A muted explanation under a section title or a card.
fn note(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .px_1()
        .pb_2()
        .text_size(px(11.))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

/// Rows on a rounded card, divided by hairlines.
fn card(rows: Vec<AnyElement>, cx: &App) -> Div {
    let theme = cx.theme();
    let count = rows.len();
    let divider = theme.border.opacity(0.7);
    div()
        .flex()
        .flex_col()
        .rounded(px(10.))
        .bg(theme.foreground.opacity(0.035))
        .border_1()
        .border_color(theme.border.opacity(0.6))
        .children(rows.into_iter().enumerate().map(move |(ix, row)| {
            div()
                .when(ix + 1 < count, |row| row.border_b_1().border_color(divider))
                .child(row)
        }))
}

/// A setting: label and hint on the left, its control on the right.
fn setting(
    label: impl Into<SharedString>,
    hint: Option<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    div()
        .min_h(SETTING_ROW)
        .px(px(14.))
        .py_2()
        .flex()
        .items_center()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(px(13.)).child(label.into()))
                .children(hint.map(|hint| div().text_size(px(11.)).text_color(muted).child(hint))),
        )
        .child(control)
        .into_any_element()
}

/// Options in a pill, the chosen one raised (Rex's "Snappy · Natural · Off").
fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &[(T, &'static str)],
    current: T,
    on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let on_pick = std::rc::Rc::new(on_pick);
    div()
        .p(px(2.))
        .flex()
        .rounded_full()
        .bg(theme.foreground.opacity(0.06))
        .children(options.iter().enumerate().map(|(ix, (value, label))| {
            let value = *value;
            let chosen = value == current;
            let on_pick = on_pick.clone();
            div()
                .id((id, ix))
                .h(px(24.))
                .px_3()
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(12.))
                .cursor_pointer()
                .when(chosen, |pill| {
                    pill.bg(theme.background)
                        .border_1()
                        .border_color(theme.border)
                        .font_weight(FontWeight::MEDIUM)
                })
                .when(!chosen, |pill| pill.text_color(theme.muted_foreground))
                .on_click(move |_, window, cx| on_pick(value, window, cx))
                .child(*label)
        }))
}

/// A shortcut shown like a key on a keyboard, Rex-style: a wide gray box.
fn key_box(text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .w(px(160.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .bg(theme.foreground.opacity(0.08))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .child(text.into())
}

// MARK: pages

impl SettingsWindow {
    fn render_interface(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.state.read(cx);
        let settings = &state.data.settings;
        let (current_icon, place) = (settings.app_icon, settings.icon_place);
        let show_on_launch = settings.show_window_on_launch;
        let login_on = state.login_item != LoginItem::Off;
        let theme = cx.theme();
        let icons = div()
            .p_3()
            .flex()
            .gap_3()
            .children(AppIcon::ALL.iter().map(|&icon| {
                let chosen = icon == current_icon;
                let state = self.state.clone();
                div()
                    .id(SharedString::from(format!("icon-{}", icon.label())))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| state.update(cx, |s, cx| s.set_app_icon(icon, cx)))
                    .child(
                        div()
                            .w_full()
                            .h(px(96.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(12.))
                            .bg(theme.foreground.opacity(0.05))
                            .border_2()
                            .border_color(if chosen {
                                theme.primary
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .child(img(app_icon_path(icon)).size(px(64.))),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .when(chosen, |label| label.font_weight(FontWeight::SEMIBOLD))
                            .when(!chosen, |label| label.text_color(theme.muted_foreground))
                            .child(icon.label()),
                    )
            }));
        let place_button = {
            let state = self.state.clone();
            Button::new("icon-place")
                .ghost()
                .small()
                .label(place.label())
                .dropdown_caret(true)
                .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                    let mut menu = menu;
                    for option in IconPlace::ALL {
                        let state = state.clone();
                        menu = menu.item(
                            PopupMenuItem::new(option.label())
                                .checked(option == place)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| s.set_icon_place(option, cx))
                                }),
                        );
                    }
                    menu
                })
        };
        let place_hint: Option<SharedString> = match place {
            IconPlace::Dock => Some(
                "Without the menu bar icon, click Loadout in the Dock or press the palette shortcut."
                    .into(),
            ),
            IconPlace::DockAndMenuBar => Some("The Dock icon uses the app icon above.".into()),
            IconPlace::MenuBar => None,
        };
        let switch =
            |id: &'static str, on: bool, set: fn(&mut AppState, bool, &mut Context<AppState>)| {
                let state = self.state.clone();
                Switch::new(id)
                    .checked(on)
                    .on_click(move |on, _, cx| state.update(cx, |s, cx| set(s, *on, cx)))
            };
        vec![
            section_title("App Icon").into_any_element(),
            card(
                vec![
                    icons.into_any_element(),
                    setting("Show app icon", place_hint, place_button, cx),
                ],
                cx,
            )
            .into_any_element(),
            section_title("Startup").into_any_element(),
            card(
                vec![
                    setting(
                        "Show window on launch",
                        Some("Open these settings when Loadout starts.".into()),
                        switch(
                            "show-on-launch",
                            show_on_launch,
                            AppState::set_show_window_on_launch,
                        ),
                        cx,
                    ),
                    setting(
                        "Open at login",
                        Some("Needs Loadout.app in Applications.".into()),
                        switch("login", login_on, AppState::set_open_at_login),
                        cx,
                    ),
                ],
                cx,
            )
            .into_any_element(),
        ]
    }

    fn render_shortcuts(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let filter = self.shortcut_filter.read(cx).value().to_lowercase();
        let filter = filter.trim().to_string();
        let spec = self.state.read(cx).data.settings.hotkey.clone();
        let matches = |label: &str, keys: &str| {
            filter.is_empty()
                || label.to_lowercase().contains(&filter)
                || keys.to_lowercase().contains(&filter)
        };
        let mut items = vec![
            div()
                .h(px(32.))
                .px_3()
                .flex()
                .items_center()
                .rounded_full()
                .bg(cx.theme().foreground.opacity(0.06))
                .child(
                    Input::new(&self.shortcut_filter)
                        .appearance(false)
                        .small()
                        .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground)),
                )
                .into_any_element(),
            note(
                "The palette shortcut works in every app; click it to record a new one. The others work inside Loadout's windows.",
                cx,
            )
            .pt_2()
            .into_any_element(),
        ];
        let palette_keys = shortcut::display(&spec);
        if matches("open palette", &palette_keys) {
            let recording = self.recording;
            let default = spec == hotkey::DEFAULT;
            let recorder = div()
                .flex()
                .items_center()
                .gap_2()
                .when(!default, |row| {
                    row.child(
                        Button::new("reset-hotkey")
                            .ghost()
                            .xsmall()
                            .label("Reset")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_hotkey(hotkey::DEFAULT, cx);
                                cx.notify();
                            })),
                    )
                })
                .child(
                    div()
                        .id("record-hotkey")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.recording = !this.recording;
                            this.hotkey_error = None;
                            window.focus(&this.focus, cx);
                            cx.notify();
                        }))
                        .child(
                            key_box(
                                if recording {
                                    "Press a shortcut…".to_string()
                                } else {
                                    palette_keys.clone()
                                },
                                cx,
                            )
                            .when(recording, |key| {
                                key.border_1().border_color(cx.theme().primary)
                            }),
                        ),
                );
            items.push(section_title("Global").into_any_element());
            items.push(
                card(
                    vec![setting(
                        "Open Palette",
                        self.hotkey_error
                            .clone()
                            .or_else(|| recording.then(|| "Esc cancels.".into())),
                        recorder,
                        cx,
                    )],
                    cx,
                )
                .into_any_element(),
            );
        }
        let groups: [(&str, &[(&str, &str)]); 3] = [
            (
                "Palette",
                &[
                    ("Install or update the selection", "↩"),
                    ("Install and open the agent", "⌘↩"),
                    ("Actions for the highlighted row", "⌘K"),
                    ("Select or deselect", "Space"),
                    ("Preview a skill", "⌘Y"),
                    ("Diff against a loadout", "⌘D"),
                    ("Save the selection as a loadout", "⌘S"),
                    ("Choose the project", "⌘P"),
                    ("Uninstall (press twice)", "⌘⌫"),
                    ("Clear the search, go back, or close", "Esc"),
                ],
            ),
            (
                "Menu",
                &[
                    ("Update all", "⌘U"),
                    ("Check for updates", "⌘R"),
                    ("Settings…", "⌘,"),
                    ("Quit Loadout", "⌘Q"),
                ],
            ),
            ("Settings", &[("Close Settings", "⌘W"), ("Go back", "Esc")]),
        ];
        for (title, rows) in groups {
            let rows: Vec<AnyElement> = rows
                .iter()
                .filter(|(label, keys)| matches(label, keys))
                .map(|(label, keys)| setting(*label, None, key_box(*keys, cx), cx))
                .collect();
            if !rows.is_empty() {
                items.push(section_title(title).into_any_element());
                items.push(card(rows, cx).into_any_element());
            }
        }
        items
    }

    fn render_integrations(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.state.read(cx);
        let chosen = agents::parse_list(&state.data.settings.agents);
        let has_token = state.token.is_some();
        let launch_after = state.data.settings.launch_after_install;
        let agent_rows: Vec<AnyElement> = agents::AGENTS
            .iter()
            .enumerate()
            .map(|(ix, agent)| {
                let on = chosen.iter().any(|id| id == agent.id);
                let state = self.state.clone();
                let id = agent.id;
                setting(
                    agent.name,
                    Some(format!("{} · {}", agent.id, agent.project_dir).into()),
                    Checkbox::new(("agent", ix))
                        .checked(on)
                        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.toggle_agent(id, cx))),
                    cx,
                )
            })
            .collect();
        let unknown: Vec<&String> = chosen
            .iter()
            .filter(|id| agents::find(id).is_none())
            .collect();
        let summary: SharedString = if chosen.is_empty() {
            "None checked: the skills CLI detects the agents you have.".into()
        } else {
            let names: Vec<&str> = chosen
                .iter()
                .map(|id| agents::find(id).map_or(id.as_str(), |a| a.name))
                .collect();
            format!("Skills install for {}.", names.join(", ")).into()
        };
        let launch_rows: Vec<AnyElement> = self
            .launchers
            .iter()
            .filter(|(agent, _)| chosen.iter().any(|id| id == agent))
            .map(|(agent, input)| {
                setting(
                    launch::agent_name(agent),
                    None,
                    div().w(px(320.)).child(Input::new(input).small()),
                    cx,
                )
            })
            .collect();
        let mut items = vec![
            section_title("Coding Agents").into_any_element(),
            note(summary, cx).into_any_element(),
        ];
        if !unknown.is_empty() {
            let unknown: Vec<&str> = unknown.iter().map(|s| s.as_str()).collect();
            items.push(
                note(
                    format!("Also passed to the CLI: {}.", unknown.join(", ")),
                    cx,
                )
                .into_any_element(),
            );
        }
        items.push(card(agent_rows, cx).into_any_element());
        items.push(section_title("Launch").into_any_element());
        items.push(
            card(
                vec![setting(
                    "Open the agent after installing",
                    Some("Return does what ⌘↩ does: install, then open the loadout's preferred agent (or the first one checked).".into()),
                    {
                        let state = self.state.clone();
                        Switch::new("launch-after")
                            .checked(launch_after)
                            .on_click(move |on, _, cx| {
                                state.update(cx, |s, cx| s.set_launch_after_install(*on, cx))
                            })
                    },
                    cx,
                )],
                cx,
            )
            .into_any_element(),
        );
        if !launch_rows.is_empty() {
            items.push(section_title("Launch Commands").into_any_element());
            items.push(
                note(
                    "⌘↩ in the palette opens the agent after installing. Leave a field empty for the default, or write a shell command; {dir} is the project folder. Return saves.",
                    cx,
                )
                .into_any_element(),
            );
            items.push(card(launch_rows, cx).into_any_element());
        }
        items.push(section_title("GitHub").into_any_element());
        items.push(
            card(
                vec![setting(
                    "Token",
                    Some(
                        if has_token {
                            "Saved in your Keychain: 5,000 requests an hour."
                        } else {
                            "Optional. Without one GitHub allows 60 requests an hour. Return saves it to your Keychain."
                        }
                        .into(),
                    ),
                    div()
                        .w(px(260.))
                        .child(Input::new(&self.token).small().mask_toggle()),
                    cx,
                )],
                cx,
            )
            .into_any_element(),
        );
        items
    }

    fn render_notifications(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let settings = &self.state.read(cx).data.settings;
        let digest_now = settings
            .digest_enabled
            .then_some(settings.digest_interval_hours);
        let auto_check = settings.auto_check;
        let digest_hint: SharedString = if digest_now.is_some() {
            "One quiet summary when updates are pending, and no other update notifications.".into()
        } else {
            "Off: each new version gets its own notification.".into()
        };
        let state = self.state.clone();
        let digest_control = segmented(
            "digest",
            &[
                (None, "Off"),
                (Some(digest::DAILY), "Daily"),
                (Some(digest::WEEKLY), "Weekly"),
            ],
            digest_now,
            move |interval, _, cx| state.update(cx, |s, cx| s.set_digest(interval, cx)),
            cx,
        );
        let check = {
            let state = self.state.clone();
            Switch::new("auto-check")
                .checked(auto_check)
                .on_click(move |on, _, cx| state.update(cx, |s, cx| s.set_auto_check(*on, cx)))
        };
        let test = {
            let state = self.state.clone();
            Button::new("test-notification")
                .ghost()
                .small()
                .label("Send")
                .on_click(move |_, _, cx| state.update(cx, |s, cx| s.send_test_notification(cx)))
        };
        vec![
            section_title("Updates").into_any_element(),
            card(
                vec![
                    setting(
                        "Check for updates automatically",
                        Some("Every 6 hours, only for skills Loadout installed.".into()),
                        check,
                        cx,
                    ),
                    setting("Update digest", Some(digest_hint), digest_control, cx),
                ],
                cx,
            )
            .into_any_element(),
            section_title("Test").into_any_element(),
            card(
                vec![setting(
                    "Test notification",
                    Some("Notifications only work from Loadout.app.".into()),
                    test,
                    cx,
                )],
                cx,
            )
            .into_any_element(),
        ]
    }

    fn render_skills(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if let Detail::Changes(id) = self.detail {
            let name = self
                .state
                .read(cx)
                .data
                .favorites
                .iter()
                .find(|f| f.id == id)
                .map_or_else(String::new, |f| f.name.clone());
            let mut items =
                vec![self.view_header(format!("What changed in {name}").into(), Page::Skills, cx)];
            items.extend(self.render_changes(id, cx).into_iter().skip(1));
            return items;
        }
        let state = self.state.read(cx);
        let count = state.data.favorites.len();
        let pending = state.pending().len();
        let busy = state.installing;
        let checking = state.checking;
        let check_status = state.check_status();
        let theme = cx.theme();
        let mut items = vec![
            div()
                .h(px(34.))
                .px_3()
                .flex()
                .items_center()
                .rounded_full()
                .bg(theme.foreground.opacity(0.06))
                .child(
                    Input::new(&self.paste)
                        .appearance(false)
                        .small()
                        .prefix(Icon::new(IconName::Plus).text_color(theme.muted_foreground)),
                )
                .into_any_element(),
            note(
                "owner/repo, owner/repo@skill, GitHub, GitLab, skills.sh, or ui-skills.com links. Return adds it as a favorite; the palette installs it.",
                cx,
            )
            .pt_2()
            .into_any_element(),
        ];
        let header = div()
            .flex()
            .items_center()
            .child(section_title(format!("Favorites ({count})")))
            .child(div().flex_1())
            .children(check_status.map(|status| {
                div()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child(status)
            }))
            .child(
                Button::new("check-now")
                    .ghost()
                    .xsmall()
                    .icon(IconName::RotateCw)
                    .tooltip("Check for updates")
                    .accessibility_label("Check for updates")
                    .disabled(checking)
                    .on_click({
                        let state = self.state.clone();
                        move |_, _, cx| {
                            state.update(cx, |s, cx| s.check_for_updates(CheckReason::Manual, cx))
                        }
                    }),
            )
            .when(pending > 0, |header| {
                let state = self.state.clone();
                header.child(
                    Button::new("update-all")
                        .primary()
                        .xsmall()
                        .label(format!("Update All ({pending})"))
                        .disabled(busy)
                        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.update(None, cx))),
                )
            });
        items.push(header.into_any_element());
        if count == 0 {
            items.push(note("No favorites yet. Paste one above.", cx).into_any_element());
        } else {
            let rows: Vec<AnyElement> = (0..count)
                .map(|ix| {
                    div()
                        .px_1()
                        .child(self.render_favorite(ix, cx))
                        .into_any_element()
                })
                .collect();
            items.push(card(rows, cx).p_1().into_any_element());
            items.push(
                note(
                    "Right-click a skill to rename, tag, pin, uninstall, or see its history.",
                    cx,
                )
                .pt_2()
                .into_any_element(),
            );
        }
        items
    }

    fn render_loadouts_page(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items = Vec::new();
        if self.state.read(cx).pending_import.is_some() {
            items.push(section_title("Import").into_any_element());
            let rows = self.render_import(cx);
            items.push(
                card(
                    vec![
                        div()
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(rows)
                            .into_any_element(),
                    ],
                    cx,
                )
                .into_any_element(),
            );
        }
        if let Detail::Loadout(id) = self.detail {
            let mut members = self.render_loadout_members(id, cx);
            let header = members.remove(0);
            items.push(header);
            items.push(
                note(
                    "Click a skill to add it to this loadout or take it out.",
                    cx,
                )
                .into_any_element(),
            );
            items.push(card(members, cx).p_1().into_any_element());
            return items;
        }
        let count = self.state.read(cx).data.loadouts.len();
        items.push(
            div()
                .flex()
                .items_center()
                .child(section_title(format!("Loadouts ({count})")))
                .child(div().flex_1())
                .child(
                    Button::new("import-file")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Plus)
                        .label("Import File…")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.import_file(cx))
                        }),
                )
                .into_any_element(),
        );
        let rows = self.render_loadouts(cx);
        if count == 0 {
            items.extend(
                rows.into_iter()
                    .map(|row| div().px_1().child(row).into_any_element()),
            );
        } else {
            items.push(card(rows, cx).p_1().into_any_element());
            items.push(
                note("Click a loadout to edit its skills. Right-click to rename, set its agent, tags, version, or share it.", cx)
                    .pt_2()
                    .into_any_element(),
            );
        }
        items
    }

    fn render_projects(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.state.read(cx);
        let global = state.data.settings.global;
        let current = state.target.root.clone();
        let now = provenance::now();
        let projects: Vec<(PathBuf, Option<u64>)> = {
            let mut projects: Vec<_> = state
                .data
                .projects
                .iter()
                .map(|p| (p.path.clone(), p.last_used))
                .collect();
            projects.sort_by_key(|(_, used)| std::cmp::Reverse(*used));
            projects
        };
        let muted = cx.theme().muted_foreground;
        let row = |ix: usize,
                   icon: Icon,
                   name: String,
                   detail: String,
                   is_current: bool,
                   path: Option<PathBuf>,
                   cx: &mut Context<Self>| {
            let state = self.state.clone();
            let hover = cx.theme().foreground.opacity(0.05);
            let target = path.clone();
            let element = div()
                .id(("project", ix))
                .h(ROW_HEIGHT)
                .px_2()
                .flex()
                .items_center()
                .gap_3()
                .rounded(px(10.))
                .hover(move |s| s.bg(hover))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| s.choose_target(target.clone(), cx))
                })
                .child(tile(false, Glyph::icon(icon), cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .truncate()
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(muted)
                                .truncate()
                                .child(detail),
                        ),
                )
                .when(is_current, |row| {
                    row.child(div().text_size(px(11.)).text_color(muted).child("Current"))
                        .child(Icon::new(IconName::Check).small().text_color(muted))
                });
            let Some(path) = path else {
                return element.into_any_element();
            };
            let state = self.state.clone();
            element
                .context_menu(move |menu, _, _| {
                    let (reveal, forget, state) = (path.clone(), path.clone(), state.clone());
                    menu.item(
                        PopupMenuItem::new("Reveal in Finder")
                            .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
                    )
                    .separator()
                    .item(PopupMenuItem::new("Forget").on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| s.forget_project(&forget, cx))
                    }))
                })
                .into_any_element()
        };
        let mut rows = vec![row(
            0,
            Icon::new(IconName::Globe),
            "Global".into(),
            "Your home folder: every project sees these skills".into(),
            global,
            None,
            cx,
        )];
        for (ix, (path, used)) in projects.into_iter().enumerate() {
            let is_current = !global && current.as_ref() == Some(&path);
            let detail = match used {
                Some(at) => format!("{} · used {}", path.display(), provenance::ago(at, now)),
                None => path.display().to_string(),
            };
            let name = folder_name(&path);
            rows.push(row(
                ix + 1,
                Icon::new(IconName::Folder),
                name,
                detail,
                is_current,
                Some(path),
                cx,
            ));
        }
        let rows: Vec<AnyElement> = rows
            .into_iter()
            .map(|row| div().px_1().child(row).into_any_element())
            .collect();
        vec![
            div()
                .flex()
                .items_center()
                .child(section_title("Install Into"))
                .child(div().flex_1())
                .child(
                    Button::new("add-project")
                        .ghost()
                        .xsmall()
                        .icon(ExtraIcon::FolderPlus)
                        .label("Add Project…")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.add_project(cx))
                        }),
                )
                .into_any_element(),
            note(
                "The palette installs into the project in front of you. Click one here (or press ⌘P in the palette) to choose it instead. Right-click to reveal or forget a project.",
                cx,
            )
            .into_any_element(),
            card(rows, cx).p_1().into_any_element(),
        ]
    }

    fn render_history(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let tab = segmented(
            "history-tab",
            &[
                (HistoryTab::History, "History"),
                (HistoryTab::Output, "Output"),
            ],
            self.history_tab,
            {
                let this = cx.entity().downgrade();
                move |tab, _, cx| {
                    this.update(cx, |this, cx| {
                        this.history_tab = tab;
                        this.scroll = ScrollHandle::new();
                        if tab == HistoryTab::Output {
                            this.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    })
                    .ok();
                }
            },
            cx,
        );
        let state = self.state.read(cx);
        let muted = cx.theme().muted_foreground;
        let mono = |text: SharedString| {
            div()
                .py(px(2.))
                .font_family("Menlo")
                .text_size(px(11.))
                .child(text)
                .into_any_element()
        };
        if self.history_tab == HistoryTab::Output {
            let lines: Vec<AnyElement> = state.log.iter().cloned().map(mono).collect();
            let empty = lines.is_empty();
            return vec![
                div().pb_2().flex().child(tab).into_any_element(),
                if empty {
                    note(
                        "Nothing yet. What `npx skills` prints during installs shows up here.",
                        cx,
                    )
                    .into_any_element()
                } else {
                    card(
                        vec![
                            div()
                                .p_3()
                                .text_color(muted)
                                .children(lines)
                                .into_any_element(),
                        ],
                        cx,
                    )
                    .into_any_element()
                },
            ];
        }
        let project = state
            .data
            .selected_project_id
            .and_then(|id| state.data.projects.iter().find(|p| p.id == id))
            .map(|p| p.path.clone());
        let skill = match self.history {
            HistoryFilter::Skill(id) => state.data.favorites.iter().find(|f| f.id == id),
            _ => None,
        };
        let entries: Vec<&ProvenanceEntry> = match (self.history, &project, skill) {
            (HistoryFilter::Skill(_), _, Some(favorite)) => {
                provenance::for_skill(&state.data.provenance, favorite)
            }
            (HistoryFilter::Project, Some(path), _) => {
                provenance::for_project(&state.data.provenance, path)
            }
            _ => provenance::recent(&state.data.provenance, usize::MAX),
        };
        let now = provenance::now();
        let lines: Vec<AnyElement> = entries
            .into_iter()
            .take(HISTORY_LINES)
            .map(|entry| mono(provenance::describe(entry, now).into()))
            .collect();
        let mut filters: Vec<(HistoryFilter, &'static str)> = vec![(HistoryFilter::All, "All")];
        if project.is_some() {
            filters.push((HistoryFilter::Project, "This project"));
        }
        let skill_label: Option<SharedString> = skill.map(|f| format!("Showing {}", f.name).into());
        if let HistoryFilter::Skill(id) = self.history {
            filters.push((HistoryFilter::Skill(id), "This skill"));
        }
        let filter = segmented(
            "history-filter",
            &filters,
            self.history,
            {
                let this = cx.entity().downgrade();
                move |filter, _, cx| {
                    this.update(cx, |this, cx| {
                        this.history = filter;
                        cx.notify();
                    })
                    .ok();
                }
            },
            cx,
        );
        let empty = lines.is_empty();
        vec![
            div()
                .pb_2()
                .flex()
                .items_center()
                .gap_3()
                .child(tab)
                .child(div().flex_1())
                .children(
                    skill_label
                        .map(|label| div().text_size(px(11.)).text_color(muted).child(label)),
                )
                .child(filter)
                .into_any_element(),
            if empty {
                note(
                    "Nothing yet. Installs, updates, removals, and imports show up here.",
                    cx,
                )
                .into_any_element()
            } else {
                card(vec![div().p_3().children(lines).into_any_element()], cx).into_any_element()
            },
        ]
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let pending = state.pending().len();
        let theme = cx.theme();
        let (dot, status): (Hsla, SharedString) = if state.installing {
            (theme.blue, "Installing…".into())
        } else if pending > 0 {
            (
                theme.blue,
                match pending {
                    1 => "1 update".into(),
                    n => format!("{n} updates").into(),
                },
            )
        } else {
            (theme.success, "Up to date".into())
        };
        let icon = app_icon_path(state.data.settings.app_icon);
        let selected = self.page;
        let groups = Page::GROUPS.iter().map(|group| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(group.iter().map(|&page| {
                    let chosen = page == selected;
                    let hover = theme.sidebar_accent.opacity(0.6);
                    div()
                        .id(SharedString::from(page.title()))
                        .h(px(30.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .rounded(px(7.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .when(chosen, |item| {
                            item.bg(theme.sidebar_accent)
                                .text_color(theme.sidebar_accent_foreground)
                                .font_weight(FontWeight::MEDIUM)
                        })
                        .when(!chosen, |item| item.hover(move |s| s.bg(hover)))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.go(page, Detail::None, cx)),
                        )
                        .child(
                            div()
                                .size(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(5.))
                                .bg(page.color(theme))
                                .child(page.icon().xsmall().text_color(theme.background)),
                        )
                        .child(page.title())
                        .when(page == Page::Skills && pending > 0, |item| {
                            item.child(div().flex_1()).child(
                                div()
                                    .px(px(6.))
                                    .rounded_full()
                                    .bg(theme.primary)
                                    .text_color(theme.primary_foreground)
                                    .text_size(px(10.))
                                    .child(pending.to_string()),
                            )
                        })
                }))
        });
        div()
            .w(SIDEBAR_WIDTH)
            .flex_none()
            .h_full()
            .pt(TITLE_BAR)
            .px(px(10.))
            .flex()
            .flex_col()
            .gap_4()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .text_color(theme.sidebar_foreground)
            .child(
                div()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(img(icon).size(px(30.)).rounded(px(7.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Loadout"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .child(div().size(px(6.)).rounded_full().bg(dot))
                                    .child(status),
                            ),
                    ),
            )
            .children(groups)
            .into_any_element()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = self.render_sidebar(cx);
        let items = match self.page {
            Page::Interface => self.render_interface(cx),
            Page::Shortcuts => self.render_shortcuts(cx),
            Page::Integrations => self.render_integrations(cx),
            Page::Notifications => self.render_notifications(cx),
            Page::Skills => self.render_skills(cx),
            Page::Loadouts => self.render_loadouts_page(cx),
            Page::Projects => self.render_projects(cx),
            Page::History => self.render_history(cx),
        };
        let state = self.state.read(cx);
        let notice = state
            .notice
            .clone()
            .or_else(|| state.install_status.clone());
        let theme = cx.theme();
        div()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(TITLE_BAR)
                            .flex_none()
                            .px_6()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.page.title()),
                            )
                            .child(div().flex_1())
                            .children(notice.map(|notice| {
                                div()
                                    .max_w(px(360.))
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(notice)
                            })),
                    )
                    .child(
                        div()
                            .id("page")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .px_6()
                            .pb_6()
                            .flex()
                            .flex_col()
                            .children(items),
                    ),
            )
    }
}
