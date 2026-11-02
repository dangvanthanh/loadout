//! The idle "what changed" digest (U11): at most one calm notification per interval, and
//! only when real updates are pending.

/// Intervals the settings offer: daily and weekly.
pub const DAILY: u32 = 24;
pub const WEEKLY: u32 = 168;

/// Whether `interval_hours` have passed since `last` (never run = due). Intervals under an hour
/// count as an hour.
pub fn due(last: Option<u64>, interval_hours: u32, now: u64) -> bool {
    let Some(last) = last else { return true };
    let interval = u64::from(interval_hours.max(1)) * 3600;
    now >= last && now - last >= interval
}

/// "frontend-design can be updated." / "frontend-design, tdd, and 2 more can be updated."
/// `None` when nothing is pending.
pub fn body(names: &[&str]) -> Option<String> {
    let mut unique: Vec<&str> = Vec::new();
    for name in names {
        if !unique.contains(name) {
            unique.push(name);
        }
    }
    let list = match unique.as_slice() {
        [] => return None,
        [one] => one.to_string(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b}, and {c}"),
        [a, b, rest @ ..] => format!("{a}, {b}, and {} more", rest.len()),
    };
    Some(format!("{list} can be updated."))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn due_after_the_interval() {
        assert!(due(None, DAILY, NOW));
        assert!(!due(Some(NOW - 23 * 3600), DAILY, NOW));
        assert!(due(Some(NOW - 24 * 3600), DAILY, NOW));
        assert!(!due(Some(NOW - 6 * 86_400), WEEKLY, NOW));
        assert!(due(Some(NOW - 7 * 86_400), WEEKLY, NOW));
        assert!(due(Some(NOW - 3600), 0, NOW));
        // A clock that went backwards waits rather than firing.
        assert!(!due(Some(NOW + 600), DAILY, NOW));
    }

    #[test]
    fn body_names_a_few() {
        assert_eq!(body(&[]), None);
        assert_eq!(body(&["tdd"]).as_deref(), Some("tdd can be updated."));
        assert_eq!(
            body(&["a", "b"]).as_deref(),
            Some("a and b can be updated.")
        );
        assert_eq!(
            body(&["a", "b", "c"]).as_deref(),
            Some("a, b, and c can be updated.")
        );
        assert_eq!(
            body(&["a", "b", "c", "d", "e"]).as_deref(),
            Some("a, b, and 3 more can be updated.")
        );
        assert_eq!(
            body(&["a", "a", "b"]).as_deref(),
            Some("a and b can be updated.")
        );
    }
}
