//! Persisted data. Keys are camelCase and compatible with the prototype's `data.json`.
//! Never rename or remove a key; every field has a default.

use std::collections::{BTreeMap, HashMap};
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
    /// Where a team loadout is published: an https URL to its `.loadout.json`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
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
    /// Agent id → shell command that opens it, `{dir}` = the project folder. Overrides
    /// `launch::launcher`'s defaults.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub launchers: BTreeMap<String, String>,
    /// Which app icon variant the Dock and the app's own headers show.
    #[serde(deserialize_with = "or_default")]
    pub app_icon: AppIcon,
    /// Where Loadout's icon appears.
    #[serde(deserialize_with = "or_default")]
    pub icon_place: IconPlace,
    /// Open the Settings window when Loadout starts.
    pub show_window_on_launch: bool,
}

/// A value from a newer Loadout falls back to the default instead of failing all of data.json.
fn or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).unwrap_or_default())
}

/// App icon variants (Settings → Interface).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppIcon {
    #[default]
    Default,
    Twilight,
    Graphite,
}

impl AppIcon {
    pub const ALL: [AppIcon; 3] = [AppIcon::Default, AppIcon::Twilight, AppIcon::Graphite];

    pub fn label(self) -> &'static str {
        match self {
            AppIcon::Default => "Default",
            AppIcon::Twilight => "Twilight",
            AppIcon::Graphite => "Graphite",
        }
    }
}

/// Where Loadout's icon shows. Without the menu bar icon, ⌃⌥L and the Dock still reach it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IconPlace {
    #[default]
    MenuBar,
    DockAndMenuBar,
    Dock,
}

impl IconPlace {
    pub const ALL: [IconPlace; 3] = [
        IconPlace::MenuBar,
        IconPlace::DockAndMenuBar,
        IconPlace::Dock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IconPlace::MenuBar => "Menu bar",
            IconPlace::DockAndMenuBar => "Dock and menu bar",
            IconPlace::Dock => "Dock",
        }
    }

    pub fn in_dock(self) -> bool {
        self != IconPlace::MenuBar
    }

    pub fn in_menu_bar(self) -> bool {
        self != IconPlace::Dock
    }
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
            launchers: BTreeMap::new(),
            app_icon: AppIcon::Default,
            icon_place: IconPlace::MenuBar,
            show_window_on_launch: false,
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
    /// Pin / ignore / notified, keyed by `skill_key`.
    pub skill_state: BTreeMap<String, SkillState>,
    /// When the last digest ran (unix seconds), whether or not it notified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_digest: Option<u64>,
}

/// Choices about one skill's updates.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SkillState {
    /// Stay on the installed version: no update is surfaced.
    pub pinned: bool,
    /// This latest version was dismissed; a newer one surfaces again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored_version: Option<String>,
    /// The latest version a notification already mentioned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notified_version: Option<String>,
}

/// `skill_state` key: `source` or `source@skill`, so it survives removing and re-adding a favorite.
pub fn skill_key(favorite: &Favorite) -> String {
    let source = favorite.source.trim();
    match favorite
        .skill
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(skill) => format!("{source}@{skill}"),
        None => source.to_string(),
    }
}

/// A recorded install whose skill has a newer version on GitHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// `None` = global.
    pub project: Option<Uuid>,
    pub favorite: Uuid,
    pub installed: String,
    pub latest: String,
}

