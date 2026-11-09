//! The About page: the app icon, name, and version (moved from the menu bar menu).

use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

use super::SettingsWindow;
use crate::assets::app_icon_path;

impl SettingsWindow {
    pub(super) fn render_about(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let icon = app_icon_path(self.state.read(cx).data.settings.app_icon);
        let muted = cx.theme().muted_foreground;
        vec![
            div()
                .pt_8()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(img(icon).size(px(96.)))
                .child(
                    div()
                        .pt_2()
                        .text_size(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Loadout"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(muted)
                        .child(concat!("Version ", env!("CARGO_PKG_VERSION"))),
                )
                .child(
                    div()
                        .pt_2()
                        .text_size(px(13.))
                        .text_color(muted)
                        .child("The right skills for the project you're in."),
                )
                .into_any_element(),
        ]
    }
}
