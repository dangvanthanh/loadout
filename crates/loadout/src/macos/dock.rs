//! The Dock icon's image while Loadout shows in the Dock (Settings → Interface → App icon).
//! The bundle's AppIcon.icns still decides the Finder and notification icon.

use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::NSData;

pub fn set_icon(png: &[u8]) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png)) else {
        return;
    };
    // SAFETY: on the main thread, with a valid image; AppKit retains it.
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
}
