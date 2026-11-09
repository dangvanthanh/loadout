//! The menu under the menu bar icon: a glance and a few actions, like a native app menu.
//! Managing skills, loadouts, and projects happens in Settings.

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use loadout_core::{provenance, shortcut};

use crate::Quit;
use crate::assets::app_icon_path;
use crate::state::{AppState, CheckReason};
use crate::views::settings::Page;
use crate::windows;

pub const WIDTH: Pixels = px(290.);
const PADDING: Pixels = px(6.);
const HEADER: Pixels = px(54.);
const SEPARATOR: Pixels = px(9.);
const ITEM: Pixels = px(28.);

/// Height for the menu; "Update All" only shows when something is pending.
pub fn height(updates: bool) -> Pixels {
    let items = if updates { 4. } else { 3. };
    PADDING * 2. + HEADER + SEPARATOR + ITEM * items + SEPARATOR + ITEM
}

pub struct Menu {
    state: Entity<AppState>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Menu {
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
                    windows::menu_blurred(cx);
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

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let cmd = keystroke.modifiers.platform;
        match (cmd, keystroke.key.as_str()) {
            (false, "escape") => {
                windows::close_menu(cx);
            }
            (true, ",") => windows::open_settings(None, cx),
            (true, "u") => self.state.update(cx, |s, cx| s.update(None, cx)),
            (true, "r") => self
                .state
                .update(cx, |s, cx| s.check_for_updates(CheckReason::Manual, cx)),
            _ => return,
        }
        cx.stop_propagation();
    }
}

/// One clickable menu row: label on the left, a shortcut or detail on the right.
fn item(
    id: &'static str,
    label: impl Into<SharedString>,
    trailing: Option<SharedString>,
    enabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.foreground.opacity(0.08);
    div()
        .id(id)
        .h(ITEM)
        .px(px(10.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .text_size(px(13.))
        .when(enabled, |row| {
            row.hover(move |s| s.bg(hover)).cursor_pointer()
        })
        .when(!enabled, |row| row.text_color(theme.muted_foreground))
        .child(label.into())
        .child(div().flex_1())
        .children(trailing.map(|text| {
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child(text)
        }))
}

fn separator(cx: &App) -> Div {
    div()
        .h(SEPARATOR)
        .px(px(10.))
        .flex()
        .items_center()
        .child(div().h(px(1.)).w_full().bg(cx.theme().border))
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let pending = state.pending().len();
        let settings = &state.data.settings;
        let hotkey: SharedString = shortcut::display(&settings.hotkey).into();
        let icon = app_icon_path(settings.app_icon);
        // The status line: what's happening now, else whether skills are current.
        let (dot, status): (Hsla, SharedString) = if state.installing {
            (theme.blue, state.install_status.clone().unwrap_or_default())
        } else if state.checking {
            (theme.muted_foreground, "Checking for updates…".into())
        } else if let Some(notice) = state.notice.clone() {
            (theme.blue, notice)
        } else if let Some(error) = state.check_error.clone() {
            (theme.warning, error)
        } else if pending > 0 {
            (
                theme.blue,
                match pending {
                    1 => "1 update available".into(),
                    n => format!("{n} updates available").into(),
                },
            )
        } else if state.last_checked.is_some() {
            (theme.success, "Up to date".into())
        } else {
            (theme.success, "Ready".into())
        };
        let checked: Option<SharedString> = state
            .last_checked
            .map(|at| provenance::ago(at, provenance::now()).into());
        let checking = state.checking;
        let busy = state.installing;
        let muted = theme.muted_foreground;
        let hover = theme.foreground.opacity(0.08);
        let state = self.state.clone();
        div()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .p(PADDING)
            .flex()
            .flex_col()
            // Glass: the window blurs what's behind it; this tint keeps text readable.
            .bg(cx.theme().background.opacity(0.78))
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .id("header")
                    .h(HEADER)
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(move |_, _, cx| {
                        let page = (pending > 0).then_some(Page::Skills);
                        windows::open_settings(page, cx)
                    })
                    .child(img(icon).size(px(34.)).rounded(px(8.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Loadout"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .text_size(px(12.))
                                    .text_color(muted)
                                    .child(div().flex_none().size(px(7.)).rounded_full().bg(dot))
                                    .child(div().min_w_0().truncate().child(status)),
                            ),
                    )
                    .child(Icon::new(IconName::ChevronRight).small().text_color(muted)),
            )
            .child(separator(cx))
            .child(
                item("palette", "Open Palette", Some(hotkey), true, cx).on_click(|_, _, cx| {
                    windows::close_menu(cx);
                    windows::toggle_palette(cx);
                }),
            )
            .when(pending > 0, |menu| {
                let state = state.clone();
                menu.child(
                    item(
                        "update-all",
                        format!("Update All ({pending})"),
                        Some("⌘U".into()),
                        !busy,
                        cx,
                    )
                    .on_click(move |_, _, cx| state.update(cx, |s, cx| s.update(None, cx))),
                )
            })
            .child({
                let state = state.clone();
                let label = if checking {
                    "Checking…"
                } else {
                    "Check for Updates…"
                };
                item("check", label, checked, !checking, cx).on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| s.check_for_updates(CheckReason::Manual, cx))
                })
            })
            .child(
                item("settings", "Settings…", Some("⌘,".into()), true, cx)
                    .on_click(|_, _, cx| windows::open_settings(None, cx)),
            )
            .child(separator(cx))
            .child(
                item("quit", "Quit Loadout", Some("⌘Q".into()), true, cx)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Quit), cx)),
            )
    }
}
