//! `data.json`: atomic saves, prototype import, corrupt-file recovery.

use std::path::{Path, PathBuf};

use crate::model::SavedState;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("couldn't read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("couldn't write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, PartialEq)]
pub enum LoadNote {
    /// Nothing saved yet; state came from the SkillDock prototype's file.
    ImportedPrototype(PathBuf),
    /// The file didn't parse; it was moved aside and Loadout starts empty.
    RecoveredCorrupt { backup: PathBuf },
}

#[derive(Debug)]
pub struct Loaded {
    pub state: SavedState,
    pub note: Option<LoadNote>,
}

/// `~/Library/Application Support/Loadout/data.json`
pub fn data_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("Loadout").join("data.json"))
}

/// `~/Library/Application Support/SkillDock/data.json`
pub fn prototype_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("SkillDock").join("data.json"))
}

/// Reads `path`. With no file yet, imports `prototype` if it exists and parses.
/// A file that doesn't parse is renamed to `data.json.corrupt-<unix secs>`, never overwritten.
pub fn load(path: &Path, prototype: Option<&Path>) -> Result<Loaded, StorageError> {
    if let Some(text) = read_if_exists(path)? {
        return match serde_json::from_str(&text) {
            Ok(state) => Ok(Loaded { state, note: None }),
            Err(_) => {
                let backup = corrupt_backup_path(path);
                std::fs::rename(path, &backup).map_err(|source| StorageError::Write {
                    path: backup.clone(),
                    source,
                })?;
                Ok(Loaded {
                    state: SavedState::default(),
                    note: Some(LoadNote::RecoveredCorrupt { backup }),
                })
            }
        };
    }
    if let Some(prototype) = prototype
        && let Some(text) = read_if_exists(prototype)?
        && let Ok(state) = serde_json::from_str(&text)
    {
        return Ok(Loaded {
            state,
            note: Some(LoadNote::ImportedPrototype(prototype.to_path_buf())),
        });
    }
    Ok(Loaded {
        state: SavedState::default(),
        note: None,
    })
}

/// Writes a sibling temp file, syncs it, then renames it over `path`.
pub fn save(path: &Path, state: &SavedState) -> Result<(), StorageError> {
    let write_err = |source| StorageError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(write_err)?;
    }
    // Serializing plain structs with string keys can't fail.
    let json = serde_json::to_string_pretty(state).map_err(|err| write_err(err.into()))?;
    let tmp = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp).map_err(write_err)?;
    std::io::Write::write_all(&mut file, json.as_bytes()).map_err(write_err)?;
    file.sync_all().map_err(write_err)?;
    std::fs::rename(&tmp, path).map_err(write_err)
}

fn read_if_exists(path: &Path) -> Result<Option<String>, StorageError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(StorageError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn corrupt_backup_path(path: &Path) -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".corrupt-{secs}"));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Favorite, Loadout, ProvenanceEntry};
    use uuid::Uuid;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "loadout-storage-{name}-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/skilldock.json")
    }

    #[test]
    fn storage_missing_file_is_default() {
        let dir = temp_dir("missing");
        let loaded = load(&dir.join("data.json"), None).unwrap();
        assert_eq!(loaded.state, SavedState::default());
        assert_eq!(loaded.note, None);
    }

    #[test]
    fn storage_empty_object_uses_defaults() {
        let dir = temp_dir("empty");
        let path = dir.join("data.json");
        std::fs::write(&path, "{}").unwrap();
        let loaded = load(&path, None).unwrap();
        assert_eq!(loaded.state, SavedState::default());
        assert_eq!(loaded.state.settings.agents, "claude-code");
        assert!(loaded.state.settings.auto_check);
        assert_eq!(loaded.state.settings.hotkey, "ctrl+alt+KeyL");
        assert_eq!(loaded.state.settings.digest_interval_hours, 24);
    }

    #[test]
    fn storage_reads_prototype_file() {
        let dir = temp_dir("prototype");
        let loaded = load(&dir.join("data.json"), Some(&fixture())).unwrap();
        assert_eq!(loaded.note, Some(LoadNote::ImportedPrototype(fixture())));
        let state = loaded.state;

        assert_eq!(state.favorites.len(), 2);
        assert_eq!(state.favorites[0].source, "anthropics/skills");
        assert_eq!(state.favorites[0].skill.as_deref(), Some("frontend-design"));
        assert_eq!(state.favorites[1].skill, None);
        assert!(state.favorites[1].tags.is_empty());
        assert_eq!(
            state.projects[0].path,
            PathBuf::from("/Users/me/Code/shop-dashboard")
        );
        assert_eq!(state.selected_project_id, Some(state.projects[0].id));
        assert_eq!(state.settings.agents, "claude-code,cursor");
        assert!(!state.settings.auto_check);
        assert_eq!(state.settings.hotkey, "ctrl+alt+KeyL");
        assert_eq!(state.installed.len(), 2);
        assert_eq!(
            state.installed["global|0B9F5C1E-2D3A-4F6B-8C7D-9E0F1A2B3C4D"],
            "?"
        );
    }

    #[test]
    fn storage_own_file_wins_over_prototype() {
        let dir = temp_dir("own");
        let path = dir.join("data.json");
        std::fs::write(&path, r#"{"favorites":[]}"#).unwrap();
        let loaded = load(&path, Some(&fixture())).unwrap();
        assert!(loaded.state.favorites.is_empty());
        assert_eq!(loaded.note, None);
    }

    #[test]
    fn storage_round_trip() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("nested").join("data.json");
        let mut state = load(&dir.join("none.json"), Some(&fixture()))
            .unwrap()
            .state;
        state.favorites.push(Favorite {
            id: Uuid::new_v4(),
            name: "tdd".into(),
            source: "mattpocock/skills".into(),
            skill: Some("tdd".into()),
            tags: vec!["testing".into()],
        });
        state.loadouts.push(Loadout {
            id: Uuid::new_v4(),
            name: "Web app".into(),
            favorite_ids: vec![state.favorites[0].id],
            tags: vec!["nextjs".into()],
            preferred_agent: Some("pi".into()),
            version: None,
        });
        state.provenance.push(ProvenanceEntry {
            id: Uuid::new_v4(),
            ts: 1_790_000_000,
            action: "install".into(),
            project_path: Some("/Users/me/Code/shop-dashboard".into()),
            skill_source: "anthropics/skills".into(),
            skill_name: Some("frontend-design".into()),
            ..Default::default()
        });

        save(&path, &state).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"selectedProjectID\""));
        assert!(text.contains("\"autoCheck\""));
        assert!(text.contains("\"preferredAgent\": \"pi\""));
        assert!(!path.with_extension("json.tmp").exists());

        let again = load(&path, None).unwrap();
        assert_eq!(again.state, state);
        assert_eq!(again.note, None);
    }

    #[test]
    fn storage_corrupt_file_is_moved_aside() {
        let dir = temp_dir("corrupt");
        let path = dir.join("data.json");
        std::fs::write(&path, "{\"favorites\": [oops").unwrap();

        let loaded = load(&path, Some(&fixture())).unwrap();
        assert_eq!(loaded.state, SavedState::default());
        let Some(LoadNote::RecoveredCorrupt { backup }) = loaded.note else {
            panic!("expected recovery note, got {:?}", loaded.note);
        };
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "{\"favorites\": [oops"
        );
    }
}
