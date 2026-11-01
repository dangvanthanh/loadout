//! What the current project has versus a loadout (U7): missing, present, and extra skills.

use crate::installed::Installed;
use crate::model::{SavedState, Uuid};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadoutDiff {
    /// Loadout skills not in the project, in the loadout's order.
    pub missing: Vec<Uuid>,
    /// Loadout skills already there.
    pub present: Vec<Uuid>,
    /// Skills in the project (lock file / agent folders) that no loadout skill accounts for,
    /// by folder name, sorted.
    pub extra: Vec<String>,
}

/// `installed_here` says whether a favorite is in the project (Loadout's records or `installed`).
pub fn diff(
    state: &SavedState,
    loadout: Uuid,
    installed: &Installed,
    installed_here: impl Fn(Uuid) -> bool,
) -> LoadoutDiff {
    if state.loadout(loadout).is_none() {
        return LoadoutDiff::default();
    }
    let members = state.loadout_favorites(loadout);
    let (present, missing): (Vec<Uuid>, Vec<Uuid>) = members
        .iter()
        .map(|f| f.id)
        .partition(|id| installed_here(*id));
    let covered: Vec<&str> = members.iter().flat_map(|f| installed.matching(f)).collect();
    let extra = installed
        .skills
        .keys()
        .filter(|name| !covered.contains(&name.as_str()))
        .cloned()
        .collect();
    LoadoutDiff {
        missing,
        present,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::{agents, installed, parser::parse};

    fn project() -> Installed {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/installed");
        installed::read(&root, &agents::parse_list("claude-code, codex"))
    }

    #[test]
    fn splits_missing_present_extra() {
        let mut state = SavedState::default();
        state.add_favorites(parse("anthropics/skills@frontend-design"));
        state.add_favorites(parse("anthropics/skills@pdf"));
        state.add_favorites(parse("mattpocock/skills@grill-me"));
        let (fd, pdf, grill) = (
            state.favorites[0].id,
            state.favorites[1].id,
            state.favorites[2].id,
        );
        let web = state
            .create_loadout("Web app", &[pdf, fd, grill], vec![])
            .unwrap();
        let installed = project();
        let here = |id: Uuid| {
            state
                .favorites
                .iter()
                .find(|f| f.id == id)
                .is_some_and(|f| installed.contains(f))
        };

        let diff = diff(&state, web, &installed, here);
        assert_eq!(diff.missing, [pdf, grill]);
        assert_eq!(diff.present, [fd]);
        // tdd is in .agents/skills but not in the loadout.
        assert_eq!(diff.extra, ["tdd"]);
    }

    #[test]
    fn recorded_installs_count_and_whole_sources_cover_their_skills() {
        let mut state = SavedState::default();
        state.add_favorites(parse("anthropics/skills"));
        state.add_favorites(parse("mattpocock/skills@tdd"));
        let (all, tdd) = (state.favorites[0].id, state.favorites[1].id);
        let web = state
            .create_loadout("Web app", &[all, tdd], vec![])
            .unwrap();
        let installed = project();
        // Loadout's own record says tdd is here too.
        let here = |id: Uuid| {
            id == tdd
                || state
                    .favorites
                    .iter()
                    .any(|f| f.id == id && installed.contains(f))
        };

        let diff = diff(&state, web, &installed, here);
        assert!(diff.missing.is_empty());
        assert_eq!(diff.present, [all, tdd]);
        assert!(diff.extra.is_empty(), "{:?}", diff.extra);
    }

    #[test]
    fn unknown_loadout_is_empty() {
        let state = SavedState::default();
        assert_eq!(
            diff(&state, Uuid::new_v4(), &project(), |_| false),
            LoadoutDiff::default()
        );
    }
}
