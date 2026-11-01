//! Menu bar item: an `NSStatusItem` whose clicks toggle the popup (DECISIONS 008).

use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::{Bounds, DisplayId, Global, Pixels, point, px, size};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSImage, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{NSData, NSObject, NSSize, ns_string};

use crate::macos;

static CLICKED: AtomicBool = AtomicBool::new(false);

/// 36 px artwork drawn at 18 pt; regenerate with `packaging/icon/make-menubar.sh`.
const ICON: &[u8] = include_bytes!("../assets/tray/menubar@2x.png");
/// The same with a dot: some project has an update.
const ICON_UPDATE: &[u8] = include_bytes!("../assets/tray/menubar-update@2x.png");

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
    icon: Retained<NSImage>,
    icon_update: Retained<NSImage>,
    // The button holds its target weakly; keep it alive here.
    _target: Retained<ClickTarget>,
}

impl Global for Tray {}

impl Tray {
    pub fn new(mtm: MainThreadMarker) -> Option<Self> {
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        let button = item.button(mtm)?;
        let icon = template_image(ICON)?;
        let icon_update = template_image(ICON_UPDATE)?;
        button.setImage(Some(&icon));
        // SAFETY: plain NSObject subclass with no ivars; `init` is NSObject's.
        let target: Retained<ClickTarget> = unsafe { msg_send![ClickTarget::alloc(mtm), init] };
        // SAFETY: `target` implements `click:` and lives as long as the item (both in `Tray`).
        unsafe {
            button.setTarget(Some(&target));
            button.setAction(Some(sel!(click:)));
        }
        Some(Self {
            item,
            icon,
            icon_update,
            _target: target,
        })
    }

    /// Shows the dot when any project has an update.
    pub fn set_updates(&self, updates: bool) {
        let Some(button) = MainThreadMarker::new().and_then(|mtm| self.item.button(mtm)) else {
            return;
        };
        button.setImage(Some(if updates {
            &self.icon_update
        } else {
            &self.icon
        }));
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

fn template_image(png: &[u8]) -> Option<Retained<NSImage>> {
    let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png))?;
    image.setSize(NSSize::new(18., 18.));
    // Template: AppKit tints it to match the menu bar (light, dark, highlighted).
    image.setTemplate(true);
    image.setAccessibilityDescription(Some(ns_string!("Loadout")));
    Some(image)
}

/// True once per click since the last call.
pub fn take_click() -> bool {
    CLICKED.swap(false, Ordering::Relaxed)
}
