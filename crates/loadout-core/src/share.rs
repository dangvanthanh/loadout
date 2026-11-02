//! Sharing loadouts (U5): `.loadout.json` files and `loadout://import?data=…` links.
//! Imports come from anyone, so everything is validated before it touches `data.json`;
//! installing still goes through the palette, the safety glance, and the official CLI.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

use crate::model::{Favorite, Loadout, ProvenanceEntry, SavedState, Uuid};
use crate::{agents, parser, tags};

/// The file format version this Loadout writes and reads.
pub const FORMAT: u32 = 1;
const LINK_PREFIX: &str = "loadout://import?data=";
const MAX_BYTES: usize = 256 * 1024;
const MAX_SKILLS: usize = 100;
const MAX_NAME: usize = 80;
const MAX_FIELD: usize = 500;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedLoadout {
    /// Format version; also marks the JSON as a Loadout file.
    pub loadout: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Where the team publishes this file (https), for "Check for a newer version".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub skills: Vec<SharedSkill>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedSkill {
    pub name: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShareError {
    #[error("That isn't a Loadout file or link.")]
    NotALoadout,
    #[error("This loadout was made by a newer Loadout. Update Loadout to import it.")]
    NewerFormat,
    #[error("The loadout is too large to import.")]
    TooLarge,
    #[error("The loadout has no skills.")]
    NoSkills,
    #[error("The loadout has a skill Loadout can't install: {0}")]
    BadSkill(String),
    #[error("Loadout only fetches team loadouts over https.")]
    NotHttps,
    #[error("Couldn't load the team loadout: {0}")]
    Unreachable(String),
}

/// How an incoming loadout relates to the one you have with the same name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionChange {
    /// You have no loadout with this name.
    New,
    /// Yours is older (or unversioned), so this updates it.
    Newer,
    Same,
    /// Yours is newer: importing would go back to this version.
    Older {
        yours: String,
    },
    /// Neither side has a version.
    Unversioned,
}

/// "1.10" > "1.9", "v2" = "2"; non-numeric parts compare as text.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let parts = |v: &str| -> Vec<String> {
        let v = v.trim();
        let v = v.strip_prefix(['v', 'V']).unwrap_or(v);
        v.split(['.', '-', '+']).map(str::to_string).collect()
    };
    let (a, b) = (parts(a), parts(b));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).map_or("0", String::as_str),
            b.get(i).map_or("0", String::as_str),
        );
        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

/// Compares `shared` with your same-named loadout.
pub fn version_change(state: &SavedState, shared: &SharedLoadout) -> VersionChange {
    use std::cmp::Ordering;
    let Some(yours) = state
        .loadouts
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(&shared.name))
    else {
        return VersionChange::New;
    };
    match (yours.version.as_deref(), shared.version.as_deref()) {
        (None, None) => VersionChange::Unversioned,
        (None, Some(_)) => VersionChange::Newer,
        // An unversioned file replacing a versioned loadout goes back, in effect.
        (Some(v), None) => VersionChange::Older { yours: v.into() },
        (Some(v), Some(incoming)) => match compare_versions(incoming, v) {
            Ordering::Greater => VersionChange::Newer,
            Ordering::Equal => VersionChange::Same,
            Ordering::Less => VersionChange::Older { yours: v.into() },
        },
    }
}

/// The provenance entry for an import: version and source in the line.
pub fn provenance_entry(shared: &SharedLoadout, imported: &Imported) -> ProvenanceEntry {
    ProvenanceEntry {
        id: Uuid::new_v4(),
        ts: crate::provenance::now(),
        action: crate::provenance::IMPORT_LOADOUT.into(),
        skill_source: shared.source.clone().unwrap_or_else(|| shared.name.clone()),
        skill_name: Some(imported.name.clone()),
        loadout_id: Some(imported.loadout),
        note: shared.version.as_ref().map(|v| format!("version {v}")),
        ..Default::default()
    }
}

