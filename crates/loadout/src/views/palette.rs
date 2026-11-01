use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Theme};
use gpui_kit::*;

use crate::state::{AppState, Target};

actions!(palette, [Dismiss]);

const CONTEXT: &str = "Palette";
pub const WIDTH: Pixels = px(520.);
const SEARCH_HEIGHT: Pixels = px(44.);
const INTO_HEIGHT: Pixels = px(28.);
const FOOTER_HEIGHT: Pixels = px(28.);

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// Window height for the current content.
pub fn height() -> Pixels {
    SEARCH_HEIGHT + INTO_HEIGHT + FOOTER_HEIGHT + px(1.)
}

pub struct Palette {
    state: Entity<AppState>,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Palette {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search skills"));
        search.update(cx, |search, cx| search.focus(window, cx));
        let _subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
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
            state,
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
            .child(into_line(&self.state.read(cx).target, cx))
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

/// "Into: shop-dashboard        from Zed" — where Return will install.
fn into_line(target: &Target, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let name = target
        .folder
        .as_ref()
        .and_then(|folder| folder.file_name())
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
        .child(div().flex_1())
        .children(trailing.map(|text| div().min_w_0().truncate().child(text)))
}
