use gpui_kit::*;

use crate::macos;
use crate::views::palette::{self, Palette};

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
    match open_palette(cx) {
        Ok(handle) => cx.set_global(PaletteWindow(Some(handle))),
        Err(err) => eprintln!("couldn't open the palette: {err:#}"),
    }
}

fn open_palette(cx: &mut App) -> Result<AnyWindowHandle> {
    let display_id = macos::active_display_id();
    let display = display_id
        .and_then(|id| cx.find_display(id))
        .or_else(|| cx.primary_display());
    let size = size(palette::WIDTH, palette::height());
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
        cx.new(|cx| Palette::new(window, cx))
    })?;
    Ok(handle)
}
