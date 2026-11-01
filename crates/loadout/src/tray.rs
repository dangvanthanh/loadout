//! Menu bar item: an `NSStatusItem` whose clicks toggle the popup (DECISIONS 008).

use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::{Bounds, DisplayId, Global, Pixels, point, px, size};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSImage, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{NSObject, ns_string};

use crate::macos;

static CLICKED: AtomicBool = AtomicBool::new(false);

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LoadoutTrayTarget"]
    struct ClickTarget;

    impl ClickTarget {
        #[unsafe(method(click:))]
        fn click(&self, _sender: Option<&AnyObject>) {
            CLICKED.store(true, Ordering::Relaxed);
        }
    }
);

pub struct Tray {
    item: Retained<NSStatusItem>,
    // The button holds its target weakly; keep it alive here.
    _target: Retained<ClickTarget>,
}

impl Global for Tray {}

impl Tray {
    pub fn new(mtm: MainThreadMarker) -> Option<Self> {
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        let button = item.button(mtm)?;
        // An SF Symbol is a template image already: it follows the menu bar's appearance.
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            ns_string!("square.stack.3d.up"),
            Some(ns_string!("Loadout")),
        )?;
        button.setImage(Some(&image));
        // SAFETY: plain NSObject subclass with no ivars; `init` is NSObject's.
        let target: Retained<ClickTarget> = unsafe { msg_send![ClickTarget::alloc(mtm), init] };
        // SAFETY: `target` implements `click:` and lives as long as the item (both in `Tray`).
        unsafe {
            button.setTarget(Some(&target));
            button.setAction(Some(sel!(click:)));
        }
        Some(Self {
            item,
            _target: target,
        })
    }

    /// The icon's display and frame, top-left origin relative to that display (GPUI's convention).
    pub fn icon_frame(&self) -> Option<(DisplayId, Bounds<Pixels>)> {
        let mtm = MainThreadMarker::new()?;
        let window = self.item.button(mtm)?.window()?;
        let screen = window.screen()?;
        let (frame, screen_frame) = (window.frame(), screen.frame());
        let top =
            screen_frame.origin.y + screen_frame.size.height - frame.origin.y - frame.size.height;
        let bounds = Bounds {
            origin: point(
                px((frame.origin.x - screen_frame.origin.x) as f32),
                px(top as f32),
            ),
            size: size(px(frame.size.width as f32), px(frame.size.height as f32)),
        };
        Some((macos::display_id(&screen)?, bounds))
    }
}

/// True once per click since the last call.
pub fn take_click() -> bool {
    CLICKED.swap(false, Ordering::Relaxed)
}
