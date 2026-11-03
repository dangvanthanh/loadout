use std::time::{Duration, Instant};

use gpui_kit::*;

use crate::macos::{self, frontmost};
use crate::state::{AppState, GlobalAppState};
use crate::tray::Tray;
use crate::views::menu::{self, Menu};
use crate::views::palette::{self, Palette};
use crate::views::settings::{Page, SettingsWindow};

#[derive(Default)]
struct PaletteWindow(Option<AnyWindowHandle>);

impl Global for PaletteWindow {}

/// Closes the palette if it is open, otherwise opens it on the active screen.
pub fn toggle_palette(cx: &mut App) {
    if let Some(handle) = cx.default_global::<PaletteWindow>().0.take()
        && handle
            .update(cx, |_, window, _| window.remove_window())
            .is_ok()
    {
        return;
    }
    // Read the frontmost app before the palette exists, so it is never Loadout.
    let app = frontmost::frontmost_app();
    let state = cx.global::<GlobalAppState>().0.clone();
    state.update(cx, |state, cx| state.detect_target(app, cx));
    match open_palette(state, cx) {
        Ok(handle) => cx.set_global(PaletteWindow(Some(handle))),
        Err(err) => eprintln!("couldn't open the palette: {err:#}"),
    }
}

fn open_palette(state: Entity<AppState>, cx: &mut App) -> Result<AnyWindowHandle> {
    let display_id = macos::active_display_id();
    let display = display_id
        .and_then(|id| cx.find_display(id))
        .or_else(|| cx.primary_display());
    // Tinycast-sized, shrunk to fit small screens; centred, top edge 18% down the visible area.
    let (size, origin) = match display.as_ref().map(|display| display.visible_bounds()) {
        Some(visible) => {
            let size = size(
                palette::WIDTH.min(visible.size.width - px(40.)),
                palette::HEIGHT.min(visible.size.height - px(40.)),
            );
            let top = visible.origin.y + visible.size.height * palette::TOP_FRACTION;
            // Keep the bottom on screen when the visible area is short.
            let top = top.min(visible.origin.y + visible.size.height - size.height);
            let left = visible.origin.x + (visible.size.width - size.width) / 2.;
            (size, point(left, top))
        }
        None => (size(palette::WIDTH, palette::HEIGHT), point(px(0.), px(0.))),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size })),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // Frosted glass behind the views' tinted backgrounds, like Tinycast.
        window_background: WindowBackgroundAppearance::Blurred,
        display_id: display.map(|display| display.id()),
        ..Default::default()
    };
    let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Palette::new(state, window, cx))
    })?;
    Ok(handle)
}

#[derive(Default)]
struct MenuWindow {
    handle: Option<AnyWindowHandle>,
    blurred_at: Option<Instant>,
}

impl Global for MenuWindow {}

/// A click on the icon first blurs (and closes) the menu; that same click must not reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(400);

/// Toggles the menu under the menu bar icon.
pub fn toggle_menu(cx: &mut App) {
    let menu = cx.default_global::<MenuWindow>();
    let recently_blurred = menu
        .blurred_at
        .take()
        .is_some_and(|at| at.elapsed() < REOPEN_GUARD);
    if close_menu(cx) || recently_blurred {
        return;
    }
    match open_menu(cx) {
        Ok(handle) => cx.default_global::<MenuWindow>().handle = Some(handle),
        Err(err) => eprintln!("couldn't open the menu: {err:#}"),
    }
}

/// Closes the menu; true if it was open.
pub fn close_menu(cx: &mut App) -> bool {
    let closed = cx
        .default_global::<MenuWindow>()
        .handle
        .take()
        .is_some_and(|handle| {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .is_ok()
        });
    if closed {
        clear_notice(cx);
    }
    closed
}

pub fn menu_blurred(cx: &mut App) {
    cx.default_global::<MenuWindow>().blurred_at = Some(Instant::now());
    clear_notice(cx);
}

/// Notices are for the moment they happen; the next look starts clean.
pub fn clear_notice(cx: &mut App) {
    let state = cx.global::<GlobalAppState>().0.clone();
    state.update(cx, |state, _| state.notice = None);
}

fn open_menu(cx: &mut App) -> Result<AnyWindowHandle> {
    let (display_id, icon) = cx
        .global::<Tray>()
        .icon_frame()
        .ok_or_else(|| anyhow::anyhow!("menu bar icon has no window"))?;
    let display = cx
        .find_display(display_id)
        .ok_or_else(|| anyhow::anyhow!("menu bar icon is on an unknown display"))?;
    let state = cx.global::<GlobalAppState>().0.clone();
    let updates = !state.read(cx).pending().is_empty();
    let size = size(menu::WIDTH, menu::height(updates));
    // Centered under the icon, kept 8 px inside the screen edges.
    let max_x = display.bounds().size.width - size.width - px(8.);
    let x = (icon.center().x - size.width / 2.).max(px(8.)).min(max_x);
    let origin = point(x, icon.bottom() + px(4.));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size })),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // Frosted glass behind the view's tinted background, like Tinycast.
        window_background: WindowBackgroundAppearance::Blurred,
        display_id: Some(display_id),
        ..Default::default()
    };
    let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Menu::new(state, window, cx))
    })?;
    Ok(handle)
}

#[derive(Default)]
struct SettingsWindowHandle(Option<(AnyWindowHandle, WeakEntity<SettingsWindow>)>);

impl Global for SettingsWindowHandle {}

/// Brings up Settings (opening it if needed), on `page` if given.
pub fn open_settings(page: Option<Page>, cx: &mut App) {
    close_menu(cx);
    cx.activate(true);
    if let Some((handle, view)) = cx.default_global::<SettingsWindowHandle>().0.clone() {
        let shown = handle.update(cx, |_, window, cx| {
            window.activate_window();
            if let Some(page) = page {
                view.update(cx, |view, cx| view.show_page(page, cx)).ok();
            }
        });
        if shown.is_ok() {
            return;
        }
    }
    let state = cx.global::<GlobalAppState>().0.clone();
    let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Loadout Settings".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.), px(18.))),
        }),
        kind: WindowKind::Normal,
        focus: true,
        show: true,
        is_resizable: true,
        is_minimizable: true,
        window_min_size: Some(size(px(700.), px(480.))),
        ..Default::default()
    };
    let page = page.unwrap_or_default();
    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| SettingsWindow::new(state, page, window, cx))
    }) {
        Ok((handle, view)) => {
            cx.set_global(SettingsWindowHandle(Some((handle, view.downgrade()))));
        }
        Err(err) => eprintln!("couldn't open Settings: {err:#}"),
    }
}

/// A link or file to import: previewed on Settings → Loadouts.
pub fn offer_import(text: &str, cx: &mut App) {
    let state = cx.global::<GlobalAppState>().0.clone();
    state.update(cx, |state, cx| state.offer_import(text, cx));
    open_settings(Some(Page::Loadouts), cx);
}
