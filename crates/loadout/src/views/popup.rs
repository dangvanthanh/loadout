use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, IconName, Sizable, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::diff::{ChangeKind, LineKind};
use loadout_core::model::{ProvenanceEntry, Uuid};
use loadout_core::{provenance, share};

use crate::Quit;
use crate::state::{AppState, ChangesState, CheckReason};
use crate::views::palette::update_pill;
use crate::views::settings::SettingsView;
use crate::windows;

actions!(popup, [Dismiss]);

const CONTEXT: &str = "Popup";
pub const WIDTH: Pixels = px(340.);
const PADDING: Pixels = px(14.);
const INPUT_HEIGHT: Pixels = px(30.);
const NOTICE_HEIGHT: Pixels = px(18.);
const ROW_HEIGHT: Pixels = px(44.);
const UPDATE_BAR_HEIGHT: Pixels = px(28.);
const TARGET_HEIGHT: Pixels = px(24.);
const FOOTER_HEIGHT: Pixels = px(28.);
const GAP: Pixels = px(8.);
const MIN_ROWS: usize = 4;
const MAX_ROWS: usize = 7;

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// Window height for `favorites` rows: four to seven, then the list scrolls (other views share
/// the list area). The update bar only when something is pending.
pub fn height(favorites: usize, updates: bool) -> Pixels {
    let list = ROW_HEIGHT * favorites.clamp(MIN_ROWS, MAX_ROWS) as f32;
    let bar = if updates {
        UPDATE_BAR_HEIGHT + GAP
    } else {
        px(0.)
    };
    PADDING * 2.
        + TARGET_HEIGHT
        + INPUT_HEIGHT
        + NOTICE_HEIGHT
        + list
        + bar
        + px(1.)
        + FOOTER_HEIGHT
        + GAP * 5.
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Tags,
    LoadoutName,
    LoadoutTags,
    LoadoutAgent,
    LoadoutVersion,
    LoadoutSource,
}

/// What the list area shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Favorites,
    /// Output of `skills` runs.
    Output,
    /// A pending update's files and SKILL.md diff.
    Changes(Uuid),
    /// Provenance, newest first.
    History(HistoryFilter),
    Loadouts,
    /// One loadout's skills, toggled by clicking.
    Loadout(Uuid),
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HistoryFilter {
    All,
    /// The project last detected.
    Project,
    Skill(Uuid),
}

/// Most history lines shown.
const HISTORY_LINES: usize = 200;

pub struct Popup {
    state: Entity<AppState>,
    paste: Entity<InputState>,
    edit: Entity<InputState>,
    editing: Option<(Uuid, Field)>,
    view: View,
    /// Made the first time Settings opens.
    settings: Option<Entity<SettingsView>>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Popup {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let paste =
            cx.new(|cx| InputState::new(window, cx).placeholder("Paste a link or owner/repo"));
        let edit = cx.new(|cx| InputState::new(window, cx));
        paste.update(cx, |paste, cx| paste.focus(window, cx));
        let _subscriptions = vec![
            cx.observe(&state, |this, state, cx| {
                // An update that's no longer pending (updated, pinned, ignored) closes its view.
                if let View::Changes(id) = this.view
                    && !state.read(cx).pending().iter().any(|p| p.favorite == id)
                {
                    this.view = View::Favorites;
                }
                if let View::Loadout(id) = this.view
                    && state.read(cx).data.loadout(id).is_none()
                {
                    this.view = View::Loadouts;
                }
                cx.notify();
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
                        this.set_view(View::Favorites, cx);
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
                        this.paste.update(cx, |paste, cx| paste.focus(window, cx));
                        cx.notify();
                    }
                    InputEvent::Blur => {
                        this.editing = None;
                        cx.notify();
                    }
                    _ => {}
                },
            ),
            cx.observe_window_appearance(window, |_, window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
            cx.observe_window_activation(window, |_, window, cx| {
                if !window.is_window_active() {
                    windows::popup_blurred(cx);
                    window.remove_window();
                }
            }),
        ];
        Self {
            state,
            paste,
            edit,
            editing: None,
            view: View::Favorites,
            settings: None,
            scroll: ScrollHandle::new(),
            _subscriptions,
        }
    }

