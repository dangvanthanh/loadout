use anyhow::Result;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui_kit::Global;

/// ⌃⌥L, in `global-hotkey`'s string format (same as `Settings.hotkey`).
pub const DEFAULT: &str = "ctrl+alt+KeyL";

/// The palette shortcut. Re-registered live from Settings.
pub struct Hotkey {
    // Dropping the manager unregisters the hotkey.
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
}

impl Global for Hotkey {}

impl Hotkey {
    /// Must run on the main thread with the app's event loop.
    pub fn new() -> Result<Self> {
        Ok(Self {
            manager: GlobalHotKeyManager::new()?,
            current: None,
        })
    }

    /// Switches to `spec` ("ctrl+alt+L", "cmd+shift+Space"). The old shortcut stays if the new
    /// one can't be parsed or registered (e.g. another app owns it).
    pub fn set(&mut self, spec: &str) -> Result<()> {
        let hotkey: HotKey = spec.trim().parse()?;
        if self
            .current
            .is_some_and(|current| current.id() == hotkey.id())
        {
            return Ok(());
        }
        self.manager.register(hotkey)?;
        if let Some(old) = self.current.replace(hotkey) {
            self.manager.unregister(old).ok();
        }
        Ok(())
    }

    /// Drains pending events; true if the shortcut was pressed since the last call.
    pub fn pressed(&self) -> bool {
        let Some(current) = self.current else {
            return false;
        };
        let mut pressed = false;
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            pressed |= event.id == current.id() && event.state == HotKeyState::Pressed;
        }
        pressed
    }
}
