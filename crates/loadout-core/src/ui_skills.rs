//! ui-skills.com/skills/<owner>/<skill> doesn't say which repo holds the skill, so read the page:
//! first an install command printed on it, then a GitHub repo owned by the skill's author.

use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use crate::model::Favorite;
use crate::parser::{favorite, parse_command, split_url};

#[derive(Debug, thiserror::Error)]
pub enum UiSkillsError {
    #[error("couldn't load {url}: {source}")]
    Fetch {
        url: String,
        source: Box<ureq::Error>,
    },
}

pub fn is_ui_skills_url(text: &str) -> bool {
    split_url(text.trim()).is_some_and(|(host, _)| host.ends_with("ui-skills.com"))
}

/// Fetches the page (blocking; run off the main thread) and resolves it.
pub fn resolve(url: &str) -> Result<Option<Favorite>, UiSkillsError> {
    let url = url.trim();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let html = agent
        .get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .map_err(|source| UiSkillsError::Fetch {
            url: url.to_string(),
            source: Box::new(source),
        })?;
    Ok(favorite_from_page(url, &html))
}

/// Commands are matched in the raw HTML, where attribute quotes are literal `"` (end of
/// the command) and quoted skill names are still `&quot;`; entities are decoded per match.
static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"npx skills add [^<\n"]+"#).expect("valid regex"));
static GITHUB_REPO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"github\.com/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)").expect("valid regex")
});

pub fn favorite_from_page(url: &str, html: &str) -> Option<Favorite> {
    let (_, parts) = split_url(url.trim())?;
    let ["skills", owner, skill, ..] = parts.as_slice() else {
        return None;
    };
    for command in COMMAND.find_iter(html) {
        let favorites = parse_command(&decode_entities(command.as_str()));
        let wanted = favorites
            .iter()
            .position(|f| f.skill.as_deref() == Some(skill))
            .unwrap_or(0);
        if let Some(found) = favorites.into_iter().nth(wanted) {
            return Some(found);
        }
    }
    // The site links its own repo too; only the author's repo counts.
    GITHUB_REPO.captures_iter(html).find_map(|caps| {
        let (repo_owner, repo) = (&caps[1], &caps[2]);
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        repo_owner
            .eq_ignore_ascii_case(owner)
            .then(|| favorite(skill, &format!("{repo_owner}/{repo}"), Some(skill)))
    })
}

fn decode_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "https://www.ui-skills.com/skills/anthropics/frontend-design";

    fn fixture() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/html/ui-skills-frontend-design.html"),
        )
        .unwrap()
    }

    #[test]
    fn recognises_ui_skills_links() {
        assert!(is_ui_skills_url(PAGE));
        assert!(is_ui_skills_url(" https://ui-skills.com/skills/a/b "));
        assert!(!is_ui_skills_url("https://skills.sh/a/b"));
        assert!(!is_ui_skills_url("anthropics/skills"));
    }

    #[test]
    fn install_command_on_saved_page() {
        let fav = favorite_from_page(PAGE, &fixture()).unwrap();
        assert_eq!(fav.name, "frontend-design");
        assert_eq!(fav.source, "anthropics/skills");
        assert_eq!(fav.skill.as_deref(), Some("frontend-design"));
    }

    #[test]
    fn falls_back_to_authors_repo_and_skips_site_repo() {
        let html = r#"<a href="https://github.com/ibelick/ui-skills">Star</a>
            <a href="https://github.com/emilkowalski/skills.git">Source</a>"#;
        let fav = favorite_from_page(
            "https://www.ui-skills.com/skills/EmilKowalski/improve-animations",
            html,
        )
        .unwrap();
        assert_eq!(fav.source, "emilkowalski/skills");
        assert_eq!(fav.skill.as_deref(), Some("improve-animations"));
    }

    #[test]
    fn decodes_entities_in_commands() {
        let html = "<code>npx skills add acme/kit --skill &quot;ui polish&quot;</code>";
        let fav = favorite_from_page("https://ui-skills.com/skills/acme/ui-polish", html).unwrap();
        assert_eq!(fav.source, "acme/kit");
        assert_eq!(fav.skill.as_deref(), Some("ui polish"));
    }

    #[test]
    fn non_skill_pages_and_empty_pages() {
        assert_eq!(
            favorite_from_page("https://ui-skills.com/skills/accessibility", &fixture()),
            None
        );
        assert_eq!(
            favorite_from_page("https://ui-skills.com/skills/a/b", "<p>nothing</p>"),
            None
        );
    }
}