    fn start_edit(&mut self, id: Uuid, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let data = &self.state.read(cx).data;
        let favorite = data.favorites.iter().find(|f| f.id == id);
        let loadout = data.loadout(id);
        let value = match (field, favorite, loadout) {
            (Field::Name, Some(f), _) => f.name.clone(),
            (Field::Tags, Some(f), _) => f.tags.join(", "),
            (Field::LoadoutName, _, Some(l)) => l.name.clone(),
            (Field::LoadoutTags, _, Some(l)) => l.tags.join(", "),
            (Field::LoadoutAgent, _, Some(l)) => l.preferred_agent.clone().unwrap_or_default(),
            (Field::LoadoutVersion, _, Some(l)) => l.version.clone().unwrap_or_default(),
            (Field::LoadoutSource, _, Some(l)) => l.source.clone().unwrap_or_default(),
            _ => return,
        };
        self.editing = Some((id, field));
        self.edit.update(cx, |edit, cx| {
            edit.set_value(value, window, cx);
            edit.focus(window, cx);
        });
        cx.notify();
    }

    fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        if let View::Changes(id) = view {
            self.state
                .update(cx, |state, cx| state.show_changes(id, cx));
        }
        self.view = view;
        self.scroll = ScrollHandle::new();
        if view == View::Output {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    /// Esc cancels an edit, then leaves a secondary view, before it can close the popup.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" {
            return;
        }
        if self.editing.take().is_some() {
            self.paste.update(cx, |paste, cx| paste.focus(window, cx));
        } else if self.state.read(cx).pending_import.is_some() {
            self.state.update(cx, |state, cx| state.cancel_import(cx));
        } else if self.view != View::Favorites {
            self.set_view(View::Favorites, cx);
        } else {
            return;
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        windows::popup_closed(cx);
        window.remove_window();
    }

