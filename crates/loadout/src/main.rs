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

fn main() {
    gpui_kit::application()
        .with_assets(assets::AppAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.set_activation_policy(ActivationPolicy::Accessory);
            Theme::sync_system_appearance(None, cx);

            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
            views::palette::init(cx);
            views::popup::init(cx);
            let state = cx.new(|_| state::AppState::new());
            cx.set_global(state::GlobalAppState(state));

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
                }
            })
            .detach();
        });
}
