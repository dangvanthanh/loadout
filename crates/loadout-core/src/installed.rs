//! Skills already in a project, whoever installed them: `skills-lock.json` plus the agents'
//! skill folders (a subfolder with a SKILL.md).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::agents;
use crate::model::Favorite;
use crate::updates;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Installed {
    /// Normalized skill name → source from the lock file, if listed there.
    pub skills: BTreeMap<String, Option<String>>,
}

#[derive(Deserialize)]
struct Lock {
    #[serde(default)]
    skills: BTreeMap<String, LockEntry>,
}

#[derive(Deserialize)]
struct LockEntry {
    #[serde(default)]
    source: Option<String>,
}

/// Reads `root` for these agents (all known agents when the list is empty).
pub fn read(root: &Path, agent_ids: &[String]) -> Installed {
    let mut skills = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(root.join("skills-lock.json"))
        && let Ok(lock) = serde_json::from_str::<Lock>(&text)
    {
        for (name, entry) in lock.skills {
            skills.insert(updates::normalize(&name), entry.source);
        }
    }
    let dirs = if agent_ids.is_empty() {
        let mut all: Vec<&str> = agents::AGENTS.iter().map(|a| a.project_dir).collect();
        all.sort();
        all.dedup();
        all
    } else {
        agents::project_dirs(agent_ids)
    };
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().join("SKILL.md").is_file() {
                let name = updates::normalize(&entry.file_name().to_string_lossy());
                skills.entry(name).or_insert(None);
            }
        }
    }
    Installed { skills }
}

/// "owner/repo", lowercased, for GitHub sources.
fn repo_key(source: &str) -> Option<String> {
    let skill_ref = updates::source_ref(source)?;
    Some(format!("{}/{}", skill_ref.repo.owner, skill_ref.repo.name).to_lowercase())
}

/// The names `skills remove` needs for `favorite`: what the project actually has for it,
/// else its own skill name (installs Loadout recorded but can't see, e.g. global ones).
pub fn names_to_remove(installed: &Installed, favorite: &Favorite) -> Vec<String> {
    let found: Vec<String> = installed
        .matching(favorite)
        .into_iter()
        .map(str::to_string)
        .collect();
    if !found.is_empty() {
        return found;
    }
    let own = favorite.skill.as_deref().filter(|s| !s.trim().is_empty());
    own.map(updates::normalize).into_iter().collect()
}

impl Installed {
    pub fn contains(&self, favorite: &Favorite) -> bool {
        !self.matching(favorite).is_empty()
    }

    /// The installed skill names `favorite` accounts for: its own skill, or for a whole source,
    /// every skill the lock file attributes to that repo.
    pub fn matching(&self, favorite: &Favorite) -> Vec<&str> {
        let wanted = repo_key(&favorite.source);
        let same_source = |source: &str| match (repo_key(source), &wanted) {
            (Some(a), Some(b)) => a == *b,
            _ => source.eq_ignore_ascii_case(favorite.source.trim()),
        };
        let folder = updates::skill_ref(favorite).and_then(|r| r.folder);
        let name = favorite
            .skill
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| folder.as_deref().and_then(|f| f.rsplit('/').next()));
        match name {
            // A folder without a lock entry could be from anywhere; the name decides.
            Some(name) => self
                .skills
                .get_key_value(&updates::normalize(name))
                .filter(|(_, source)| source.as_deref().is_none_or(same_source))
                .map(|(name, _)| name.as_str())
                .into_iter()
                .collect(),
            None => self
                .skills
                .iter()
                .filter(|(_, source)| source.as_deref().is_some_and(same_source))
                .map(|(name, _)| name.as_str())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::favorite;

    fn fixture() -> Installed {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/installed");
        read(&root, &agents::parse_list("claude-code, codex"))
    }

    #[test]
    fn reads_lock_and_agent_folders() {
        let found = fixture();
        assert_eq!(
            found.skills,
            BTreeMap::from([
                (
                    "frontend-design".to_string(),
                    Some("anthropics/skills".to_string())
                ),
                ("tdd".to_string(), None),
            ])
        );
    }

    #[test]
    fn only_configured_agents_folders_count() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/installed");
        // Windsurf uses .windsurf/skills, so tdd (in .agents/skills) isn't installed for it.
        let found = read(&root, &agents::parse_list("windsurf"));
        assert!(!found.skills.contains_key("tdd"));
        // The lock file still counts.
        assert!(found.skills.contains_key("frontend-design"));
        // No agents configured: every known folder counts.
        assert!(read(&root, &[]).skills.contains_key("tdd"));
    }

    #[test]
    fn matches_named_favorites() {
        let found = fixture();
        assert!(found.contains(&favorite(
            "frontend-design",
            "anthropics/skills",
            Some("frontend-design")
        )));
        assert!(found.contains(&favorite(
            "Frontend Design",
            "https://github.com/anthropics/skills",
            Some("Frontend Design")
        )));
        assert!(found.contains(&favorite("tdd", "mattpocock/skills", Some("tdd"))));
        assert!(!found.contains(&favorite("pdf", "anthropics/skills", Some("pdf"))));
        // Same name from another source isn't the same skill.
        assert!(!found.contains(&favorite(
            "frontend-design",
            "someone/else",
            Some("frontend-design")
        )));
    }

    #[test]
    fn matches_folder_urls_and_whole_sources() {
        let found = fixture();
        let url = "https://github.com/anthropics/skills/tree/main/skills/frontend-design";
        assert!(found.contains(&favorite("frontend-design", url, None)));
        assert!(found.contains(&favorite("skills", "anthropics/skills", None)));
        assert!(!found.contains(&favorite("skills", "mattpocock/skills", None)));
    }

    #[test]
    fn names_to_remove_prefer_what_is_there() {
        let found = fixture();
        let fd = favorite(
            "Frontend Design",
            "anthropics/skills",
            Some("Frontend Design"),
        );
        assert_eq!(names_to_remove(&found, &fd), ["frontend-design"]);
        let whole = favorite("skills", "anthropics/skills", None);
        assert_eq!(names_to_remove(&found, &whole), ["frontend-design"]);
        // Not visible here (a global install): its own name.
        let pdf = favorite("pdf", "anthropics/skills", Some("pdf"));
        assert_eq!(names_to_remove(&found, &pdf), ["pdf"]);
        // A whole source that isn't visible: nothing to name, so nothing to run.
        assert!(names_to_remove(&found, &favorite("x", "acme/kit", None)).is_empty());
    }

    #[test]
    fn missing_project_is_empty() {
        assert_eq!(
            read(Path::new("/nonexistent/loadout"), &[]),
            Installed::default()
        );
    }
}