    fn render_favorite(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let favorite = &state.data.favorites[ix];
        let id = favorite.id;
        let has_update = state.pending().iter().any(|p| p.favorite == id);
        let pinned = state.data.skill_state(id).pinned;
        let editing = self
            .editing
            .filter(|(editing, _)| *editing == id)
            .map(|(_, f)| f);
        let mut subtitle = favorite.source.clone();
        if favorite.skill.is_none() {
            subtitle += " · all skills";
        }
        if !favorite.tags.is_empty() {
            subtitle = format!("{subtitle} · {}", favorite.tags.join(", "));
        }
        let hover = theme.list_hover;
        let this = cx.entity().downgrade();
        let menu_item = move |label: &'static str, action: RowAction| {
            let this = this.clone();
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                this.update(cx, |this, cx| this.row_action(id, action, window, cx))
                    .ok();
            })
        };
        let remove = self.state.clone();
        div()
            .id(("favorite", ix))
            .group("favorite")
            .h(ROW_HEIGHT)
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(7.))
            .hover(move |s| s.bg(hover))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(match editing {
                        Some(Field::Name) => Input::new(&self.edit).small().into_any_element(),
                        _ => div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(favorite.name.clone())
                            .into_any_element(),
                    })
                    .child(match editing {
                        Some(Field::Tags) => Input::new(&self.edit).small().into_any_element(),
                        _ => div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(subtitle)
                            .into_any_element(),
                    }),
            )
            .when(pinned, |row| {
                row.child(
                    Icon::new(ExtraIcon::Pin)
                        .xsmall()
                        .text_color(theme.muted_foreground),
                )
            })
            .when(has_update, |row| {
                row.child(
                    div()
                        .id(("update", ix))
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.set_view(View::Changes(id), cx)),
                        )
                        .child(update_pill(cx)),
                )
            })
            .when(!has_update, |row| {
                row.child(
                    div()
                        .opacity(0.)
                        .group_hover("favorite", |s| s.opacity(1.))
                        .child(
                            Button::new(("remove", ix))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .tooltip("Remove")
                                .on_click(move |_, _, cx| {
                                    remove.update(cx, |state, cx| state.remove_favorite(id, cx))
                                }),
                        ),
                )
            })
            .context_menu(move |menu, _, _| {
                menu.when(has_update, |menu| {
                    menu.item(menu_item("What changed", RowAction::Changes))
                })
                .item(menu_item(
                    if pinned { "Unpin" } else { "Pin" },
                    RowAction::Pin(!pinned),
                ))
                .item(menu_item("History", RowAction::History))
                .separator()
                .item(menu_item("Rename…", RowAction::Edit(Field::Name)))
                .item(menu_item("Tags…", RowAction::Edit(Field::Tags)))
                .separator()
                .item(menu_item("Remove", RowAction::Remove))
            })
            .into_any_element()
    }

    fn row_action(
        &mut self,
        id: Uuid,
        action: RowAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            RowAction::Changes => self.set_view(View::Changes(id), cx),
            RowAction::History => self.set_view(View::History(HistoryFilter::Skill(id)), cx),
            RowAction::Pin(pinned) => self
                .state
                .update(cx, |state, cx| state.set_pinned(id, pinned, cx)),
            RowAction::Edit(field) => self.start_edit(id, field, window, cx),
            RowAction::Remove => self
                .state
                .update(cx, |state, cx| state.remove_favorite(id, cx)),
        }
    }

    /// Header, places, actions, file list, and SKILL.md diff for one pending update.
    fn render_changes(&self, id: Uuid, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let name = state
            .data
            .favorites
            .iter()
            .find(|f| f.id == id)
            .map_or_else(String::new, |f| f.name.clone());
        let busy = state.installing;
        let Some(shown) = state.what_changed.as_ref().filter(|w| w.favorite == id) else {
            return Vec::new();
        };
        let muted = theme.muted_foreground;
        let mono = |text: SharedString, color: Hsla| {
            div()
                .font_family("Menlo")
                .text_size(px(10.))
                .text_color(color)
                .whitespace_nowrap()
                .child(text)
                .into_any_element()
        };
        let mut items = vec![
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Button::new("back")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ArrowLeft)
                        .tooltip("Back")
                        .on_click(cx.listener(|this, _, _, cx| this.set_view(View::Favorites, cx))),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(name),
                )
                .into_any_element(),
            div()
                .text_size(px(11.))
                .text_color(muted)
                .child(format!("Behind in {}", shown.places.join(", ")))
                .into_any_element(),
            div()
                .py_1()
                .flex()
                .gap_1()
                .child(
                    Button::new("update-one")
                        .primary()
                        .xsmall()
                        .label("Update")
                        .disabled(busy)
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.update(Some(id), cx))
                        }),
                )
                .child(
                    Button::new("ignore")
                        .ghost()
                        .xsmall()
                        .label("Ignore this update")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.ignore_update(id, cx))
                        }),
                )
                .child(
                    Button::new("pin")
                        .ghost()
                        .xsmall()
                        .label("Pin")
                        .tooltip("Stay on the installed version")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.set_pinned(id, true, cx))
                        }),
                )
                .into_any_element(),
        ];
        match &shown.changes {
            ChangesState::Loading => items.push(mono("Loading changes…".into(), muted)),
            ChangesState::Unavailable(why) => items.push(
                div()
                    .text_size(px(11.))
                    .text_color(muted)
                    .child(why.clone())
                    .into_any_element(),
            ),
            ChangesState::Ready(changes) => {
                items.push(
                    div()
                        .pt_1()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .child(match changes.files.len() {
                            1 => "1 file changed".to_string(),
                            n => format!("{n} files changed"),
                        })
                        .into_any_element(),
                );
                for file in &changes.files {
                    let (mark, color) = match file.kind {
                        ChangeKind::Added => ("+", theme.success),
                        ChangeKind::Removed => ("−", theme.danger),
                        ChangeKind::Modified => ("~", muted),
                    };
                    items.push(mono(format!("{mark} {}", file.path).into(), color));
                }
                if !changes.skill_md.is_empty() {
                    items.push(
                        div()
                            .pt_2()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(muted)
                            .child("SKILL.md")
                            .into_any_element(),
                    );
                }
                for line in &changes.skill_md {
                    let (mark, color) = match line.kind {
                        LineKind::Added => ("+", theme.success),
                        LineKind::Removed => ("-", theme.danger),
                        LineKind::Context => (" ", muted),
                        LineKind::Gap => ("…", muted),
                    };
                    items.push(mono(format!("{mark} {}", line.text).into(), color));
                }
            }
        }
        items
    }
}

