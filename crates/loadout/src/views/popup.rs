use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::model::Uuid;

use crate::Quit;
use crate::macos::login_item::LoginItem;
use crate::state::AppState;
use crate::windows;

actions!(popup, [Dismiss]);

const CONTEXT: &str = "Popup";
pub const WIDTH: Pixels = px(340.);
const PADDING: Pixels = px(14.);
const INPUT_HEIGHT: Pixels = px(30.);
const NOTICE_HEIGHT: Pixels = px(18.);
const ROW_HEIGHT: Pixels = px(44.);
const EMPTY_HEIGHT: Pixels = px(32.);
const FOOTER_HEIGHT: Pixels = px(28.);
const GAP: Pixels = px(8.);
const MAX_ROWS: usize = 7;

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// Window height for `favorites` rows: at most seven, then the list scrolls.
pub fn height(favorites: usize) -> Pixels {
    let list = match favorites {
        0 => EMPTY_HEIGHT,
        n => ROW_HEIGHT * n.min(MAX_ROWS) as f32,
    };
    PADDING * 2. + INPUT_HEIGHT + NOTICE_HEIGHT + list + px(1.) + FOOTER_HEIGHT + GAP * 4.
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Tags,
}

pub struct Popup {
    state: Entity<AppState>,
    paste: Entity<InputState>,
    edit: Entity<InputState>,
    editing: Option<(Uuid, Field)>,
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
            cx.observe(&state, |_, _, cx| cx.notify()),
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
            scroll: ScrollHandle::new(),
            _subscriptions,
        }
    }

    fn start_edit(&mut self, id: Uuid, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let Some(favorite) = self
            .state
            .read(cx)
            .data
            .favorites
            .iter()
            .find(|f| f.id == id)
        else {
            return;
        };
        let value = match field {
            Field::Name => favorite.name.clone(),
            Field::Tags => favorite.tags.join(", "),
        };
        self.editing = Some((id, field));
        self.edit.update(cx, |edit, cx| {
            edit.set_value(value, window, cx);
            edit.focus(window, cx);
        });
        cx.notify();
    }

    /// Esc cancels an edit before it can close the popup.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && self.editing.take().is_some() {
            self.paste.update(cx, |paste, cx| paste.focus(window, cx));
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        windows::popup_closed(cx);
        window.remove_window();
    }

    fn render_favorite(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let favorite = &self.state.read(cx).data.favorites[ix];
        let id = favorite.id;
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
        let menu_item = move |label: &'static str, field: Option<Field>| {
            let this = this.clone();
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                this.update(cx, |this, cx| match field {
                    Some(field) => this.start_edit(id, field, window, cx),
                    None => this
                        .state
                        .update(cx, |state, cx| state.remove_favorite(id, cx)),
                })
                .ok();
            })
        };
        let state = self.state.clone();
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
            .child(
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
                                state.update(cx, |state, cx| state.remove_favorite(id, cx))
                            }),
                    ),
            )
            .context_menu(move |menu, _, _| {
                menu.item(menu_item("Rename…", Some(Field::Name)))
                    .item(menu_item("Tags…", Some(Field::Tags)))
                    .separator()
                    .item(menu_item("Remove", None))
            })
            .into_any_element()
    }
}

impl Render for Popup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.state.read(cx).data.favorites.len();
        let rows: Vec<AnyElement> = (0..count).map(|ix| self.render_favorite(ix, cx)).collect();
        let theme = cx.theme();
        let state = self.state.read(cx);
        let login_on = state.login_item != LoginItem::Off;
        let notice = state.popup_notice.clone();
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
                    .id("favorites")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .when(count == 0, |list| {
                        list.text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child("Paste a GitHub or skills.sh link, owner/repo, or an npx skills add command.")
                    })
                    .children(rows),
            )
            .child(div().h(px(1.)).bg(theme.border))
            .child(
                div()
                    .h(FOOTER_HEIGHT)
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Switch::new("open-at-login")
                            .checked(login_on)
                            .label("Open at login")
                            .small()
                            .on_click({
                                let state = self.state.clone();
                                move |on, _, cx| {
                                    state.update(cx, |state, cx| state.set_open_at_login(*on, cx))
                                }
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("test-notification")
                            .ghost()
                            .xsmall()
                            .label("Test notification")
                            .on_click({
                                let state = self.state.clone();
                                move |_, _, cx| {
                                    state.update(cx, |state, cx| state.send_test_notification(cx))
                                }
                            }),
                    )
                    .child(
                        Button::new("quit")
                            .ghost()
                            .xsmall()
                            .label("Quit")
                            .on_click(|_, window, cx| window.dispatch_action(Box::new(Quit), cx)),
                    ),
            )
    }
}
