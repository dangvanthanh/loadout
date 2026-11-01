//! "The project you're in": the frontmost app's folder (U1, DECISIONS 012).
//!
//! Finder answers through AppleScript (Automation permission); other apps through the
//! Accessibility `AXDocument` of their focused window (Accessibility permission).

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use loadout_core::project::{folder_for, path_from_document};
use objc2_app_kit::NSWorkspace;

const FINDER: &str = "com.apple.finder";

const FINDER_SCRIPT: &str = r#"tell application "Finder"
    if (count of Finder windows) is 0 then return POSIX path of (desktop as alias)
    return POSIX path of (target of front Finder window as alias)
end tell"#;

#[derive(Clone, Debug)]
pub struct FrontApp {
    pub name: String,
    pub bundle_id: String,
    pub pid: i32,
}

#[derive(Debug)]
pub enum Detection {
    Folder(PathBuf),
    NeedsAutomation,
    NeedsAccessibility,
    /// The app doesn't expose a folder (or isn't showing one right now).
    Unsupported,
}

/// The frontmost app. Call before the palette shows, so it is never Loadout itself.
pub fn frontmost_app() -> Option<FrontApp> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let pid = app.processIdentifier();
    if pid == std::process::id() as i32 {
        return None;
    }
    Some(FrontApp {
        name: app
            .localizedName()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        bundle_id: app
            .bundleIdentifier()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        pid,
    })
}

/// Blocking (AppleScript round trip or AX calls); run on the background executor.
pub fn detect(app: &FrontApp) -> Detection {
    if app.bundle_id == FINDER {
        return finder_folder();
    }
    if !ax_trusted() {
        return Detection::NeedsAccessibility;
    }
    ax_document(app.pid)
        .and_then(|doc| path_from_document(&doc))
        .and_then(|path| folder_for(&path))
        .map_or(Detection::Unsupported, Detection::Folder)
}

fn finder_folder() -> Detection {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", FINDER_SCRIPT])
        .stdin(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Detection::Unsupported;
    };
    if !output.status.success() {
        // -1743: the user hasn't allowed Loadout to control Finder.
        let denied = String::from_utf8_lossy(&output.stderr).contains("-1743");
        return if denied {
            Detection::NeedsAutomation
        } else {
            // Windows without a folder target (Recents, search results).
            Detection::Unsupported
        };
    }
    let path = String::from_utf8_lossy(&output.stdout);
    path_from_document(path.trim())
        .and_then(|path| folder_for(&path))
        .map_or(Detection::Unsupported, Detection::Folder)
}

type AXUIElementRef = CFTypeRef;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

/// Whether Loadout may use Accessibility. Shows the system prompt once per launch.
fn ax_trusted() -> bool {
    static PROMPTED: AtomicBool = AtomicBool::new(false);
    let prompt = !PROMPTED.swap(true, Ordering::Relaxed);
    // SAFETY: kAXTrustedCheckOptionPrompt is a constant CFString owned by the framework.
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::from(prompt))]);
    // SAFETY: options is a live CFDictionary for the duration of the call.
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
}

fn ax_document(pid: i32) -> Option<String> {
    // SAFETY: returns a new AXUIElement we own (Create rule); never null for a pid.
    let app = unsafe { CFType::wrap_under_create_rule(AXUIElementCreateApplication(pid)) };
    let window =
        copy_attribute(&app, "AXFocusedWindow").or_else(|| copy_attribute(&app, "AXMainWindow"))?;
    copy_attribute(&window, "AXDocument")?
        .downcast::<CFString>()
        .map(|doc| doc.to_string())
}

fn copy_attribute(element: &CFType, name: &str) -> Option<CFType> {
    let name = CFString::new(name);
    let mut value: CFTypeRef = std::ptr::null();
    // SAFETY: element is a live AXUIElement and name a live CFString; value is an out pointer.
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut value,
        )
    };
    // SAFETY: on success the value is returned under the Copy rule, so we own it.
    (err == 0 && !value.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(value) })
}
