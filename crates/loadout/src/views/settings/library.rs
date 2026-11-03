//! Settings pages that manage things: skills (and what changed), loadouts (and imports).
//! Moved from the old menu bar popup.

use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, IconName, Sizable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::diff::{ChangeKind, LineKind};
use loadout_core::model::Uuid;
use loadout_core::share;

use super::{Detail, HistoryFilter, Page, SettingsWindow};
use crate::state::ChangesState;
use crate::views::palette::{Glyph, tile, update_pill};

pub(super) const ROW_HEIGHT: Pixels = px(44.);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Name,
    Tags,
    LoadoutName,
    LoadoutTags,
    LoadoutAgent,
    LoadoutVersion,
    LoadoutSource,
}

impl SettingsWindow {
    pub(super) fn start_edit(
        &mut self,
        id: Uuid,
        field: Field,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub(super) fn render_favorite(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let favorite = &state.data.favorites[ix];
        let id = favorite.id;
        let has_update = state.pending().iter().any(|p| p.favorite == id);
        let pinned = state.data.skill_state(id).pinned;
        let installed_here = state.is_installed_here(id);
        let editing = self
            .editing
            .filter(|(editing, _)| *editing == id)
            .map(|(_, f)| f);
        let source = favorite.source.clone();
        let mut subtitle = favorite.source.clone();
        if favorite.skill.is_none() {
            subtitle += " · all skills";
        }
        if !favorite.tags.is_empty() {
            subtitle = format!("{subtitle} · {}", favorite.tags.join(", "));
        }
        let hover = theme.foreground.opacity(0.05);
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
            .rounded(px(10.))
            .hover(move |s| s.bg(hover))
            .child(tile(false, Glyph::for_source(&source), cx))
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
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.go(Page::Skills, Detail::Changes(id), cx)
                        }))
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
                                .tooltip("Remove favorite")
                                .accessibility_label("Remove favorite")
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
                .item(menu_item("Copy install command", RowAction::CopyCommand))
                .when(installed_here, |menu| {
                    menu.item(menu_item(
                        "Uninstall from this project",
                        RowAction::Uninstall,
                    ))
                })
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
            RowAction::Changes => self.go(Page::Skills, Detail::Changes(id), cx),
            RowAction::History => {
                self.history = HistoryFilter::Skill(id);
                self.go(Page::History, Detail::None, cx)
            }
            RowAction::CopyCommand => self
                .state
                .update(cx, |state, cx| state.copy_install_command(id, cx)),
            RowAction::Uninstall => self.state.update(cx, |state, cx| state.uninstall(id, cx)),
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
    pub(super) fn render_changes(&self, id: Uuid, cx: &mut Context<Self>) -> Vec<AnyElement> {
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
                        .accessibility_label("Back")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.go(Page::Skills, Detail::None, cx)),
                        ),
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

impl SettingsWindow {
    pub(super) fn view_header(
        &self,
        title: SharedString,
        back: Page,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
                    .accessibility_label("Back")
                    .on_click(cx.listener(move |this, _, _, cx| this.go(back, Detail::None, cx))),
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
    pub(super) fn render_loadouts(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items = Vec::new();
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
            let hover = theme.foreground.opacity(0.05);
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
                    .rounded(px(10.))
                    .hover(move |s| s.bg(hover))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.editing.is_none() {
                            this.go(Page::Loadouts, Detail::Loadout(id), cx)
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
    pub(super) fn render_import(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
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
                            .max_w(px(210.))
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
    pub(super) fn render_loadout_members(
        &self,
        id: Uuid,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
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
        let mut items = vec![self.view_header(title, Page::Loadouts, cx)];
        for (ix, (favorite, name, source)) in favorites.into_iter().enumerate() {
            let theme = cx.theme();
            let hover = theme.foreground.opacity(0.05);
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
                    .rounded(px(10.))
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
                            .max_w(px(180.))
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
    CopyCommand,
    Uninstall,
    Pin(bool),
    Edit(Field),
    Remove,
}

pub(super) fn folder_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
