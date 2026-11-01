//! Persisted data. Keys are camelCase and compatible with the prototype's `data.json`.
//! Never rename or remove a key; every field has a default.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

/// One saved skill (or a whole repo of skills).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Favorite {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    /// Anything `npx skills add` accepts: owner/repo, a GitHub/GitLab URL, a site URL.
    pub source: String,
    /// One skill inside the source; `None` = every skill in it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// Stack tags, e.g. `["react", "nextjs"]`.
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Project {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub path: PathBuf,
    /// Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<u64>,
}

impl Project {
    pub fn new(path: PathBuf, last_used: Option<u64>) -> Self {
        Self {
            id: Uuid::new_v4(),
            path,
            last_used,
        }
    }
}

/// A named set of favorites.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Loadout {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    pub favorite_ids: Vec<Uuid>,
    /// Suggest this loadout for matching stacks.
    pub tags: Vec<String>,
    /// Agent id to launch after applying, e.g. "pi", "claude-code", "cursor".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_agent: Option<String>,
    /// For shared team loadouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Comma-separated agent ids passed with `-a`. Empty = let the CLI detect.
    pub agents: String,
    /// Install to the home folder (`-g`) instead of the project.
    pub global: bool,
    /// Check GitHub for new versions in the background (prototype key).
    pub auto_check: bool,
    /// `global-hotkey` format.
    pub hotkey: String,
    pub digest_enabled: bool,
    pub digest_interval_hours: u32,
    /// Default for equip + launch.
    pub launch_after_install: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            agents: "claude-code".into(),
            global: false,
            auto_check: true,
            hotkey: "ctrl+alt+KeyL".into(),
            digest_enabled: false,
            digest_interval_hours: 24,
            launch_after_install: false,
        }
    }
}

/// One local audit record (U8). Append-only.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProvenanceEntry {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    /// Unix seconds.
    pub ts: u64,
    /// "install" | "update" | "remove" | "apply_loadout"
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    pub skill_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loadout_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedState {
    pub favorites: Vec<Favorite>,
    pub projects: Vec<Project>,
    pub loadouts: Vec<Loadout>,
    pub settings: Settings,
    #[serde(rename = "selectedProjectID", skip_serializing_if = "Option::is_none")]
    pub selected_project_id: Option<Uuid>,
    /// "<project id or global>|<favorite id>" → installed version ("?" = unknown). Prototype key.
    pub installed: BTreeMap<String, String>,
    pub provenance: Vec<ProvenanceEntry>,
}

/// Outcome of `SavedState::add_favorites`, by display name.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Added {
    pub added: Vec<String>,
    /// Already saved (same source and skill).
    pub skipped: Vec<String>,
}

/// `installed` key: "<project id or global>|<favorite id>", uppercase like the prototype.
pub fn installed_key(project: Option<Uuid>, favorite: Uuid) -> String {
    let upper = |id: Uuid| id.hyphenated().to_string().to_uppercase();
    let scope = project.map_or_else(|| "global".to_string(), upper);
    format!("{scope}|{}", upper(favorite))
}

impl SavedState {
    pub fn is_installed(&self, project: Option<Uuid>, favorite: Uuid) -> bool {
        self.installed
            .contains_key(&installed_key(project, favorite))
    }

    /// Records an install; the version stays "?" until an update check knows it.
    pub fn mark_installed(&mut self, project: Option<Uuid>, favorite: Uuid, version: Option<&str>) {
        let entry = self
            .installed
            .entry(installed_key(project, favorite))
            .or_insert_with(|| "?".into());
        if let Some(version) = version {
            *entry = version.to_string();
        }
    }

    /// The saved project for `path`, added if new.
    pub fn project_id(&mut self, path: &std::path::Path) -> Uuid {
        if let Some(project) = self.projects.iter().find(|p| p.path == path) {
            return project.id;
        }
        let project = Project::new(path.to_path_buf(), None);
        let id = project.id;
        self.projects.push(project);
        id
    }

    /// Adds favorites not already saved; untagged ones get `tags::suggest_tags`.
    pub fn add_favorites(&mut self, favorites: Vec<Favorite>) -> Added {
        let mut result = Added::default();
        for mut favorite in favorites {
            let same = |f: &Favorite| {
                f.source.eq_ignore_ascii_case(&favorite.source)
                    && f.skill.as_deref().unwrap_or("") == favorite.skill.as_deref().unwrap_or("")
            };
            if self.favorites.iter().any(same) {
                result.skipped.push(favorite.name);
                continue;
            }
            if favorite.tags.is_empty() {
                favorite.tags = crate::tags::suggest_tags(&favorite);
            }
            result.added.push(favorite.name.clone());
            self.favorites.push(favorite);
        }
        result
    }

    fn favorite_mut(&mut self, id: Uuid) -> Option<&mut Favorite> {
        self.favorites.iter_mut().find(|f| f.id == id)
    }