/// Fetches a team loadout from its https source (blocking) and validates it.
pub fn fetch(url: &str) -> Result<SharedLoadout, ShareError> {
    if !is_https(url) {
        return Err(ShareError::NotHttps);
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .build()
        .into();
    let text = agent
        .get(url)
        .header("User-Agent", "Loadout")
        .call()
        .and_then(|mut r| {
            r.body_mut()
                .with_config()
                .limit(MAX_BYTES as u64)
                .read_to_string()
        })
        .map_err(|e| ShareError::Unreachable(e.to_string()))?;
    parse(&text)
}

/// A source a team loadout may be fetched from: plain https, no spaces.
pub fn is_https(url: &str) -> bool {
    url.strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.contains(char::is_whitespace))
        && !has_control(url)
        && url.len() <= MAX_FIELD
}

#[derive(Debug, PartialEq, Eq)]
pub struct Imported {
    pub loadout: Uuid,
    pub name: String,
    /// Favorites created for skills you didn't have.
    pub new_favorites: usize,
    /// A loadout with this name existed and was updated.
    pub replaced: bool,
}

/// The loadout with its skills' sources, for another machine.
pub fn export(state: &SavedState, id: Uuid) -> Option<SharedLoadout> {
    let loadout = state.loadout(id)?;
    Some(SharedLoadout {
        loadout: FORMAT,
        name: loadout.name.clone(),
        tags: loadout.tags.clone(),
        preferred_agent: loadout.preferred_agent.clone(),
        version: loadout.version.clone(),
        source: loadout.source.clone(),
        skills: state
            .loadout_favorites(id)
            .into_iter()
            .map(|f| SharedSkill {
                name: f.name.clone(),
                source: f.source.clone(),
                skill: f.skill.clone(),
                tags: f.tags.clone(),
            })
            .collect(),
    })
}

pub fn to_json(shared: &SharedLoadout) -> String {
    serde_json::to_string_pretty(shared).unwrap_or_default()
}

