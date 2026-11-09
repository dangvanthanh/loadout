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

actions!(loadout, [Quit, OpenSettings]);

/// How often hotkey and menu bar clicks are drained; cheap and never redraws.
const HOTKEY_POLL: Duration = Duration::from_millis(50);
/// Background checks (updates, digest): shortly after launch, then whatever is due each hour.
const FIRST_CHECK: Duration = Duration::from_secs(3);
const TICK: Duration = Duration::from_secs(3600);

fn main() {
    let app = gpui_kit::application().with_assets(assets::AppAssets);
    app.on_open_urls(macos::url_scheme::push);
    // Clicking Loadout in the Dock (or opening it again) brings up the palette.
    app.on_reopen(windows::show_palette);
    app.run(|cx| {
        gpui_kit::init(cx);
        cx.set_activation_policy(ActivationPolicy::Accessory);
        Theme::sync_system_appearance(None, cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &OpenSettings, cx| windows::open_settings(None, cx));
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
        ]);
        // Shown only while Loadout is in the Dock (Settings → General).
        cx.set_menus([Menu::new("Loadout").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit Loadout", Quit),
        ])]);
        views::palette::init(cx);
        let state = cx.new(|_| state::AppState::new());
        cx.set_global(state::GlobalAppState(state.clone()));
        state.update(cx, |state, cx| state.load_token(cx));
        cx.on_system_notification_response(|_, cx| {
            windows::open_settings(Some(views::settings::Page::Skills), cx)
        });
        let ticker = state.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(FIRST_CHECK).await;
            loop {
                ticker.update(cx, |state, cx| state.tick(cx));
                cx.background_executor().timer(TICK).await;
            }
        })
        .detach();

        // Startup invariants: without the menu bar item and hotkey, Loadout is unreachable.
        let mtm = objc2::MainThreadMarker::new().expect("main thread");
        cx.set_global(tray::Tray::new(mtm).expect("create menu bar item"));
        let settings = state.read(cx).data.settings.clone();
        state::AppState::apply_interface(&settings, cx);
        if settings.show_window_on_launch {
            windows::open_settings(None, cx);
        }
        let mut keys = hotkey::Hotkey::new().expect("create the global hotkey manager");
        let saved = state.read(cx).data.settings.hotkey.clone();
        if keys.set(&saved).is_err() {
            keys.set(hotkey::DEFAULT).expect("register global hotkey");
            state.update(cx, |state, _| {
                state.notice =
                    Some(format!("Couldn't use the shortcut {saved}; using ctrl+alt+L.").into())
            });
        }
        cx.set_global(keys);
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(HOTKEY_POLL).await;
                if cx.update(|cx| cx.global::<hotkey::Hotkey>().pressed()) {
                    cx.update(windows::toggle_palette);
                }
                if tray::take_click() {
                    cx.update(windows::toggle_menu);
                }
                for text in macos::url_scheme::take() {
                    cx.update(|cx| windows::offer_import(&text, cx));
                }
            }
        })
        .detach();
    });
}
