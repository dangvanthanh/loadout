use std::time::{Duration, Instant};

use gpui_kit::*;

use crate::macos::{self, frontmost};
use crate::state::{AppState, GlobalAppState};
use crate::tray::Tray;
use crate::views::palette::{self, Palette};
use crate::views::popup::{self, Popup};

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
    let favorites = state.read(cx).data.favorites.len();
    let size = size(palette::WIDTH, palette::height(favorites));
    // Centered horizontally, vertically centered at 1/3 of the screen (display-relative).
    let origin = display.as_ref().map_or(point(px(0.), px(0.)), |display| {
        let screen = display.bounds().size;
        point(
            (screen.width - size.width) / 2.,
            screen.height / 3. - size.height / 2.,
        )
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size })),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: display.map(|display| display.id()),
        ..Default::default()
    };
    let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Palette::new(state, window, cx))
    })?;
    Ok(handle)
}

#[derive(Default)]
struct PopupWindow {
    handle: Option<AnyWindowHandle>,
    blurred_at: Option<Instant>,
}

impl Global for PopupWindow {}

/// A click on the icon first blurs (and closes) the popup; that same click must not reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(400);

/// Toggles the popup under the menu bar icon.
pub fn toggle_popup(cx: &mut App) {
    let popup = cx.default_global::<PopupWindow>();
    let recently_blurred = popup
        .blurred_at
        .take()
        .is_some_and(|at| at.elapsed() < REOPEN_GUARD);
    if let Some(handle) = popup.handle.take()
        && handle
            .update(cx, |_, window, _| window.remove_window())
            .is_ok()
    {
        return;
    }
    if recently_blurred {
        return;
    }
    match open_popup(cx) {
        Ok(handle) => cx.default_global::<PopupWindow>().handle = Some(handle),
        Err(err) => eprintln!("couldn't open the popup: {err:#}"),
    }
}

pub fn popup_blurred(cx: &mut App) {
    cx.default_global::<PopupWindow>().blurred_at = Some(Instant::now());
}

fn open_popup(cx: &mut App) -> Result<AnyWindowHandle> {
    let (display_id, icon) = cx
        .global::<Tray>()
        .icon_frame()
        .ok_or_else(|| anyhow::anyhow!("menu bar icon has no window"))?;
    let display = cx
        .find_display(display_id)
        .ok_or_else(|| anyhow::anyhow!("menu bar icon is on an unknown display"))?;
    let size = size(popup::WIDTH, popup::height());
    // Centered under the icon, kept 8 px inside the screen edges.
    let max_x = display.bounds().size.width - size.width - px(8.);
    let x = (icon.center().x - size.width / 2.).max(px(8.)).min(max_x);
    let origin = point(x, icon.bottom() + px(4.));
    let state = cx.global::<GlobalAppState>().0.clone();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size })),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display_id),
        ..Default::default()
    };
    let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Popup::new(state, window, cx))
    })?;
    Ok(handle)
}