    /// Trimmed; empty names are refused.
    pub fn rename_favorite(&mut self, id: Uuid, name: &str) -> bool {
        let name = name.trim();
        match self.favorite_mut(id) {
            Some(favorite) if !name.is_empty() => {
                favorite.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    pub fn set_favorite_tags(&mut self, id: Uuid, tags: Vec<String>) -> bool {
        self.favorite_mut(id).map(|f| f.tags = tags).is_some()
    }

    /// Also drops the favorite from loadouts and from install records.
    pub fn remove_favorite(&mut self, id: Uuid) -> Option<Favorite> {
        let index = self.favorites.iter().position(|f| f.id == id)?;
        for loadout in &mut self.loadouts {
            loadout.favorite_ids.retain(|f| *f != id);
        }
        // Keys are "<project id or global>|<favorite id>".
        self.installed.retain(|key, _| {
            key.rsplit_once('|')
                .and_then(|(_, fav)| Uuid::parse_str(fav).ok())
                != Some(id)
        });
        Some(self.favorites.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{favorite, parse};

    #[test]
    fn installed_keys_match_the_prototype() {
        let project = Uuid::parse_str("7c1d2e3f-4a5b-4c6d-8e9f-0a1b2c3d4e5f").unwrap();
        let fav = Uuid::parse_str("e621e1f8-c36c-495a-93fc-0c247a3e6e5f").unwrap();
        assert_eq!(
            installed_key(Some(project), fav),
            "7C1D2E3F-4A5B-4C6D-8E9F-0A1B2C3D4E5F|E621E1F8-C36C-495A-93FC-0C247A3E6E5F"
        );
        assert_eq!(
            installed_key(None, fav),
            "global|E621E1F8-C36C-495A-93FC-0C247A3E6E5F"
        );
    }

    #[test]
    fn mark_and_query_installs() {
        let mut state = SavedState::default();
        let shop = state.project_id(std::path::Path::new("/Code/shop"));
        assert_eq!(state.project_id(std::path::Path::new("/Code/shop")), shop);
        assert_eq!(state.projects.len(), 1);
        let fav = Uuid::new_v4();
        assert!(!state.is_installed(Some(shop), fav));
        state.mark_installed(Some(shop), fav, None);
        assert!(state.is_installed(Some(shop), fav));
        assert!(!state.is_installed(None, fav));
        assert_eq!(state.installed[&installed_key(Some(shop), fav)], "?");
        state.mark_installed(Some(shop), fav, Some("t1fd"));
        assert_eq!(state.installed[&installed_key(Some(shop), fav)], "t1fd");
        // A later "?" doesn't erase a known version.
        state.mark_installed(Some(shop), fav, None);
        assert_eq!(state.installed[&installed_key(Some(shop), fav)], "t1fd");
    }

    #[test]
    fn add_skips_duplicates_and_suggests_tags() {
        let mut state = SavedState::default();
        let first = state.add_favorites(parse("anthropics/skills@frontend-design"));
        assert_eq!(first.added, ["frontend-design"]);
        assert_eq!(state.favorites[0].tags, ["frontend"]);

        let again = state.add_favorites(parse(
            "npx skills add Anthropics/Skills --skill frontend-design --skill pdf",
        ));
        assert_eq!(
            again,
            Added {
                added: vec!["pdf".into()],
                skipped: vec!["frontend-design".into()]
            }
        );
        assert_eq!(state.favorites.len(), 2);

        // Whole source and one skill from it are different favorites.
        assert_eq!(
            state.add_favorites(parse("anthropics/skills")).added,
            ["skills"]
        );
    }

    #[test]
    fn add_keeps_given_tags() {
        let mut state = SavedState::default();
        let mut fav = favorite("tdd", "mattpocock/skills", Some("tdd"));
        fav.tags = vec!["custom".into()];
        state.add_favorites(vec![fav]);
        assert_eq!(state.favorites[0].tags, ["custom"]);
    }

    #[test]
    fn rename_and_tags() {
        let mut state = SavedState::default();
        state.add_favorites(parse("mattpocock/skills@tdd"));
        let id = state.favorites[0].id;
        assert!(state.rename_favorite(id, "  Test first  "));
        assert_eq!(state.favorites[0].name, "Test first");
        assert!(!state.rename_favorite(id, "   "));
        assert!(!state.rename_favorite(Uuid::new_v4(), "x"));
        assert!(state.set_favorite_tags(id, vec!["testing".into()]));
        assert_eq!(state.favorites[0].tags, ["testing"]);
    }

    #[test]
    fn remove_cleans_loadouts_and_installs() {
        let mut state = SavedState::default();
        state.add_favorites(parse("npx skills add a/b --skill x --skill y"));
        let (x, y) = (state.favorites[0].id, state.favorites[1].id);
        state.loadouts.push(Loadout {
            name: "L".into(),
            favorite_ids: vec![x, y],
            ..Default::default()
        });
        let x_key = format!("global|{}", x.hyphenated().to_string().to_uppercase());
        let y_key = format!("global|{}", y.hyphenated().to_string().to_uppercase());
        state.installed.insert(x_key.clone(), "v1".into());
        state.installed.insert(y_key.clone(), "v1".into());

        assert_eq!(state.remove_favorite(x).map(|f| f.name), Some("x".into()));
        assert_eq!(state.favorites.len(), 1);
        assert_eq!(state.loadouts[0].favorite_ids, [y]);
        assert!(!state.installed.contains_key(&x_key));
        assert!(state.installed.contains_key(&y_key));
        assert_eq!(state.remove_favorite(x), None);
    }
}
