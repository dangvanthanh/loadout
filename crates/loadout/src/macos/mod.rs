//! macOS glue. Keep `unsafe` small, each with a `// SAFETY:` comment.

pub mod frontmost;

use gpui_kit::DisplayId;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSNumber, ns_string};

/// The screen holding the key window of the frontmost app (AppKit's `mainScreen`).
pub fn active_display_id() -> Option<DisplayId> {
    let mtm = MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let number = screen
        .deviceDescription()
        .objectForKey(ns_string!("NSScreenNumber"))?
        .downcast::<NSNumber>()
        .ok()?;
    // GPUI's macOS DisplayId is the CGDirectDisplayID, which NSScreenNumber holds.
    Some(DisplayId::new(number.unsignedIntValue() as u64))
}
