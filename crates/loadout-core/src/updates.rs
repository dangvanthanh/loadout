//! Skill versions from GitHub (U3, DECISIONS 002/004). A version is the git tree SHA of the
//! skill's folder, so only changes to that folder's files count; README edits never do.
//! One unauthenticated trees request per repo per check (60/hour without a token).

use std::collections::HashMap;
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::model::Favorite;
use crate::parser::{is_shorthand, split_url};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

/// Where a favorite lives on GitHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRef {
    pub repo: Repo,
    /// Folder inside the repo, when the source points at one (`…/tree/main/skills/x`).
    pub folder: Option<String>,
}

/// A recursive git tree listing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree {
    /// Folder path → tree SHA ("" = the listed root).
    pub dirs: HashMap<String, String>,
    /// File path → blob SHA.
    pub blobs: HashMap<String, String>,
    /// Folders holding a SKILL.md ("" = the root).
    pub skill_dirs: Vec<String>,
    /// GitHub cut the listing short (very large repos).
    pub truncated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("GitHub limit reached. Try again in an hour.")]
    RateLimited,
    #[error("not found on GitHub")]
    NotFound,
    #[error("couldn't reach GitHub: {0}")]
    Http(String),
    #[error("unexpected GitHub response: {0}")]
    Parse(String),
}

/// GitHub owner/repo (and folder) for a favorite; `None` for non-GitHub sources.
pub fn skill_ref(favorite: &Favorite) -> Option<SkillRef> {
    source_ref(&favorite.source)
}

/// GitHub owner/repo (and folder) for a source string.
pub fn source_ref(source: &str) -> Option<SkillRef> {
    let source = source.trim();
    if is_shorthand(source) {
        let (owner, name) = source.split_once('/')?;
        return Some(SkillRef {
            repo: Repo {
                owner: owner.into(),
                name: name.into(),
            },
            folder: None,
        });
    }
    let (host, parts) = split_url(source)?;
    if host != "github.com" && host != "www.github.com" {
        return None;
    }
    let [owner, name, rest @ ..] = parts.as_slice() else {
        return None;
    };
    let repo = Repo {
        owner: owner.to_string(),
        name: name.strip_suffix(".git").unwrap_or(name).to_string(),
    };
    let folder = match rest {
        ["tree" | "blob", _branch, path @ ..] if !path.is_empty() => {
            let folder = path.join("/");
            // A link to the SKILL.md file itself means its folder.
            Some(
                folder
                    .strip_suffix("/SKILL.md")
                    .map(str::to_string)
                    .unwrap_or(folder),
            )
        }
        _ => None,
    };
    Some(SkillRef { repo, folder })
}

/// Parses a `GET /repos/{o}/{r}/git/trees/{ref}?recursive=1` response.
pub fn parse_tree(json: &str) -> Result<Tree, UpdateError> {
    let response: TreeResponse =
        serde_json::from_str(json).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let mut tree = Tree {
        dirs: [(String::new(), response.sha)].into(),
        truncated: response.truncated,
        ..Default::default()
    };
    for entry in response.tree {
        match entry.kind.as_str() {
            "tree" => {
                tree.dirs.insert(entry.path, entry.sha);
            }
            "blob" => {
                if entry.path == "SKILL.md" {
                    tree.skill_dirs.push(String::new());
                } else if let Some(dir) = entry.path.strip_suffix("/SKILL.md") {
                    tree.skill_dirs.push(dir.to_string());
                }
                tree.blobs.insert(entry.path, entry.sha);
            }
            _ => {}
        }
    }
    tree.skill_dirs.sort();
    Ok(tree)
}

/// The favorite's version inside `tree`: its folder's tree SHA, or for a whole-repo favorite a
/// hash over every skill folder's SHA. `None` if the skill isn't in the tree.
// shortcut: matches skills by folder name only (DECISIONS 004 also allows frontmatter `name:`);
// add the frontmatter lookup if renamed folders show up as "not found".
pub fn version(favorite: &Favorite, skill_ref: &SkillRef, tree: &Tree) -> Option<String> {
    let named =
        skill_ref.folder.is_some() || favorite.skill.as_deref().is_some_and(|s| !s.is_empty());
    if named {
        return skill_dir(favorite, skill_ref, tree).and_then(|dir| folder_version(tree, dir));
    }
    // Whole repo: combine every skill folder's version.
    let versions: Vec<String> = tree
        .skill_dirs
        .iter()
        .filter_map(|dir| folder_version(tree, dir))
        .collect();
    if versions.is_empty() {
        return tree.dirs.get("").cloned();
    }
    Some(sha256_hex(&versions.join(",")))
}