/// `installed` key → (project or global, favorite).
pub fn parse_installed_key(key: &str) -> Option<(Option<Uuid>, Uuid)> {
    let (scope, favorite) = key.split_once('|')?;
    let project = match scope {
        "global" => None,
        id => Some(Uuid::parse_str(id).ok()?),
    };
    Some((project, Uuid::parse_str(favorite).ok()?))
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoadoutError {
    #[error("Give the loadout a name.")]
    EmptyName,
    #[error("There's already a loadout called {0}.")]
    NameTaken(String),
    #[error("Select at least one skill for the loadout.")]
    NoSkills,
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

    /// Forgets an install record (after `skills remove`).
    pub fn mark_removed(&mut self, project: Option<Uuid>, favorite: Uuid) -> bool {
        self.installed
            .remove(&installed_key(project, favorite))
            .is_some()
    }

    /// Sets the version after an update, replacing whatever was recorded.
    pub fn set_installed_version(&mut self, project: Option<Uuid>, favorite: Uuid, version: &str) {
        self.installed
            .insert(installed_key(project, favorite), version.to_string());
    }

    /// Recorded installs: (project or global, favorite, version).
    fn installs(&self) -> impl Iterator<Item = (Option<Uuid>, Uuid, &str)> {
        self.installed.iter().filter_map(|(key, version)| {
            let (project, favorite) = parse_installed_key(key)?;
            Some((project, favorite, version.as_str()))
        })
    }

    /// Whether Loadout ever installed this skill anywhere (records or provenance): a first
    /// install gets a safety glance.
    pub fn ever_installed(&self, favorite: Uuid) -> bool {
        self.installs().any(|(_, id, _)| id == favorite)
            || self
                .favorites
                .iter()
                .find(|f| f.id == favorite)
                .is_some_and(|f| !crate::provenance::for_skill(&self.provenance, f).is_empty())
    }

    /// Favorites with at least one install whose version is known: the ones worth checking.
    pub fn tracked_favorites(&self) -> Vec<Favorite> {
        self.favorites
            .iter()
            .filter(|f| self.installs().any(|(_, id, v)| id == f.id && v != "?"))
            .cloned()
            .collect()
    }

    /// Recorded installs that are behind `latest`, minus pinned skills and ignored versions.
    /// Installs in removed projects don't count. Ordered by favorite, then by key.
    pub fn pending_updates(&self, latest: &HashMap<Uuid, String>) -> Vec<Pending> {
        let mut pending = Vec::new();
        for favorite in &self.favorites {
            let Some(newest) = latest.get(&favorite.id) else {
                continue;
            };
            let state = self.skill_state.get(&skill_key(favorite));
            if state.is_some_and(|s| s.pinned || s.ignored_version.as_ref() == Some(newest)) {
                continue;
            }
            for (project, id, version) in self.installs() {
                let project_known = project.is_none_or(|p| self.projects.iter().any(|x| x.id == p));
                if id == favorite.id && version != "?" && version != newest && project_known {
                    pending.push(Pending {
                        project,
                        favorite: id,
                        installed: version.to_string(),
                        latest: newest.clone(),
                    });
                }
            }
        }
        pending
    }

    /// Favorites in `pending` whose latest version wasn't announced yet; marks them announced.
    pub fn take_unnotified(&mut self, pending: &[Pending]) -> Vec<Uuid> {
        let mut fresh = Vec::new();
        for p in pending {
            let Some(favorite) = self.favorites.iter().find(|f| f.id == p.favorite) else {
                continue;
            };
            let state = self.skill_state.entry(skill_key(favorite)).or_default();
            if state.notified_version.as_ref() != Some(&p.latest) {
                state.notified_version = Some(p.latest.clone());
                fresh.push(p.favorite);
            }
        }
        fresh
    }

    pub fn skill_state(&self, favorite: Uuid) -> SkillState {
        self.favorites
            .iter()
            .find(|f| f.id == favorite)
            .and_then(|f| self.skill_state.get(&skill_key(f)).cloned())
            .unwrap_or_default()
    }

    fn skill_state_mut(&mut self, favorite: Uuid) -> Option<&mut SkillState> {
        let key = skill_key(self.favorites.iter().find(|f| f.id == favorite)?);
        Some(self.skill_state.entry(key).or_default())
    }

    pub fn set_pinned(&mut self, favorite: Uuid, pinned: bool) -> bool {
        self.skill_state_mut(favorite)
            .map(|s| s.pinned = pinned)
            .is_some()
    }

    /// Hides `version` of this skill; a later version shows again.
    pub fn ignore_update(&mut self, favorite: Uuid, version: &str) -> bool {
        self.skill_state_mut(favorite)
            .map(|s| s.ignored_version = Some(version.to_string()))
            .is_some()
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

    /// A new loadout of the existing favorites in `favorite_ids` (duplicates dropped, order kept).
    pub fn create_loadout(
        &mut self,
        name: &str,
        favorite_ids: &[Uuid],
        tags: Vec<String>,
    ) -> Result<Uuid, LoadoutError> {
        let name = self.free_loadout_name(name, None)?;
        let mut ids: Vec<Uuid> = Vec::new();
        for id in favorite_ids {
            if !ids.contains(id) && self.favorites.iter().any(|f| f.id == *id) {
                ids.push(*id);
            }
        }
        if ids.is_empty() {
            return Err(LoadoutError::NoSkills);
        }
        let loadout = Loadout {
            id: Uuid::new_v4(),
            name,
            favorite_ids: ids,
            tags,
            ..Default::default()
        };
        let id = loadout.id;
        self.loadouts.push(loadout);
        Ok(id)
    }

    /// `name` trimmed, if no other loadout (than `except`) has it.
    fn free_loadout_name(&self, name: &str, except: Option<Uuid>) -> Result<String, LoadoutError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LoadoutError::EmptyName);
        }
        match self
            .loadouts
            .iter()
            .find(|l| Some(l.id) != except && l.name.eq_ignore_ascii_case(name))
        {
            Some(taken) => Err(LoadoutError::NameTaken(taken.name.clone())),
            None => Ok(name.to_string()),
        }
    }

    pub fn loadout(&self, id: Uuid) -> Option<&Loadout> {
        self.loadouts.iter().find(|l| l.id == id)
    }

    fn loadout_mut(&mut self, id: Uuid) -> Option<&mut Loadout> {
        self.loadouts.iter_mut().find(|l| l.id == id)
    }

    /// Names are unique, case-insensitively.
    pub fn rename_loadout(&mut self, id: Uuid, name: &str) -> Result<(), LoadoutError> {
        let name = self.free_loadout_name(name, Some(id))?;
        if let Some(loadout) = self.loadout_mut(id) {
            loadout.name = name;
        }
        Ok(())
    }

    pub fn set_loadout_tags(&mut self, id: Uuid, tags: Vec<String>) -> bool {
        self.loadout_mut(id).map(|l| l.tags = tags).is_some()
    }

    /// `None` clears it. The id isn't checked here; see `agents::resolve`.
    pub fn set_loadout_agent(&mut self, id: Uuid, agent: Option<String>) -> bool {
        self.loadout_mut(id)
            .map(|l| l.preferred_agent = agent)
            .is_some()
    }

    /// Adds or removes one favorite; a loadout keeps at least one.
    pub fn toggle_loadout_favorite(
        &mut self,
        id: Uuid,
        favorite: Uuid,
    ) -> Result<(), LoadoutError> {
        let Some(loadout) = self.loadout_mut(id) else {
            return Ok(());
        };
        match loadout.favorite_ids.iter().position(|f| *f == favorite) {
            Some(_) if loadout.favorite_ids.len() == 1 => return Err(LoadoutError::NoSkills),
            Some(index) => {
                loadout.favorite_ids.remove(index);
            }
            None => loadout.favorite_ids.push(favorite),
        }
        Ok(())
    }

    pub fn remove_loadout(&mut self, id: Uuid) -> Option<Loadout> {
        let index = self.loadouts.iter().position(|l| l.id == id)?;
        Some(self.loadouts.remove(index))
    }

    /// The loadout's favorites that still exist, in its order.
    pub fn loadout_favorites(&self, id: Uuid) -> Vec<&Favorite> {
        self.loadout(id).map_or_else(Vec::new, |loadout| {
            loadout
                .favorite_ids
                .iter()
                .filter_map(|fav| self.favorites.iter().find(|f| f.id == *fav))
                .collect()
        })
    }

    /// Tags a loadout is suggested for: its own, or else the union of its skills' tags.
    pub fn loadout_tags(&self, id: Uuid) -> Vec<String> {
        let Some(loadout) = self.loadout(id) else {
            return Vec::new();
        };
        if !loadout.tags.is_empty() {
            return loadout.tags.clone();
        }
        let mut tags: Vec<String> = Vec::new();
        for favorite in self.loadout_favorites(id) {
            let own = if favorite.tags.is_empty() {
                crate::tags::suggest_tags(favorite)
            } else {
                favorite.tags.clone()
            };
            for tag in own {
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }
        tags
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

    fn updates_fixture() -> (SavedState, Uuid, Uuid, Uuid, Uuid) {
        let mut state = SavedState::default();
        state.add_favorites(parse(
            "npx skills add anthropics/skills --skill frontend-design --skill pdf",
        ));
        state.add_favorites(parse("mattpocock/skills@tdd"));
        let (fd, pdf, tdd) = (
            state.favorites[0].id,
            state.favorites[1].id,
            state.favorites[2].id,
        );
        let shop = state.project_id(std::path::Path::new("/Code/shop"));
        state.mark_installed(Some(shop), fd, Some("old-fd"));
        state.mark_installed(None, fd, Some("old-fd"));
        state.mark_installed(Some(shop), pdf, None); // unknown version: never an update
        state.mark_installed(Some(shop), tdd, Some("same"));
        (state, fd, pdf, tdd, shop)
    }

    fn loadout_fixture() -> (SavedState, Uuid, Uuid, Uuid) {
        let mut state = SavedState::default();
        state.add_favorites(parse(
            "npx skills add anthropics/skills --skill frontend-design --skill pdf",
        ));
        state.add_favorites(parse("mattpocock/skills@tdd"));
        let ids = (
            state.favorites[0].id,
            state.favorites[1].id,
            state.favorites[2].id,
        );
        (state, ids.0, ids.1, ids.2)
    }

    #[test]
    fn create_loadout_validates() {
        let (mut state, fd, _, tdd) = loadout_fixture();
        let ghost = Uuid::new_v4();
        let id = state
            .create_loadout("  Web app ", &[fd, tdd, fd, ghost], vec!["nextjs".into()])
            .unwrap();
        let loadout = state.loadout(id).unwrap();
        assert_eq!(loadout.name, "Web app");
        assert_eq!(loadout.favorite_ids, [fd, tdd]);
        assert_eq!(loadout.tags, ["nextjs"]);
        assert_eq!(
            state.create_loadout("web APP", &[fd], vec![]),
            Err(LoadoutError::NameTaken("Web app".into()))
        );
        assert_eq!(
            state.create_loadout(" ", &[fd], vec![]),
            Err(LoadoutError::EmptyName)
        );
        assert_eq!(
            state.create_loadout("Empty", &[ghost], vec![]),
            Err(LoadoutError::NoSkills)
        );
        assert_eq!(state.loadouts.len(), 1);
    }

    #[test]
    fn edit_loadouts() {
        let (mut state, fd, pdf, tdd) = loadout_fixture();
        let web = state.create_loadout("Web app", &[fd], vec![]).unwrap();
        let docs = state.create_loadout("Docs", &[pdf], vec![]).unwrap();
        assert_eq!(
            state.rename_loadout(docs, "web app"),
            Err(LoadoutError::NameTaken("Web app".into()))
        );
        assert_eq!(state.rename_loadout(web, "Web App"), Ok(()));
        assert_eq!(state.rename_loadout(web, ""), Err(LoadoutError::EmptyName));
        assert_eq!(state.loadout(web).unwrap().name, "Web App");

        assert_eq!(state.toggle_loadout_favorite(web, tdd), Ok(()));
        assert_eq!(state.loadout(web).unwrap().favorite_ids, [fd, tdd]);
        assert_eq!(state.toggle_loadout_favorite(web, fd), Ok(()));
        assert_eq!(
            state.toggle_loadout_favorite(web, tdd),
            Err(LoadoutError::NoSkills)
        );
        assert_eq!(state.loadout(web).unwrap().favorite_ids, [tdd]);

        assert!(state.set_loadout_agent(web, Some("pi".into())));
        assert_eq!(
            state.loadout(web).unwrap().preferred_agent.as_deref(),
            Some("pi")
        );
        assert_eq!(
            state.remove_loadout(docs).map(|l| l.name),
            Some("Docs".into())
        );
        assert!(state.loadout(docs).is_none());
    }

    #[test]
    fn loadout_members_and_tags() {
        let (mut state, fd, pdf, tdd) = loadout_fixture();
        let web = state
            .create_loadout("Web app", &[tdd, fd, pdf], vec![])
            .unwrap();
        state.remove_favorite(pdf);
        let names: Vec<_> = state
            .loadout_favorites(web)
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, ["tdd", "frontend-design"]);
        // Untagged: the union of its skills' tags, in order.
        assert_eq!(state.loadout_tags(web), ["testing", "frontend"]);
        state.set_loadout_tags(web, vec!["nextjs".into()]);
        assert_eq!(state.loadout_tags(web), ["nextjs"]);
        assert!(state.loadout_tags(Uuid::new_v4()).is_empty());
    }

    #[test]
    fn interface_settings_default_and_survive_unknown_values() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            (settings.app_icon, settings.icon_place),
            (AppIcon::Default, IconPlace::MenuBar)
        );
        assert!(!settings.show_window_on_launch);
        let json =
            r#"{"appIcon": "twilight", "iconPlace": "dockAndMenuBar", "showWindowOnLaunch": true}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(
            (settings.app_icon, settings.icon_place),
            (AppIcon::Twilight, IconPlace::DockAndMenuBar)
        );
        assert!(settings.show_window_on_launch);
        // A value from a newer Loadout falls back instead of failing the whole file.
        let settings: Settings =
            serde_json::from_str(r#"{"appIcon": "neon", "iconPlace": "touchBar"}"#).unwrap();
        assert_eq!(
            (settings.app_icon, settings.icon_place),
            (AppIcon::Default, IconPlace::MenuBar)
        );
        assert!(IconPlace::Dock.in_dock() && !IconPlace::Dock.in_menu_bar());
    }

    #[test]
    fn ever_installed_uses_records_and_provenance() {
        let mut state = SavedState::default();
        state.add_favorites(parse(
            "npx skills add anthropics/skills --skill pdf --skill frontend-design",
        ));
        let (pdf, fd) = (state.favorites[0].id, state.favorites[1].id);
        assert!(!state.ever_installed(pdf));
        state.mark_installed(None, pdf, None);
        assert!(state.ever_installed(pdf));
        // Removed from every project since, but the log remembers.
        let favorite = state.favorites[1].clone();
        crate::provenance::append(
            &mut state,
            crate::provenance::entry("install", None, &favorite),
        );
        assert!(state.ever_installed(fd));
        assert!(!state.ever_installed(Uuid::new_v4()));
    }

    #[test]
    fn skill_keys_and_installed_keys_round_trip() {
        assert_eq!(
            skill_key(&favorite("x", "anthropics/skills", Some("pdf"))),
            "anthropics/skills@pdf"
        );
        assert_eq!(
            skill_key(&favorite("x", " anthropics/skills ", None)),
            "anthropics/skills"
        );
        assert_eq!(skill_key(&favorite("x", "a/b", Some(""))), "a/b");
        let (p, f) = (Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(
            parse_installed_key(&installed_key(Some(p), f)),
            Some((Some(p), f))
        );
        assert_eq!(
            parse_installed_key(&installed_key(None, f)),
            Some((None, f))
        );
        assert_eq!(parse_installed_key("nonsense"), None);
    }

    #[test]
    fn pending_updates_compare_known_versions() {
        let (state, fd, pdf, tdd, shop) = updates_fixture();
        let latest = HashMap::from([
            (fd, "new-fd".to_string()),
            (pdf, "x".into()),
            (tdd, "same".into()),
        ]);
        let pending = state.pending_updates(&latest);
        let mut got: Vec<_> = pending
            .iter()
            .map(|p| (p.project, p.favorite, p.installed.as_str()))
            .collect();
        got.sort_by_key(|(p, _, _)| p.is_some());
        assert_eq!(got, [(None, fd, "old-fd"), (Some(shop), fd, "old-fd")]);
        assert!(pending.iter().all(|p| p.latest == "new-fd"));
        assert!(state.pending_updates(&HashMap::new()).is_empty());
    }

    #[test]
    fn pin_and_ignore_hide_updates() {
        let (mut state, fd, _, _, _) = updates_fixture();
        let latest = HashMap::from([(fd, "v2".to_string())]);
        assert!(state.set_pinned(fd, true));
        assert!(state.pending_updates(&latest).is_empty());
        assert!(state.set_pinned(fd, false));
        assert!(state.ignore_update(fd, "v2"));
        assert!(state.pending_updates(&latest).is_empty());
        // A newer version shows again.
        assert_eq!(
            state
                .pending_updates(&HashMap::from([(fd, "v3".to_string())]))
                .len(),
            2
        );
        assert!(!state.set_pinned(Uuid::new_v4(), true));
        // Persisted by skill, so it survives remove + re-add.
        let key = skill_key(&state.favorites[0]);
        assert_eq!(
            state.skill_state[&key].ignored_version.as_deref(),
            Some("v2")
        );
        state.remove_favorite(fd);
        state.add_favorites(parse("anthropics/skills@frontend-design"));
        let again = state.favorites.last().unwrap().id;
        assert_eq!(
            state.skill_state(again).ignored_version.as_deref(),
            Some("v2")
        );
    }

    #[test]
    fn removed_projects_dont_count() {
        let (mut state, fd, _, _, shop) = updates_fixture();
        state.projects.retain(|p| p.id != shop);
        let pending = state.pending_updates(&HashMap::from([(fd, "v2".to_string())]));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].project, None);
    }

    #[test]
    fn notifies_each_version_once() {
        let (mut state, fd, _, _, _) = updates_fixture();
        let pending = state.pending_updates(&HashMap::from([(fd, "v2".to_string())]));
        assert_eq!(state.take_unnotified(&pending), [fd]);
        assert!(state.take_unnotified(&pending).is_empty());
        let newer = state.pending_updates(&HashMap::from([(fd, "v3".to_string())]));
        assert_eq!(state.take_unnotified(&newer), [fd]);
    }

    #[test]
    fn tracked_and_updated_versions() {
        let (mut state, fd, pdf, tdd, shop) = updates_fixture();
        let tracked: Vec<_> = state.tracked_favorites().iter().map(|f| f.id).collect();
        assert_eq!(tracked, [fd, tdd]);
        state.set_installed_version(Some(shop), pdf, "v1");
        assert_eq!(state.installed[&installed_key(Some(shop), pdf)], "v1");
        assert!(state.tracked_favorites().iter().any(|f| f.id == pdf));
    }

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
    fn mark_removed_drops_one_record() {
        let mut state = SavedState::default();
        let shop = state.project_id(std::path::Path::new("/Code/shop"));
        let fav = Uuid::new_v4();
        state.mark_installed(Some(shop), fav, Some("v1"));
        state.mark_installed(None, fav, Some("v1"));
        assert!(state.mark_removed(Some(shop), fav));
        assert!(!state.is_installed(Some(shop), fav));
        assert!(state.is_installed(None, fav));
        assert!(!state.mark_removed(Some(shop), fav));
        // The skill was installed once, so it isn't a first install again.
        assert!(state.ever_installed(fav));
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
