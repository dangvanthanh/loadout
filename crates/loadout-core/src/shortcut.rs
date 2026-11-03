//! The palette shortcut in its three forms: global-hotkey specs ("ctrl+alt+KeyL", saved in
//! `Settings.hotkey`), macOS symbols for display ("⌃⌥L"), and a recorded keystroke.

/// Modifiers held while recording.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

/// "ctrl+alt+KeyL" → "⌃⌥L", "cmd+shift+Space" → "⇧⌘Space" (Apple's order: ⌃⌥⇧⌘).
pub fn display(spec: &str) -> String {
    let mut mods = Mods::default();
    let mut key = String::new();
    for token in spec.split('+').map(str::trim) {
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods.ctrl = true,
            "alt" | "option" => mods.alt = true,
            "shift" => mods.shift = true,
            "cmd" | "command" | "super" | "meta" => mods.cmd = true,
            _ => key = key_label(token),
        }
    }
    let mut text = String::new();
    for (on, symbol) in [
        (mods.ctrl, "⌃"),
        (mods.alt, "⌥"),
        (mods.shift, "⇧"),
        (mods.cmd, "⌘"),
    ] {
        if on {
            text.push_str(symbol);
        }
    }
    text + &key
}

/// Punctuation keys: global-hotkey's name and the character on the key.
const PUNCTUATION: &[(&str, &str)] = &[
    ("Comma", ","),
    ("Period", "."),
    ("Slash", "/"),
    ("Semicolon", ";"),
    ("Quote", "'"),
    ("BracketLeft", "["),
    ("BracketRight", "]"),
    ("Backslash", "\\"),
    ("Minus", "-"),
    ("Equal", "="),
    ("Backquote", "`"),
];

/// "KeyL" → "L", "Digit1" → "1", "Comma" → ",", "Space" → "Space".
fn key_label(token: &str) -> String {
    if let Some((_, ch)) = PUNCTUATION
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(token))
    {
        return ch.to_string();
    }
    let bare = token
        .strip_prefix("Key")
        .or_else(|| token.strip_prefix("Digit"))
        .filter(|rest| rest.len() == 1)
        .unwrap_or(token);
    if bare.chars().count() == 1 {
        bare.to_uppercase()
    } else {
        let mut chars = bare.chars();
        chars
            .next()
            .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
    }
}

/// A recorded keystroke (GPUI key names: "l", "space", "f5", ",") → a spec global-hotkey
/// accepts. `None` without ⌃, ⌥, or ⌘ (a bare key or ⇧-only would fire while typing).
pub fn from_keystroke(mods: Mods, key: &str) -> Option<String> {
    if !(mods.ctrl || mods.alt || mods.cmd) {
        return None;
    }
    let lower = key.to_ascii_lowercase();
    let code = match lower.as_str() {
        k if k.len() == 1 && k.as_bytes()[0].is_ascii_alphabetic() => {
            format!("Key{}", k.to_uppercase())
        }
        k if k.len() == 1 && k.as_bytes()[0].is_ascii_digit() => format!("Digit{k}"),
        "space" => "Space".into(),
        "enter" => "Enter".into(),
        "tab" => "Tab".into(),
        "up" | "down" | "left" | "right" => format!("Arrow{}", key_label(&lower)),
        k if k.starts_with('f') && k[1..].parse::<u8>().is_ok_and(|n| (1..=20).contains(&n)) => {
            k.to_uppercase()
        }
        k => PUNCTUATION
            .iter()
            .find(|(_, ch)| *ch == k)
            .map(|(name, _)| name.to_string())?,
    };
    let mut parts: Vec<&str> = Vec::new();
    for (on, name) in [
        (mods.ctrl, "ctrl"),
        (mods.alt, "alt"),
        (mods.shift, "shift"),
        (mods.cmd, "cmd"),
    ] {
        if on {
            parts.push(name);
        }
    }
    Some(format!("{}+{code}", parts.join("+")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_specs_as_symbols() {
        assert_eq!(display("ctrl+alt+KeyL"), "⌃⌥L");
        assert_eq!(display("cmd+shift+Space"), "⇧⌘Space");
        assert_eq!(display("shift+cmd+ctrl+Digit1"), "⌃⇧⌘1");
        assert_eq!(display("option+command+F5"), "⌥⌘F5");
        assert_eq!(display("ctrl+alt+l"), "⌃⌥L");
        assert_eq!(display("alt+Comma"), "⌥,");
    }

    #[test]
    fn recorded_keystrokes_become_specs() {
        let ctrl_alt = Mods {
            ctrl: true,
            alt: true,
            ..Mods::default()
        };
        assert_eq!(
            from_keystroke(ctrl_alt, "l").as_deref(),
            Some("ctrl+alt+KeyL")
        );
        let cmd_shift = Mods {
            shift: true,
            cmd: true,
            ..Mods::default()
        };
        assert_eq!(
            from_keystroke(cmd_shift, "space").as_deref(),
            Some("shift+cmd+Space")
        );
        let alt = Mods {
            alt: true,
            ..Mods::default()
        };
        assert_eq!(from_keystroke(alt, "5").as_deref(), Some("alt+Digit5"));
        assert_eq!(from_keystroke(alt, "f5").as_deref(), Some("alt+F5"));
        assert_eq!(from_keystroke(alt, ",").as_deref(), Some("alt+Comma"));
        // Round trip through the display form.
        assert_eq!(display(&from_keystroke(ctrl_alt, "l").unwrap()), "⌃⌥L");
        // Not shortcuts: no modifier, shift only, an unknown key.
        assert_eq!(from_keystroke(Mods::default(), "l"), None);
        assert_eq!(
            from_keystroke(
                Mods {
                    shift: true,
                    ..Mods::default()
                },
                "l"
            ),
            None
        );
        assert_eq!(from_keystroke(ctrl_alt, "§§"), None);
    }
}