/// A folder's tree SHA. The repo root also holds repo-level files, so a root skill hashes its
/// files minus README / LICENSE / CHANGELOG / .github instead.
fn folder_version(tree: &Tree, dir: &str) -> Option<String> {
    if !dir.is_empty() {
        return tree.dirs.get(dir).cloned();
    }
    let repo_level = |path: &str| {
        let upper = path.to_ascii_uppercase();
        path.starts_with(".github/")
            || (!path.contains('/')
                && ["README", "LICENSE", "CHANGELOG"]
                    .iter()
                    .any(|p| upper.starts_with(p)))
    };
    let mut files: Vec<String> = tree
        .blobs
        .iter()
        .filter(|(path, _)| !repo_level(path))
        .map(|(path, sha)| format!("{path}:{sha}"))
        .collect();
    files.sort();
    Some(sha256_hex(&files.join(",")))
}

fn sha256_hex(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// The skill's folder path inside `tree`, if found.
pub fn skill_dir<'a>(favorite: &Favorite, skill_ref: &SkillRef, tree: &'a Tree) -> Option<&'a str> {
    if let Some(folder) = &skill_ref.folder {
        return tree
            .dirs
            .get_key_value(folder.as_str())
            .map(|(k, _)| k.as_str());
    }
    let want = normalize(favorite.skill.as_deref()?);
    tree.skill_dirs
        .iter()
        .find(|dir| {
            let name = if dir.is_empty() {
                skill_ref.repo.name.as_str()
            } else {
                dir.rsplit('/').next().unwrap_or(dir)
            };
            normalize(name) == want
        })
        .map(String::as_str)
}

pub(crate) fn normalize(name: &str) -> String {
    name.trim().to_lowercase().replace(' ', "-")
}

#[derive(Deserialize)]
struct TreeResponse {
    sha: String,
    #[serde(default)]
    tree: Vec<TreeEntry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
}

/// Latest tree of the default branch (blocking).
pub fn fetch_tree(repo: &Repo, token: Option<&str>) -> Result<Tree, UpdateError> {
    match fetch_tree_at(repo, "HEAD", token) {
        Err(UpdateError::NotFound) => {
            let json = get(
                &format!("https://api.github.com/repos/{}/{}", repo.owner, repo.name),
                token,
            )?;
            let branch = serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| Some(v.get("default_branch")?.as_str()?.to_string()))
                .ok_or_else(|| UpdateError::Parse("no default_branch".into()))?;
            fetch_tree_at(repo, &branch, token)
        }
        result => result,
    }
}

/// Tree at a branch, tag, or tree SHA (a skill folder's version lists just that folder).
pub fn fetch_tree_at(repo: &Repo, git_ref: &str, token: Option<&str>) -> Result<Tree, UpdateError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{git_ref}?recursive=1",
        repo.owner, repo.name
    );
    parse_tree(&get(&url, token)?)
}

