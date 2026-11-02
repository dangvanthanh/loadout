mod assets;
mod hotkey;
mod macos;
mod state;
mod tray;
mod views;
mod windows;

use std::time::Duration;

use gpui_kit::component::Theme;
use gpui_kit::*;

actions!(loadout, [Quit]);

/// How often hotkey and menu bar clicks are drained; cheap and never redraws.
const HOTKEY_POLL: Duration = Duration::from_millis(50);
/// Background checks (updates, digest): shortly after launch, then whatever is due each hour.
const FIRST_CHECK: Duration = Duration::from_secs(3);
const TICK: Duration = Duration::from_secs(3600);

fn main() {
    let app = gpui_kit::application().with_assets(assets::AppAssets);
    app.on_open_urls(macos::url_scheme::push);
    app.run(|cx| {
        gpui_kit::init(cx);
        cx.set_activation_policy(ActivationPolicy::Accessory);
        Theme::sync_system_appearance(None, cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        views::palette::init(cx);
        views::popup::init(cx);
        let state = cx.new(|_| state::AppState::new());
        cx.set_global(state::GlobalAppState(state.clone()));
        cx.on_system_notification_response(|_, cx| windows::show_popup(cx));
        cx.spawn(async move |cx| {
            cx.background_executor().timer(FIRST_CHECK).await;
            loop {
                state.update(cx, |state, cx| state.tick(cx));
                cx.background_executor().timer(TICK).await;
            }
        })
        .detach();

        // Startup invariants: without the menu bar item and hotkey, Loadout is unreachable.
        let mtm = objc2::MainThreadMarker::new().expect("main thread");
        cx.set_global(tray::Tray::new(mtm).expect("create menu bar item"));
        let hotkey = hotkey::Hotkey::register(hotkey::DEFAULT).expect("register global hotkey");
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(HOTKEY_POLL).await;
                if hotkey.pressed() {
                    cx.update(windows::toggle_palette);
                }
                if tray::take_click() {
                    cx.update(windows::toggle_popup);
                }
                for text in macos::url_scheme::take() {
                    cx.update(|cx| windows::offer_import(&text, cx));
                }
            }
        })
        .detach();
    });
}
