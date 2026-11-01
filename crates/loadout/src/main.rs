use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::*;

struct Hello {
    input: Entity<InputState>,
    _appearance: Subscription,
}

impl Hello {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search skills"));
        let _appearance = cx.observe_window_appearance(window, |_, window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        });
        Self { input, _appearance }
    }
}

impl Render for Hello {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(Input::new(&self.input))
            .child(Button::new("install").primary().label("Install"))
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        Theme::sync_system_appearance(None, cx);
        let bounds = Bounds::centered(None, size(px(520.), px(360.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        // Startup invariant: without a window there is nothing to run.
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Hello::new(window, cx))
        })
        .expect("failed to open window");
        cx.activate(true);
    });
}
