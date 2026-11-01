//! macOS glue. Keep `unsafe` small, each with a `// SAFETY:` comment.

pub mod frontmost;
pub mod launch;
pub mod login_item;

use gpui_kit::DisplayId;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSBundle, NSNumber, ns_string};

/// The screen holding the key window of the frontmost app (AppKit's `mainScreen`).
pub fn active_display_id() -> Option<DisplayId> {
    let screen = NSScreen::mainScreen(MainThreadMarker::new()?)?;
    display_id(&screen)
}

pub fn display_id(screen: &NSScreen) -> Option<DisplayId> {
    let number = screen
        .deviceDescription()
        .objectForKey(ns_string!("NSScreenNumber"))?
        .downcast::<NSNumber>()
        .ok()?;
    // GPUI's macOS DisplayId is the CGDirectDisplayID, which NSScreenNumber holds.
    Some(DisplayId::new(number.unsignedIntValue() as u64))
}

/// Notifications and login items only work from `Loadout.app`, not `cargo run`.
pub fn is_bundled() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some()
}
