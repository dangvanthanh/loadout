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