/// `loadout://import?data=<base64url of the JSON>`.
pub fn to_link(shared: &SharedLoadout) -> String {
    let json = serde_json::to_string(shared).unwrap_or_default();
    format!("{LINK_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
}

/// A pasted or opened link, or file contents. Validated.
pub fn parse(text: &str) -> Result<SharedLoadout, ShareError> {
    if text.len() > MAX_BYTES {
        return Err(ShareError::TooLarge);
    }
    let text = text.trim();
    let json = match text.strip_prefix(LINK_PREFIX) {
        Some(data) => {
            let bytes = URL_SAFE_NO_PAD
                .decode(data.trim_end_matches('='))
                .map_err(|_| ShareError::NotALoadout)?;
            String::from_utf8(bytes).map_err(|_| ShareError::NotALoadout)?
        }
        None => text.to_string(),
    };
    let mut shared: SharedLoadout =
        serde_json::from_str(&json).map_err(|_| ShareError::NotALoadout)?;
    if shared.loadout > FORMAT {
        return Err(ShareError::NewerFormat);
    }
    shared.name = shared.name.trim().to_string();
    if shared.loadout == 0 || shared.name.is_empty() || shared.name.chars().count() > MAX_NAME {
        return Err(ShareError::NotALoadout);
    }
    if has_control(&shared.name) {
        return Err(ShareError::NotALoadout);
    }
    if shared.skills.is_empty() {
        return Err(ShareError::NoSkills);
    }
    if shared.skills.len() > MAX_SKILLS {
        return Err(ShareError::TooLarge);
    }
    for skill in &mut shared.skills {
        clean_skill(skill)?;
    }
    shared.tags = tags::parse_tags(&shared.tags.join(","));
    shared.preferred_agent = shared
        .preferred_agent
        .as_deref()
        .and_then(agents::resolve)
        .map(|a| a.id.to_string());
    shared.version = shared
        .version
        .map(|v| v.trim().chars().take(MAX_NAME).collect::<String>())
        .filter(|v| !v.is_empty() && !has_control(v));
    // Optional metadata: a source that isn't plain https is dropped, not fatal.
    shared.source = shared
        .source
        .map(|s| s.trim().to_string())
        .filter(|s| is_https(s));
    Ok(shared)
}

fn has_control(text: &str) -> bool {
    text.chars().any(char::is_control)
}

/// Trims and checks one skill: plain text, sane length, and a source the paste parser knows
/// (owner/repo, GitHub/GitLab/skills.sh URLs, git@) — never a local path or a command.
fn clean_skill(skill: &mut SharedSkill) -> Result<(), ShareError> {
    let bad = |skill: &SharedSkill| ShareError::BadSkill(skill.source.clone());
    skill.source = skill.source.trim().to_string();
    skill.name = skill.name.trim().to_string();
    skill.skill = skill
        .skill
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let texts = [
        skill.name.as_str(),
        skill.source.as_str(),
        skill.skill.as_deref().unwrap_or(""),
    ];
    if texts
        .iter()
        .any(|t| has_control(t) || t.chars().count() > MAX_FIELD)
        || skill.tags.iter().any(|t| has_control(t))
    {
        return Err(bad(skill));
    }
    let known =
        !skill.source.contains(char::is_whitespace) && !parser::parse(&skill.source).is_empty();
    if !known {
        return Err(bad(skill));
    }
    if skill.name.is_empty() {
        skill.name = skill.skill.clone().unwrap_or_else(|| skill.source.clone());
    }
    skill.tags = tags::parse_tags(&skill.tags.join(","));
    Ok(())
}

/// Adds the skills you don't have as favorites and creates the loadout, or updates the one with
/// the same name. Never removes anything. `shared` must come from `parse`.
pub fn import(state: &mut SavedState, shared: &SharedLoadout) -> Imported {
    let mut ids: Vec<Uuid> = Vec::new();
    let mut new_favorites = 0;
    for skill in &shared.skills {
        let existing = state.favorites.iter().find(|f| {
            f.source.eq_ignore_ascii_case(&skill.source)
                && f.skill.as_deref().unwrap_or("") == skill.skill.as_deref().unwrap_or("")
        });
        let id = match existing {
            Some(favorite) => favorite.id,
            None => {
                let mut favorite = Favorite {
                    id: Uuid::new_v4(),
                    name: skill.name.clone(),
                    source: skill.source.clone(),
                    skill: skill.skill.clone(),
                    tags: skill.tags.clone(),
                };
                if favorite.tags.is_empty() {
                    favorite.tags = tags::suggest_tags(&favorite);
                }
                new_favorites += 1;
                let id = favorite.id;
                state.favorites.push(favorite);
                id
            }
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let same_name = state
        .loadouts
        .iter_mut()
        .find(|l| l.name.eq_ignore_ascii_case(&shared.name));
    let replaced = same_name.is_some();
    let loadout = match same_name {
        Some(loadout) => {
            loadout.favorite_ids = ids;
            loadout.tags = shared.tags.clone();
            loadout.preferred_agent = shared.preferred_agent.clone();
            loadout.version = shared.version.clone();
            loadout.source = shared.source.clone();
            loadout.clone()
        }
        None => {
            let loadout = Loadout {
                id: Uuid::new_v4(),
                name: shared.name.clone(),
                favorite_ids: ids,
                tags: shared.tags.clone(),
                preferred_agent: shared.preferred_agent.clone(),
                version: shared.version.clone(),
                source: shared.source.clone(),
            };
            state.loadouts.push(loadout.clone());
            loadout
        }
    };
    Imported {
        loadout: loadout.id,
        name: loadout.name,
        new_favorites,
        replaced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse as paste;

    fn web_app() -> (SavedState, Uuid) {
        let mut state = SavedState::default();
        state.add_favorites(paste(
            "npx skills add anthropics/skills --skill frontend-design --skill pdf",
        ));
        state.add_favorites(paste("https://github.com/acme/kit/tree/main/skills/deploy"));
        let ids: Vec<Uuid> = state.favorites.iter().map(|f| f.id).collect();
        let id = state
            .create_loadout("Web app", &[ids[0], ids[2]], vec!["nextjs".into()])
            .unwrap();
        state.set_loadout_agent(id, Some("pi".into()));
        (state, id)
    }

    #[test]
    fn export_carries_sources() {
        let (state, id) = web_app();
        let shared = export(&state, id).unwrap();
        assert_eq!(shared.loadout, FORMAT);
        assert_eq!(shared.name, "Web app");
        assert_eq!(shared.preferred_agent.as_deref(), Some("pi"));
        assert_eq!(shared.tags, ["nextjs"]);
        assert_eq!(shared.skills.len(), 2);
        assert_eq!(shared.skills[0].source, "anthropics/skills");
        assert_eq!(shared.skills[0].skill.as_deref(), Some("frontend-design"));
        assert_eq!(
            shared.skills[1].source,
            "https://github.com/acme/kit/tree/main/skills/deploy"
        );
        assert!(export(&state, Uuid::new_v4()).is_none());
    }

    #[test]
    fn file_and_link_round_trip_to_a_clean_machine() {
        let (state, id) = web_app();
        let shared = export(&state, id).unwrap();
        let json = to_json(&shared);
        assert!(json.contains("\"preferredAgent\": \"pi\""));
        let link = to_link(&shared);
        assert!(link.starts_with("loadout://import?data="));
        let data = link.strip_prefix(LINK_PREFIX).unwrap();
        assert!(!data.contains(['+', '/', '=']), "{data}");
        assert_eq!(parse(&json).unwrap(), shared);
        assert_eq!(parse(&format!("  {link}\n")).unwrap(), shared);

        let mut clean = SavedState::default();
        let imported = import(&mut clean, &parse(&link).unwrap());
        assert_eq!(imported.name, "Web app");
        assert_eq!(imported.new_favorites, 2);
        assert!(!imported.replaced);
        let loadout = clean.loadout(imported.loadout).unwrap();
        assert_eq!(loadout.preferred_agent.as_deref(), Some("pi"));
        assert_eq!(loadout.tags, ["nextjs"]);
        let names: Vec<_> = clean
            .loadout_favorites(loadout.id)
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, ["frontend-design", "deploy"]);
    }

    #[test]
    fn import_reuses_favorites_and_updates_by_name() {
        let (mut state, id) = web_app();
        let mut shared = export(&state, id).unwrap();
        shared.name = "web APP".into();
        shared.preferred_agent = Some("cursor".into());
        shared.skills.push(SharedSkill {
            name: "tdd".into(),
            source: "mattpocock/skills".into(),
            skill: Some("tdd".into()),
            tags: vec![],
        });
        let before = state.favorites.len();
        let imported = import(&mut state, &parse(&to_json(&shared)).unwrap());
        assert_eq!(imported.loadout, id);
        assert!(imported.replaced);
        assert_eq!(imported.new_favorites, 1);
        assert_eq!(state.favorites.len(), before + 1);
        assert_eq!(state.loadouts.len(), 1);
        let loadout = state.loadout(id).unwrap();
        assert_eq!(loadout.name, "Web app");
        assert_eq!(loadout.favorite_ids.len(), 3);
        assert_eq!(loadout.preferred_agent.as_deref(), Some("cursor"));
        // The new favorite got suggested tags like a pasted one.
        assert_eq!(state.favorites.last().unwrap().tags, ["testing"]);
    }

    fn json(skills: &str, extra: &str) -> String {
        format!(r#"{{"loadout": 1, "name": "Web app", {extra} "skills": [{skills}]}}"#)
    }

    #[test]
    fn rejects_what_it_cant_trust() {
        let ok = r#"{"name": "tdd", "source": "mattpocock/skills", "skill": "tdd"}"#;
        assert!(parse(&json(ok, "")).is_ok());
        assert_eq!(parse("hello"), Err(ShareError::NotALoadout));
        assert_eq!(parse(r#"{"name": "x"}"#), Err(ShareError::NotALoadout));
        assert_eq!(
            parse("loadout://import?data=!!!"),
            Err(ShareError::NotALoadout)
        );
        assert_eq!(
            parse(&json(ok, "").replace("\"loadout\": 1", "\"loadout\": 2")),
            Err(ShareError::NewerFormat)
        );
        assert_eq!(parse(&json("", "")), Err(ShareError::NoSkills));
        assert_eq!(
            parse(&json(ok, "").replace("Web app", " ")),
            Err(ShareError::NotALoadout)
        );
        let local = r#"{"name": "x", "source": "/etc", "skill": "x"}"#;
        assert_eq!(
            parse(&json(local, "")),
            Err(ShareError::BadSkill("/etc".into()))
        );
        let control = "{\"name\": \"x\", \"source\": \"a/b\", \"skill\": \"x\\u001b[31m\"}";
        assert!(matches!(
            parse(&json(control, "")),
            Err(ShareError::BadSkill(_))
        ));
        let many = vec![ok; MAX_SKILLS + 1].join(",");
        assert_eq!(parse(&json(&many, "")), Err(ShareError::TooLarge));
        assert_eq!(parse(&" ".repeat(MAX_BYTES + 1)), Err(ShareError::TooLarge));
    }

    fn team(version: Option<&str>) -> SharedLoadout {
        let ok = r#"{"name": "tdd", "source": "mattpocock/skills", "skill": "tdd"}"#;
        let extra = match version {
            Some(v) => format!(
                r#""version": "{v}", "source": "https://example.com/team/web.loadout.json","#
            ),
            None => String::new(),
        };
        parse(&json(ok, &extra)).unwrap()
    }

    #[test]
    fn versions_compare_numerically() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("1.10", "1.9"), Greater);
        assert_eq!(compare_versions("v2", "2.0"), Equal);
        assert_eq!(compare_versions("2026.11.1", "2026.2.30"), Greater);
        assert_eq!(compare_versions("1.0-beta", "1.0-alpha"), Greater);
        assert_eq!(compare_versions("1", "1.0.1"), Less);
    }

    #[test]
    fn versioned_import_records_version_and_source() {
        let mut state = SavedState::default();
        let v1 = team(Some("1.2"));
        assert_eq!(
            v1.source.as_deref(),
            Some("https://example.com/team/web.loadout.json")
        );
        assert_eq!(version_change(&state, &v1), VersionChange::New);
        let imported = import(&mut state, &v1);
        let loadout = state.loadout(imported.loadout).unwrap();
        assert_eq!(loadout.version.as_deref(), Some("1.2"));
        assert_eq!(
            loadout.source.as_deref(),
            Some("https://example.com/team/web.loadout.json")
        );

        let entry = provenance_entry(&v1, &imported);
        assert_eq!(entry.action, "import_loadout");
        assert_eq!(entry.loadout_id, Some(imported.loadout));
        assert_eq!(
            entry.skill_source,
            "https://example.com/team/web.loadout.json"
        );
        assert_eq!(entry.skill_name.as_deref(), Some("Web app"));
        assert_eq!(entry.note.as_deref(), Some("version 1.2"));
        assert_eq!(
            crate::provenance::describe(&entry, entry.ts),
            "Imported Web app in Global · version 1.2 · just now"
        );

        assert_eq!(
            version_change(&state, &team(Some("1.10"))),
            VersionChange::Newer
        );
        assert_eq!(
            version_change(&state, &team(Some("v1.2"))),
            VersionChange::Same
        );
        assert_eq!(
            version_change(&state, &team(Some("1.1"))),
            VersionChange::Older {
                yours: "1.2".into()
            }
        );
        // Exported again, it carries both.
        let again = export(&state, imported.loadout).unwrap();
        assert_eq!(
            (again.version.as_deref(), again.source.is_some()),
            (Some("1.2"), true)
        );
    }

    #[test]
    fn unversioned_and_bad_sources() {
        let mut state = SavedState::default();
        let plain = team(None);
        import(&mut state, &plain);
        assert_eq!(version_change(&state, &plain), VersionChange::Unversioned);
        assert_eq!(
            version_change(&state, &team(Some("1"))),
            VersionChange::Newer
        );
        let ok = r#"{"name": "tdd", "source": "mattpocock/skills", "skill": "tdd"}"#;
        for source in [
            "http://example.com/x.json",
            "file:///etc/passwd",
            "https://",
            "https://a b",
        ] {
            let shared = parse(&json(ok, &format!(r#""source": "{source}","#))).unwrap();
            assert_eq!(shared.source, None, "{source}");
        }
        assert_eq!(
            fetch("http://example.com/x.json"),
            Err(ShareError::NotHttps)
        );
    }

    #[test]
    fn unknown_agents_are_dropped_and_names_trimmed() {
        let ok = r#"{"name": " tdd ", "source": "mattpocock/skills", "skill": "tdd"}"#;
        let shared = parse(&json(ok, r#""preferredAgent": "vim","#)).unwrap();
        assert_eq!(shared.preferred_agent, None);
        assert_eq!(shared.skills[0].name, "tdd");
        let shared = parse(&json(ok, r#""preferredAgent": "Claude Code","#)).unwrap();
        assert_eq!(shared.preferred_agent.as_deref(), Some("claude-code"));
    }
}
