mod hotkey;
mod macos;
mod views;
mod windows;

use std::time::Duration;

use gpui_kit::assets::Assets;
use gpui_kit::component::Theme;
use gpui_kit::*;

actions!(loadout, [Quit]);

/// How often the hotkey receiver is drained; cheap and never redraws.
const HOTKEY_POLL: Duration = Duration::from_millis(50);

fn main() {
    gpui_kit::application().with_assets(Assets).run(|cx| {
        gpui_kit::init(cx);
        cx.set_activation_policy(ActivationPolicy::Accessory);
        Theme::sync_system_appearance(None, cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        views::palette::init(cx);

        // Startup invariant: without the hotkey the palette is unreachable.
        let hotkey = hotkey::Hotkey::register(hotkey::DEFAULT).expect("register global hotkey");
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(HOTKEY_POLL).await;
                if hotkey.pressed() {
                    cx.update(windows::toggle_palette);
                }
            }
        })
        .detach();
    });
}