impl Popup {
    /// "Into: shop-dashboard ⌄": recent projects, Global, Add project…, Forget.
    fn render_target(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let global = state.data.settings.global;
        let current = (!global)
            .then(|| {
                state
                    .target
                    .root
                    .clone()
                    .or_else(|| state.recent_projects().into_iter().next())
            })
            .flatten();
        let label: SharedString = match (&current, global) {
            (_, true) => "Into: Global".into(),
            (Some(path), false) => format!("Into: {}", folder_name(path)).into(),
            (None, false) => "Into: no project yet".into(),
        };
        let projects: Vec<std::path::PathBuf> =
            state.recent_projects().into_iter().take(10).collect();
        let entity = self.state.clone();
        div()
            .h(TARGET_HEIGHT)
            .flex()
            .items_center()
            .child(
                Button::new("target")
                    .ghost()
                    .xsmall()
                    .label(label)
                    .icon(IconName::ChevronDown)
                    .tooltip("Where installs go")
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        for path in &projects {
                            let (state, path) = (entity.clone(), path.clone());
                            let checked = !global && current.as_ref() == Some(&path);
                            menu = menu.item(
                                PopupMenuItem::new(folder_name(&path))
                                    .checked(checked)
                                    .on_click(move |_, _, cx| {
                                        state.update(cx, |s, cx| {
                                            s.choose_target(Some(path.clone()), cx)
                                        })
                                    }),
                            );
                        }
                        let (to_global, adder, forget) =
                            (entity.clone(), entity.clone(), entity.clone());
                        let current = current.clone();
                        menu.separator()
                            .item(
                                PopupMenuItem::new("Global (home folder)")
                                    .checked(global)
                                    .on_click(move |_, _, cx| {
                                        to_global.update(cx, |s, cx| s.choose_target(None, cx))
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Add project…").on_click(move |_, _, cx| {
                                    adder.update(cx, |s, cx| s.add_project(cx))
                                }),
                            )
                            .when_some(current, |menu, path| {
                                menu.item(
                                    PopupMenuItem::new(format!("Forget {}", folder_name(&path)))
                                        .on_click(move |_, _, cx| {
                                            forget.update(cx, |s, cx| s.forget_project(&path, cx))
                                        }),
                                )
                            })
                    }),
            )
            .into_any_element()
    }

    /// A back button and a title, for secondary views.
    fn view_header(&self, title: SharedString, back: View, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                Button::new("back")
                    .ghost()
                    .xsmall()
                    .icon(IconName::ArrowLeft)
                    .tooltip("Back")
                    .on_click(cx.listener(move |this, _, _, cx| this.set_view(back, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .into_any_element()
    }

    /// Every loadout: click for its skills, right-click to rename, set the agent or tags, remove.
    fn render_loadouts(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items = vec![self.view_header("Loadouts".into(), View::Favorites, cx)];
        let state = self.state.read(cx);
        let theme = cx.theme();
        if state.data.loadouts.is_empty() {
            items.push(
                div()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child("No loadouts yet. Select skills in the palette (⌃⌥L) and press ⌘S to save them as one.")
                    .into_any_element(),
            );
            return items;
        }
        let rows: Vec<(Uuid, String, String, Vec<String>, bool)> = state
            .data
            .loadouts
            .iter()
            .map(|l| {
                let mut subtitle = state.loadout_subtitle(l.id);
                if let Some(version) = &l.version {
                    subtitle += &format!(" · version {version}");
                }
                (
                    l.id,
                    l.name.clone(),
                    subtitle,
                    l.tags.clone(),
                    l.source.is_some(),
                )
            })
            .collect();
        for (ix, (id, name, subtitle, tags, has_source)) in rows.into_iter().enumerate() {
            let editing = self
                .editing
                .filter(|(editing, _)| *editing == id)
                .map(|(_, f)| f);
            let subtitle = if tags.is_empty() {
                subtitle
            } else {
                format!("{subtitle} · {}", tags.join(", "))
            };
            let theme = cx.theme();
            let hover = theme.list_hover;
            let this = cx.entity().downgrade();
            let menu_item = move |label: &'static str, action: LoadoutAction| {
                let this = this.clone();
                PopupMenuItem::new(label).on_click(move |_, window, cx| {
                    this.update(cx, |this, cx| match action {
                        LoadoutAction::Edit(field) => this.start_edit(id, field, window, cx),
                        LoadoutAction::CopyLink => this
                            .state
                            .update(cx, |state, cx| state.copy_loadout_link(id, cx)),
                        LoadoutAction::SaveFile => this
                            .state
                            .update(cx, |state, cx| state.save_loadout_file(id, cx)),
                        LoadoutAction::CheckSource => this
                            .state
                            .update(cx, |state, cx| state.check_loadout_source(id, cx)),
                        LoadoutAction::Remove => this
                            .state
                            .update(cx, |state, cx| state.remove_loadout(id, cx)),
                    })
                    .ok();
                })
            };
            items.push(
                div()
                    .id(("loadout", ix))
                    .h(ROW_HEIGHT)
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(7.))
                    .hover(move |s| s.bg(hover))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.editing.is_none() {
                            this.set_view(View::Loadout(id), cx)
                        }
                    }))
                    .child(
                        Icon::new(ExtraIcon::Layers)
                            .small()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(match editing {
                                Some(Field::LoadoutName) => {
                                    Input::new(&self.edit).small().into_any_element()
                                }
                                _ => div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(name)
                                    .into_any_element(),
                            })
                            .child(match editing {
                                Some(
                                    Field::LoadoutTags
                                    | Field::LoadoutAgent
                                    | Field::LoadoutVersion
                                    | Field::LoadoutSource,
                                ) => Input::new(&self.edit).small().into_any_element(),
                                _ => div()
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(subtitle)
                                    .into_any_element(),
                            }),
                    )
                    .context_menu(move |menu, _, _| {
                        menu.item(menu_item(
                            "Rename…",
                            LoadoutAction::Edit(Field::LoadoutName),
                        ))
                        .item(menu_item(
                            "Preferred agent…",
                            LoadoutAction::Edit(Field::LoadoutAgent),
                        ))
                        .item(menu_item("Tags…", LoadoutAction::Edit(Field::LoadoutTags)))
                        .separator()
                        .item(menu_item(
                            "Version…",
                            LoadoutAction::Edit(Field::LoadoutVersion),
                        ))
                        .item(menu_item(
                            "Source URL…",
                            LoadoutAction::Edit(Field::LoadoutSource),
                        ))
                        .when(has_source, |menu| {
                            menu.item(menu_item(
                                "Check for a newer version",
                                LoadoutAction::CheckSource,
                            ))
                        })
                        .separator()
                        .item(menu_item("Copy link", LoadoutAction::CopyLink))
                        .item(menu_item("Save file…", LoadoutAction::SaveFile))
                        .separator()
                        .item(menu_item("Remove", LoadoutAction::Remove))
                    })
                    .into_any_element(),
            );
        }
        items
    }

    /// What a link or file would add, before anything changes.
    fn render_import(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.state.read(cx);
        let Some(shared) = state.pending_import.as_ref() else {
            return Vec::new();
        };
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        // What importing does to your same-named loadout, if any: (sentence, warn).
        let yours = state
            .data
            .loadouts
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(&shared.name));
        let change: Option<(String, bool)> = match share::version_change(&state.data, shared) {
            share::VersionChange::New => None,
            share::VersionChange::Newer => yours.map(|l| {
                let from = l
                    .version
                    .as_ref()
                    .map_or_else(String::new, |v| format!(" from version {v}"));
                (format!("Updates your {}{from}.", l.name), false)
            }),
            share::VersionChange::Same => Some(("You already have this version.".into(), false)),
            share::VersionChange::Older { yours } => Some((
                format!("Older than yours (version {yours}); importing goes back to it."),
                true,
            )),
            share::VersionChange::Unversioned => Some((
                format!(
                    "Replaces the skills and agent of your loadout {}.",
                    shared.name
                ),
                true,
            )),
        };
        let mut summary = format!(
            "{} skill{}",
            shared.skills.len(),
            if shared.skills.len() == 1 { "" } else { "s" }
        );
        if let Some(agent) = &shared.preferred_agent {
            summary += &format!(" · prefers {}", loadout_core::launch::agent_name(agent));
        }
        if let Some(version) = &shared.version {
            summary += &format!(" · version {version}");
        }
        let skills: Vec<(String, String, bool)> = shared
            .skills
            .iter()
            .map(|skill| {
                let have = state.data.favorites.iter().any(|f| {
                    f.source.eq_ignore_ascii_case(&skill.source)
                        && f.skill.as_deref().unwrap_or("") == skill.skill.as_deref().unwrap_or("")
                });
                (skill.name.clone(), skill.source.clone(), have)
            })
            .collect();
        let mut items = vec![
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .child(format!("Import {}?", shared.name))
                .into_any_element(),
            div()
                .text_size(px(11.))
                .text_color(muted)
                .child(summary)
                .into_any_element(),
        ];
        if let Some((text, warn)) = change {
            items.push(
                div()
                    .text_size(px(11.))
                    .text_color(if warn { theme.warning } else { muted })
                    .child(text)
                    .into_any_element(),
            );
        }
        items.push(
            div()
                .py_1()
                .flex()
                .gap_1()
                .child(
                    Button::new("import")
                        .primary()
                        .xsmall()
                        .label("Import")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.confirm_import(cx))
                        }),
                )
                .child(
                    Button::new("cancel-import")
                        .ghost()
                        .xsmall()
                        .label("Cancel")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |s, cx| s.cancel_import(cx))
                        }),
                )
                .into_any_element(),
        );
        for (name, source, have) in skills {
            items.push(
                div()
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_size(px(12.)).truncate().child(name))
                    .child(div().flex_1())
                    .child(
                        div()
                            .max_w(px(170.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(muted)
                            .child(if have {
                                format!("{source} · saved")
                            } else {
                                source
                            }),
                    )
                    .into_any_element(),
            );
        }
        items.push(
            div()
                .pt_1()
                .text_size(px(11.))
                .text_color(muted)
                .child("Nothing is installed now. Apply it from the palette (⌃⌥L).")
                .into_any_element(),
        );
        items
    }

    /// Every favorite with a check for membership; clicking toggles it.
    fn render_loadout_members(&self, id: Uuid, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.state.read(cx);
        let Some(loadout) = state.data.loadout(id) else {
            return Vec::new();
        };
        let title: SharedString = loadout.name.clone().into();
        let members = loadout.favorite_ids.clone();
        let favorites: Vec<(Uuid, String, String)> = state
            .data
            .favorites
            .iter()
            .map(|f| (f.id, f.name.clone(), f.source.clone()))
            .collect();
        let mut items = vec![self.view_header(title, View::Loadouts, cx)];
        for (ix, (favorite, name, source)) in favorites.into_iter().enumerate() {
            let theme = cx.theme();
            let hover = theme.list_hover;
            let icon = if members.contains(&favorite) {
                Icon::new(IconName::CircleCheck).text_color(theme.primary)
            } else {
                Icon::new(ExtraIcon::Circle).text_color(theme.muted_foreground)
            };
            let state = self.state.clone();
            items.push(
                div()
                    .id(("member", ix))
                    .h(px(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(7.))
                    .hover(move |s| s.bg(hover))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| s.toggle_loadout_favorite(id, favorite, cx))
                    })
                    .child(icon.small())
                    .child(div().text_size(px(13.)).truncate().child(name))
                    .child(div().flex_1())
                    .child(
                        div()
                            .max_w(px(140.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child(source),
                    )
                    .into_any_element(),
            );
        }
        items
    }

    /// Back, a title, the project toggle, and one line per entry.
    fn render_history(&self, filter: HistoryFilter, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let project = state
            .data
            .selected_project_id
            .and_then(|id| state.data.projects.iter().find(|p| p.id == id))
            .map(|p| p.path.clone());
        let skill = match filter {
            HistoryFilter::Skill(id) => state.data.favorites.iter().find(|f| f.id == id),
            _ => None,
        };
        let entries: Vec<&ProvenanceEntry> = match (filter, &project, skill) {
            (HistoryFilter::Skill(_), _, Some(favorite)) => {
                provenance::for_skill(&state.data.provenance, favorite)
            }
            (HistoryFilter::Project, Some(path), _) => {
                provenance::for_project(&state.data.provenance, path)
            }
            _ => provenance::recent(&state.data.provenance, usize::MAX),
        };
        let title: SharedString = match (filter, &project, skill) {
            (HistoryFilter::Skill(_), _, Some(favorite)) => {
                format!("History · {}", favorite.name).into()
            }
            (HistoryFilter::Project, Some(path), _) => format!(
                "History · {}",
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            )
            .into(),
            _ => "History".into(),
        };
        let toggle = match (filter, project.is_some()) {
            (HistoryFilter::All, true) => Some(("This project", HistoryFilter::Project)),
            (HistoryFilter::Project, _) => Some(("All projects", HistoryFilter::All)),
            _ => None,
        };
        let now = provenance::now();
        let mut items = vec![
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Button::new("back")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ArrowLeft)
                        .tooltip("Back")
                        .on_click(cx.listener(|this, _, _, cx| this.set_view(View::Favorites, cx))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .when_some(toggle, |row, (label, next)| {
                    row.child(
                        Button::new("history-filter")
                            .ghost()
                            .xsmall()
                            .label(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_view(View::History(next), cx)
                            })),
                    )
                })
                .into_any_element(),
        ];
        if entries.is_empty() {
            items.push(
                div()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child("Nothing yet. Installs and updates show up here.")
                    .into_any_element(),
            );
        }
        items.extend(entries.into_iter().take(HISTORY_LINES).map(|entry| {
            div()
                .py(px(1.))
                .font_family("Menlo")
                .text_size(px(10.))
                .child(provenance::describe(entry, now))
                .into_any_element()
        }));
        items
    }
}