/// A file's text by blob SHA.
pub fn fetch_blob(repo: &Repo, sha: &str, token: Option<&str>) -> Result<String, UpdateError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/blobs/{sha}",
        repo.owner, repo.name
    );
    let json: serde_json::Value =
        serde_json::from_str(&get(&url, token)?).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let content: String = json
        .get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| UpdateError::Parse("blob without content".into()))?
        .split_whitespace()
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content)
        .map_err(|e| UpdateError::Parse(e.to_string()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn get(url: &str, token: Option<&str>) -> Result<String, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    let mut request = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Loadout");
    if let Some(token) = token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    match request.call() {
        Ok(mut response) => response
            .body_mut()
            .read_to_string()
            .map_err(|e| UpdateError::Http(e.to_string())),
        Err(ureq::Error::StatusCode(403 | 429)) => Err(UpdateError::RateLimited),
        Err(ureq::Error::StatusCode(404 | 409 | 422)) => Err(UpdateError::NotFound),
        Err(err) => Err(UpdateError::Http(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::favorite;

    fn tree(name: &str) -> Tree {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/trees")
            .join(format!("{name}.json"));
        parse_tree(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn repo(owner: &str, name: &str) -> Repo {
        Repo {
            owner: owner.into(),
            name: name.into(),
        }
    }

    #[test]
    fn refs_for_github_sources_only() {
        let r = |source: &str| skill_ref(&favorite("x", source, None));
        assert_eq!(
            r("anthropics/skills"),
            Some(SkillRef {
                repo: repo("anthropics", "skills"),
                folder: None
            })
        );
        assert_eq!(
            r("https://github.com/acme/kit.git"),
            Some(SkillRef {
                repo: repo("acme", "kit"),
                folder: None
            })
        );
        assert_eq!(
            r("https://github.com/anthropics/skills/tree/main/skills/pdf"),
            Some(SkillRef {
                repo: repo("anthropics", "skills"),
                folder: Some("skills/pdf".into())
            })
        );
        assert_eq!(r("https://gitlab.com/a/b"), None);
        assert_eq!(r("https://docs.example.com"), None);
        assert_eq!(r("https://github.com/acme"), None);
    }

    #[test]
    fn parses_tree_listing() {
        let t = tree("before");
        assert_eq!(t.dirs[""], "r1");
        assert_eq!(t.dirs["skills/frontend-design"], "t1fd");
        assert_eq!(t.blobs["skills/pdf/scripts/fill.py"], "b0fill");
        assert_eq!(t.skill_dirs, ["skills/frontend-design", "skills/pdf"]);
        assert!(!t.truncated);
        assert_eq!(tree("single-skill-repo").skill_dirs, [""]);
        assert!(matches!(parse_tree("not json"), Err(UpdateError::Parse(_))));
    }

    #[test]
    fn readme_only_change_is_not_an_update() {
        let fav = favorite(
            "frontend-design",
            "anthropics/skills",
            Some("frontend-design"),
        );
        let whole = favorite("skills", "anthropics/skills", None);
        for fav in [fav, whole] {
            let r = skill_ref(&fav).unwrap();
            let before = version(&fav, &r, &tree("before")).unwrap();
            assert_eq!(
                version(&fav, &r, &tree("readme-only")),
                Some(before),
                "{}",
                fav.name
            );
        }
    }

    #[test]
    fn folder_change_is_an_update() {
        let fav = favorite(
            "frontend-design",
            "anthropics/skills",
            Some("frontend-design"),
        );
        let r = skill_ref(&fav).unwrap();
        assert_eq!(version(&fav, &r, &tree("before")).as_deref(), Some("t1fd"));
        assert_eq!(
            version(&fav, &r, &tree("folder-change")).as_deref(),
            Some("t3fd")
        );

        let whole = favorite("skills", "anthropics/skills", None);
        let r = skill_ref(&whole).unwrap();
        assert_ne!(
            version(&whole, &r, &tree("before")),
            version(&whole, &r, &tree("folder-change"))
        );

        // Another skill in the same repo is unaffected.
        let pdf = favorite("pdf", "anthropics/skills", Some("pdf"));
        let r = skill_ref(&pdf).unwrap();
        assert_eq!(
            version(&pdf, &r, &tree("before")),
            version(&pdf, &r, &tree("folder-change"))
        );
    }

    #[test]
    fn version_by_folder_url_and_name_normalization() {
        let url = favorite(
            "pdf",
            "https://github.com/anthropics/skills/tree/main/skills/pdf",
            None,
        );
        assert_eq!(
            version(&url, &skill_ref(&url).unwrap(), &tree("before")).as_deref(),
            Some("t0pdf")
        );

        let spaced = favorite(
            "Frontend Design",
            "anthropics/skills",
            Some("Frontend Design"),
        );
        assert_eq!(
            version(&spaced, &skill_ref(&spaced).unwrap(), &tree("before")).as_deref(),
            Some("t1fd")
        );

        let missing = favorite("nope", "anthropics/skills", Some("nope"));
        assert_eq!(
            version(&missing, &skill_ref(&missing).unwrap(), &tree("before")),
            None
        );
    }

    #[test]
    fn single_skill_repo_matches_repo_name_and_ignores_readme() {
        let fav = favorite("grill-me", "someone/grill-me", Some("grill-me"));
        let r = skill_ref(&fav).unwrap();
        let before = tree("single-skill-repo");
        assert_eq!(skill_dir(&fav, &r, &before), Some(""));
        let v = version(&fav, &r, &before).unwrap();

        let mut readme = before.clone();
        readme.blobs.insert("README.md".into(), "bread2".into());
        readme.dirs.insert("".into(), "rsingle2".into());
        assert_eq!(version(&fav, &r, &readme).as_ref(), Some(&v));

        let mut changed = before.clone();
        changed.blobs.insert("SKILL.md".into(), "bsingle2".into());
        assert_ne!(version(&fav, &r, &changed).as_ref(), Some(&v));
    }

    #[test]
    fn blob_link_to_skill_md_means_its_folder() {
        let fav = favorite(
            "pdf",
            "https://github.com/anthropics/skills/blob/main/skills/pdf/SKILL.md",
            None,
        );
        assert_eq!(
            skill_ref(&fav).unwrap().folder.as_deref(),
            Some("skills/pdf")
        );
    }

    #[test]
    fn whole_repo_without_skills_falls_back_to_root() {
        let t = Tree {
            dirs: [("".to_string(), "root".to_string())].into(),
            ..Default::default()
        };
        let fav = favorite("x", "a/b", None);
        assert_eq!(
            version(&fav, &skill_ref(&fav).unwrap(), &t).as_deref(),
            Some("root")
        );
    }
}
