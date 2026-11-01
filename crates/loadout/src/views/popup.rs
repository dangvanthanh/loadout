use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable, Theme};
use gpui_kit::*;

use crate::Quit;
use crate::macos::login_item::LoginItem;
use crate::state::AppState;
use crate::windows;

actions!(popup, [Dismiss]);

const CONTEXT: &str = "Popup";
pub const WIDTH: Pixels = px(340.);
const PADDING: Pixels = px(14.);
const ROW_HEIGHT: Pixels = px(28.);
const GAP: Pixels = px(8.);

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// Window height for the current content: three rows plus the notice line.
pub fn height() -> Pixels {
    PADDING * 2. + ROW_HEIGHT * 4. + GAP * 3.
}

pub struct Popup {
    state: Entity<AppState>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Popup {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let _subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
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
            focus,
            _subscriptions,
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }
}

impl Render for Popup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let login_on = state.login_item != LoginItem::Off;
        let notice = state.popup_notice.clone();
        let row = || div().h(ROW_HEIGHT).flex().items_center();
        div()
            .track_focus(&self.focus)
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::dismiss))
            .size_full()
            .p(PADDING)
            .flex()
            .flex_col()
            .gap(GAP)
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(px(13.))
            .child(
                row().child(
                    Switch::new("open-at-login")
                        .checked(login_on)
                        .label("Open at login")
                        .on_click({
                            let state = self.state.clone();
                            move |on, _, cx| {
                                state.update(cx, |state, cx| state.set_open_at_login(*on, cx))
                            }
                        }),
                ),
            )
            .child(
                row().child(
                    Button::new("test-notification")
                        .ghost()
                        .small()
                        .label("Send test notification")
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| {
                                state.update(cx, |state, cx| state.send_test_notification(cx))
                            }
                        }),
                ),
            )
            .child(
                row()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .children(notice.map(|notice| div().truncate().child(notice))),
            )
            .child(
                row().justify_end().child(
                    Button::new("quit")
                        .ghost()
                        .small()
                        .label("Quit Loadout")
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(Quit), cx)),
                ),
            )
    }
}