#[derive(Clone, Copy)]
enum LoadoutAction {
    Edit(Field),
    CheckSource,
    CopyLink,
    SaveFile,
    Remove,
}

#[derive(Clone, Copy)]
enum RowAction {
    Changes,
    History,
    Pin(bool),
    Edit(Field),
    Remove,
}

impl Render for Popup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.view == View::Settings && self.settings.is_none() {
            let state = self.state.clone();
            self.settings = Some(cx.new(|cx| SettingsView::new(state, window, cx)));
        }
        let count = self.state.read(cx).data.favorites.len();
        let target = self.render_target(cx);
        let importing = self.state.read(cx).pending_import.is_some();
        let items: Vec<AnyElement> = if importing {
            self.render_import(cx)
        } else {
            match self.view {
                View::Favorites => (0..count).map(|ix| self.render_favorite(ix, cx)).collect(),
                View::Output => Vec::new(),
                View::Changes(id) => self.render_changes(id, cx),
                View::History(filter) => self.render_history(filter, cx),
                View::Loadouts => self.render_loadouts(cx),
                View::Loadout(id) => self.render_loadout_members(id, cx),
                View::Settings => {
                    let mut items = vec![self.view_header("Settings".into(), View::Favorites, cx)];
                    items.extend(self.settings.clone().map(|view| view.into_any_element()));
                    items
                }
            }
        };
        let theme = cx.theme();
        let state = self.state.read(cx);
        let checking = state.checking;
        let check_status = state.check_status();
        let pending = state.pending().len();
        let busy = state.installing;
        let notice = state
            .popup_notice
            .clone()
            .or_else(|| state.install_status.clone());
        let has_log = !state.log.is_empty();
        let has_history = !state.data.provenance.is_empty();
        let log: Vec<SharedString> = if self.view == View::Output {
            state.log.clone()
        } else {
            Vec::new()
        };
        div()
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::dismiss))
            .capture_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .p(PADDING)
            .flex()
            .flex_col()
            .gap(GAP)
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(px(13.))
            .child(target)
            .child(
                div()
                    .h(INPUT_HEIGHT)
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(px(7.))
                    .bg(theme.muted)
                    .child(
                        Input::new(&self.paste)
                            .appearance(false)
                            .small()
                            .prefix(Icon::new(IconName::Plus).text_color(theme.muted_foreground)),
                    ),
            )
            .child(
                div()
                    .h(NOTICE_HEIGHT)
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .children(notice),
            )
            .child(
                div()
                    .id("list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .when(self.view == View::Output, |list| {
                        list.font_family("Menlo")
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .children(log.into_iter().map(|line| div().child(line)))
                    })
                    .when(!importing && self.view == View::Favorites && count == 0, |list| {
                        list.text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child("Paste a GitHub or skills.sh link, owner/repo, or an npx skills add command.")
                    })
                    .children(items),
            )
            .when(!importing && pending > 0 && self.view == View::Favorites, |popup| {
                popup.child(
                    div()
                        .h(UPDATE_BAR_HEIGHT)
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(12.))
                        .child(div().size(px(6.)).rounded_full().bg(theme.primary))
                        .child(match pending {
                            1 => "1 update available".to_string(),
                            n => format!("{n} updates available"),
                        })
                        .child(div().flex_1())
                        .child(
                            Button::new("update-all")
                                .ghost()
                                .xsmall()
                                .label("Update all")
                                .disabled(busy)
                                .on_click({
                                    let state = self.state.clone();
                                    move |_, _, cx| state.update(cx, |state, cx| state.update(None, cx))
                                }),
                        ),
                )
            })
            .child(div().h(px(1.)).bg(theme.border))
            .child(
                div()
                    .h(FOOTER_HEIGHT)
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .children(check_status),
                    )
                    .child(
                        Button::new("check")
                            .ghost()
                            .xsmall()
                            .icon(IconName::RotateCw)
                            .tooltip("Check for updates")
                            .disabled(checking)
                            .on_click({
                                let state = self.state.clone();
                                move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.check_for_updates(CheckReason::Manual, cx)
                                    })
                                }
                            }),
                    )
                    .child(
                        Button::new("settings")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Settings)
                            .tooltip("More")
                            .dropdown_menu_with_anchor(Anchor::BottomRight, {
                                let state = self.state.clone();
                                let this = cx.entity().downgrade();
                                move |menu, _, _| {
                                    let item = |label: &'static str| PopupMenuItem::new(label);
                                    let view = |label: &'static str, view: View| {
                                        let this = this.clone();
                                        item(label).on_click(move |_, _, cx| {
                                            this.update(cx, |this, cx| this.set_view(view, cx)).ok();
                                        })
                                    };
                                    let importer = state.clone();
                                    menu.item(view("Loadouts", View::Loadouts))
                                        .item(item("Import loadout file…").on_click(move |_, _, cx| {
                                            importer.update(cx, |s, cx| s.import_file(cx))
                                        }))
                                        .when(has_history, |menu| {
                                            menu.item(view("History", View::History(HistoryFilter::All)))
                                        })
                                        .when(has_log, |menu| menu.item(view("Output", View::Output)))
                                        .separator()
                                        .item(view("Settings…", View::Settings))
                                        .item(item("Quit Loadout").on_click(|_, window, cx| {
                                            window.dispatch_action(Box::new(Quit), cx)
                                        }))
                                }
                            }),
                    ),
            )
    }
}

fn folder_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
