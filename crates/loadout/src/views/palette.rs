use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Theme};
use gpui_kit::*;

actions!(palette, [Dismiss]);

const CONTEXT: &str = "Palette";
pub const WIDTH: Pixels = px(520.);
const SEARCH_HEIGHT: Pixels = px(44.);
const FOOTER_HEIGHT: Pixels = px(28.);

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// Window height for the current content.
pub fn height() -> Pixels {
    SEARCH_HEIGHT + FOOTER_HEIGHT + px(1.)
}

pub struct Palette {
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Palette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search skills"));
        search.update(cx, |search, cx| search.focus(window, cx));
        let _subscriptions = vec![
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
        Self {
            search,
            _subscriptions,
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }
}

impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::dismiss))
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
            .child(div().h(px(1.)).bg(theme.border))
            .child(
                div()
                    .h(FOOTER_HEIGHT)
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child("esc close"),
            )
    }
}
