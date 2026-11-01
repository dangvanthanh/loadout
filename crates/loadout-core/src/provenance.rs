//! Local, append-only audit log of what Loadout did (U8, DECISIONS 015).
//! Entries are only ever pushed with `append`; nothing edits or removes them.

use std::path::Path;

use uuid::Uuid;

use crate::model::{Favorite, ProvenanceEntry, SavedState};

pub const INSTALL: &str = "install";
pub const UPDATE: &str = "update";
pub const REMOVE: &str = "remove";
pub const APPLY_LOADOUT: &str = "apply_loadout";

/// Unix seconds now.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// "just now", "1 minute ago", "4 hours ago", "3 days ago". Future times read as "just now".
pub fn ago(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then);
    let (n, unit) = match secs {
        0..60 => return "just now".into(),
        60..3600 => (secs / 60, "minute"),
        3600..86_400 => (secs / 3600, "hour"),
        _ => (secs / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// A new entry for `favorite`; set `tree_sha`, `agent`, `loadout_id`, `note` as known.
/// `project` is `None` for global installs.
pub fn entry(action: &str, project: Option<&Path>, favorite: &Favorite) -> ProvenanceEntry {
    ProvenanceEntry {
        id: Uuid::new_v4(),
        ts: now(),
        action: action.to_string(),
        project_path: project.map(|p| p.to_string_lossy().into_owned()),
        skill_source: favorite.source.clone(),
        skill_name: favorite.skill.clone().filter(|s| !s.is_empty()),
        ..Default::default()
    }
}

pub fn append(state: &mut SavedState, entry: ProvenanceEntry) {
    state.provenance.push(entry);
}

/// Newest first; among equal timestamps the later append comes first.
fn newest_first<'a>(
    entries: impl DoubleEndedIterator<Item = &'a ProvenanceEntry>,
) -> Vec<&'a ProvenanceEntry> {
    let mut list: Vec<_> = entries.rev().collect();
    list.sort_by_key(|e| std::cmp::Reverse(e.ts));
    list
}

/// Newest first, at most `limit`.
pub fn recent(entries: &[ProvenanceEntry], limit: usize) -> Vec<&ProvenanceEntry> {
    let mut list = newest_first(entries.iter());
    list.truncate(limit);
    list
}

/// Newest first, for one project folder.
pub fn for_project<'a>(entries: &'a [ProvenanceEntry], project: &Path) -> Vec<&'a ProvenanceEntry> {
    let project = project.to_string_lossy();
    newest_first(
        entries
            .iter()
            .filter(|e| e.project_path.as_deref() == Some(&*project)),
    )
}

/// Newest first, for one favorite's source + skill.
pub fn for_skill<'a>(
    entries: &'a [ProvenanceEntry],
    favorite: &Favorite,
) -> Vec<&'a ProvenanceEntry> {
    let skill = favorite.skill.as_deref().filter(|s| !s.is_empty());
    newest_first(
        entries
            .iter()
            .filter(|e| e.skill_source == favorite.source && e.skill_name.as_deref() == skill),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::favorite;
    use crate::storage;

    fn at(mut e: ProvenanceEntry, ts: u64) -> ProvenanceEntry {
        e.ts = ts;
        e
    }

    fn sample() -> (SavedState, Favorite, Favorite) {
        let fd = favorite(
            "frontend-design",
            "anthropics/skills",
            Some("frontend-design"),
        );
        let tdd = favorite("tdd", "mattpocock/skills", Some("tdd"));
        let shop = Path::new("/Users/me/Code/shop");
        let blog = Path::new("/Users/me/Code/blog");
        let mut state = SavedState::default();
        append(&mut state, at(entry(INSTALL, Some(shop), &fd), 100));
        append(&mut state, at(entry(INSTALL, Some(blog), &tdd), 200));
        append(&mut state, at(entry(UPDATE, Some(shop), &fd), 300));
        append(&mut state, at(entry(REMOVE, None, &tdd), 300));
        (state, fd, tdd)
    }

    #[test]
    fn relative_times() {
        let now = 1_800_000_000;
        assert_eq!(ago(now, now), "just now");
        assert_eq!(ago(now - 59, now), "just now");
        assert_eq!(ago(now - 60, now), "1 minute ago");
        assert_eq!(ago(now - 4 * 60 - 30, now), "4 minutes ago");
        assert_eq!(ago(now - 3600, now), "1 hour ago");
        assert_eq!(ago(now - 5 * 3600, now), "5 hours ago");
        assert_eq!(ago(now - 86_400, now), "1 day ago");
        assert_eq!(ago(now - 40 * 86_400, now), "40 days ago");
        assert_eq!(ago(now + 10, now), "just now");
    }

    #[test]
    fn entry_fields() {
        let fav = favorite(
            "frontend-design",
            "anthropics/skills",
            Some("frontend-design"),
        );
        let e = entry(INSTALL, Some(Path::new("/p")), &fav);
        assert_eq!(e.action, "install");
        assert_eq!(e.project_path.as_deref(), Some("/p"));
        assert_eq!(e.skill_source, "anthropics/skills");
        assert_eq!(e.skill_name.as_deref(), Some("frontend-design"));
        assert!(e.ts >= 1_790_000_000);
        assert_ne!(e.id, Uuid::nil());
        let whole = entry(
            INSTALL,
            None,
            &favorite("skills", "anthropics/skills", None),
        );
        assert_eq!((whole.project_path, whole.skill_name), (None, None));
    }

    #[test]
    fn append_only_keeps_order() {
        let (state, _, _) = sample();
        let actions: Vec<_> = state.provenance.iter().map(|e| e.action.as_str()).collect();
        assert_eq!(actions, ["install", "install", "update", "remove"]);
    }

    #[test]
    fn recent_is_newest_first_and_later_append_wins_ties() {
        let (state, _, _) = sample();
        let actions: Vec<_> = recent(&state.provenance, 3)
            .iter()
            .map(|e| e.action.as_str())
            .collect();
        assert_eq!(actions, ["remove", "update", "install"]);
    }

    #[test]
    fn filters_by_project_and_skill() {
        let (state, fd, tdd) = sample();
        let shop: Vec<_> = for_project(&state.provenance, Path::new("/Users/me/Code/shop"))
            .iter()
            .map(|e| e.ts)
            .collect();
        assert_eq!(shop, [300, 100]);
        assert_eq!(for_skill(&state.provenance, &fd).len(), 2);
        assert_eq!(for_skill(&state.provenance, &tdd)[0].action, "remove");
        let whole = favorite("skills", "anthropics/skills", None);
        assert!(for_skill(&state.provenance, &whole).is_empty());
    }

    #[test]
    fn entries_survive_round_trip() {
        let (mut state, _, _) = sample();
        state.provenance[2].tree_sha = Some("t3fd".into());
        state.provenance[2].agent = Some("claude-code".into());
        let dir = std::env::temp_dir().join(format!("loadout-provenance-{}", Uuid::new_v4()));
        let path = dir.join("data.json");
        storage::save(&path, &state).unwrap();
        let loaded = storage::load(&path, None).unwrap().state;
        assert_eq!(loaded.provenance, state.provenance);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
