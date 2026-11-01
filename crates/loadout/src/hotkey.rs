use anyhow::Result;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// ⌃⌥L, in `global-hotkey`'s string format (same as `Settings.hotkey`).
pub const DEFAULT: &str = "ctrl+alt+KeyL";

pub struct Hotkey {
    // Dropping the manager unregisters the hotkey.
    _manager: GlobalHotKeyManager,
    id: u32,
}

impl Hotkey {
    /// Must run on the main thread with the app's event loop.
    pub fn register(spec: &str) -> Result<Self> {
        let hotkey: HotKey = spec.parse()?;
        let manager = GlobalHotKeyManager::new()?;
        manager.register(hotkey)?;
        Ok(Self {
            _manager: manager,
            id: hotkey.id(),
        })
    }

    /// Drains pending events; true if the hotkey was pressed since the last call.
    pub fn pressed(&self) -> bool {
        let mut pressed = false;
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            pressed |= event.id == self.id && event.state == HotKeyState::Pressed;
        }
        pressed
    }
}
