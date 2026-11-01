//! Paste text → favorites (port of the prototype's `SourceParser`). Understands:
//!   owner/repo                         owner/repo@skill
//!   https://github.com/owner/repo      https://github.com/owner/repo/tree/main/skills/x
//!   https://gitlab.com/group/repo      git@github.com:owner/repo.git
//!   https://skills.sh/owner/repo/skill https://skills.sh/site/domain.com/skill
//!   npx skills add owner/repo --skill a --skill b
//! ui-skills.com links need a page fetch; see `ui_skills`.

use uuid::Uuid;

use crate::model::Favorite;

/// skills.sh paths that are site pages, not owners.
const SKILLS_SH_RESERVED: &[&str] = &[
    "trending", "hot", "official", "packs", "topic", "audits", "docs", "agent", "about", "contact",
    "privacy", "terms", "api",
];

pub fn parse(raw: &str) -> Vec<Favorite> {
    let text = raw.trim();
    if text.contains("skills add ") {
        return parse_command(text);
    }
    parse_one(text).into_iter().collect()
}

fn parse_one(text: &str) -> Option<Favorite> {
    if text.is_empty() {
        return None;
    }
    // owner/repo@skill
    if !text.contains("://")
        && !text.starts_with("git@")
        && let Some((repo, skill)) = text.split_once('@')
    {
        return (is_shorthand(repo) && !skill.is_empty())
            .then(|| favorite(skill, repo, Some(skill)));
    }
    if is_shorthand(text) {
        let (_, repo) = text.split_once('/')?;
        return Some(favorite(repo, text, None));
    }
    // git@github.com:owner/repo.git
    if text.starts_with("git@") {
        let last = text.rsplit('/').next().unwrap_or(text);
        return Some(favorite(
            last.strip_suffix(".git").unwrap_or(last),
            text,
            None,
        ));
    }

    let (host, parts) = split_url(text)?;
    if host == "skills.sh" || host.ends_with(".skills.sh") {
        return match parts.as_slice() {
            ["site", domain, skill, ..] => {
                Some(favorite(skill, &format!("https://{domain}"), Some(skill)))
            }
            [first, ..] if SKILLS_SH_RESERVED.contains(first) => None,
            [owner, repo, skill, ..] => {
                Some(favorite(skill, &format!("{owner}/{repo}"), Some(skill)))
            }
            [owner, repo] => Some(favorite(repo, &format!("{owner}/{repo}"), None)),
            _ => None,
        };
    }
    if host == "github.com" || host == "www.github.com" {
        return match parts.as_slice() {
            // A path to one skill folder: the CLI accepts the full URL.
            [_, _, "tree" | "blob", _, ..] => Some(favorite(parts.last()?, text, None)),
            [owner, repo, ..] => {
                let repo = repo.strip_suffix(".git").unwrap_or(repo);
                Some(favorite(repo, &format!("{owner}/{repo}"), None))
            }
            _ => None,
        };
    }
    if host.ends_with("gitlab.com") {
        return Some(favorite(parts.last().copied().unwrap_or(text), text, None));
    }
    None
}

fn is_shorthand(text: &str) -> bool {
    let allowed = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    };
    text.split_once('/')
        .is_some_and(|(owner, repo)| allowed(owner) && allowed(repo))
}

/// A pasted `npx skills add …` (or bunx/pnpm dlx) command.
pub(crate) fn parse_command(text: &str) -> Vec<Favorite> {
    let tokens = tokenize(text);
    let Some(add) = tokens.iter().position(|t| t == "add") else {
        return Vec::new();
    };
    let mut source = None;
    let mut skills = Vec::new();
    let mut rest = tokens[add + 1..].iter().peekable();
    while let Some(token) = rest.next() {
        match token.as_str() {
            "--skill" | "-s" | "-a" | "--agent" => {
                let is_skill = token == "--skill" || token == "-s";
                while let Some(value) = rest.next_if(|t| !t.starts_with('-')) {
                    if is_skill {
                        skills.push(value.as_str());
                    }
                }
            }
            flag if flag.starts_with('-') => {}
            value => {
                source.get_or_insert(value);
            }
        }
    }
    let Some(source) = source else {
        return Vec::new();
    };
    let base = parse_one(source).unwrap_or_else(|| favorite(source, source, None));
    let named: Vec<_> = skills.into_iter().filter(|s| *s != "*").collect();
    if named.is_empty() {
        return vec![base];
    }
    named
        .into_iter()
        .map(|skill| favorite(skill, &base.source, Some(skill)))
        .collect()
}

/// Splits on whitespace; single or double quotes group words.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for c in text.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// `https://host/a/b?q#f` → ("host", ["a", "b"]). Only http(s).
pub(crate) fn split_url(text: &str) -> Option<(String, Vec<&str>)> {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if host.is_empty() || host.contains(char::is_whitespace) {
        return None;
    }
    let parts = path.split('/').filter(|part| !part.is_empty()).collect();
    Some((host.to_ascii_lowercase(), parts))
}

pub(crate) fn favorite(name: &str, source: &str, skill: Option<&str>) -> Favorite {
    Favorite {
        id: Uuid::new_v4(),
        name: name.to_string(),
        source: source.to_string(),
        skill: skill.map(str::to_string),
        tags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (name, source, skill) for each parsed favorite.
    fn p(text: &str) -> Vec<(String, String, Option<String>)> {
        parse(text)
            .into_iter()
            .map(|f| (f.name, f.source, f.skill))
            .collect()
    }

    fn one(name: &str, source: &str, skill: Option<&str>) -> Vec<(String, String, Option<String>)> {
        vec![(name.into(), source.into(), skill.map(Into::into))]
    }

    #[test]
    fn shorthand() {
        assert_eq!(
            p("vercel-labs/agent-skills"),
            one("agent-skills", "vercel-labs/agent-skills", None)
        );
    }

    #[test]
    fn shorthand_with_skill() {
        assert_eq!(
            p("anthropics/skills@frontend-design"),
            one(
                "frontend-design",
                "anthropics/skills",
                Some("frontend-design")
            )
        );
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(
            p("  \n anthropics/skills \t"),
            one("skills", "anthropics/skills", None)
        );
    }

    #[test]
    fn github_repo_url() {
        assert_eq!(
            p("https://github.com/mattpocock/skills"),
            one("skills", "mattpocock/skills", None)
        );
        assert_eq!(
            p("https://www.github.com/mattpocock/skills/"),
            one("skills", "mattpocock/skills", None)
        );
        assert_eq!(
            p("https://github.com/mattpocock/skills?tab=readme#top"),
            one("skills", "mattpocock/skills", None)
        );
    }

    #[test]
    fn github_dot_git_only_strips_suffix() {
        assert_eq!(
            p("https://github.com/acme/skills.git"),
            one("skills", "acme/skills", None)
        );
        assert_eq!(
            p("https://github.com/acme/my.github-tools"),
            one("my.github-tools", "acme/my.github-tools", None)
        );
    }

    #[test]
    fn github_tree_and_blob_urls_keep_full_url() {
        let tree = "https://github.com/anthropics/skills/tree/main/skills/frontend-design";
        assert_eq!(p(tree), one("frontend-design", tree, None));
        let blob = "https://github.com/anthropics/skills/blob/main/skills/pdf";
        assert_eq!(p(blob), one("pdf", blob, None));
    }

    #[test]
    fn github_owner_only_is_rejected() {
        assert!(p("https://github.com/anthropics").is_empty());
    }

    #[test]
    fn gitlab_url() {
        let url = "https://gitlab.com/acme/tools/agent-skills";
        assert_eq!(p(url), one("agent-skills", url, None));
    }

    #[test]
    fn ssh_git_url() {
        let url = "git@github.com:acme/agent-skills.git";
        assert_eq!(p(url), one("agent-skills", url, None));
    }

    #[test]
    fn skills_sh_repo_and_skill() {
        assert_eq!(
            p("https://skills.sh/vercel-labs/agent-skills"),
            one("agent-skills", "vercel-labs/agent-skills", None)
        );
        assert_eq!(
            p("https://skills.sh/vercel-labs/agent-skills/vercel-react-best-practices"),
            one(
                "vercel-react-best-practices",
                "vercel-labs/agent-skills",
                Some("vercel-react-best-practices")
            )
        );
    }

    #[test]
    fn skills_sh_site_skill() {
        assert_eq!(
            p("https://skills.sh/site/docs.example.com/deploy"),
            one("deploy", "https://docs.example.com", Some("deploy"))
        );
    }

    #[test]
    fn skills_sh_reserved_and_bare_paths() {
        assert!(p("https://skills.sh/trending").is_empty());
        assert!(p("https://skills.sh/docs/cli/add").is_empty());
        assert!(p("https://skills.sh/").is_empty());
        assert!(p("https://skills.sh/anthropics").is_empty());
    }

    #[test]
    fn command_with_several_skills() {
        assert_eq!(
            p("npx skills add mattpocock/skills --skill tdd --skill grill-me"),
            vec![
                ("tdd".into(), "mattpocock/skills".into(), Some("tdd".into())),
                (
                    "grill-me".into(),
                    "mattpocock/skills".into(),
                    Some("grill-me".into())
                ),
            ]
        );
    }

    #[test]
    fn command_with_quotes_short_flags_and_agents() {
        assert_eq!(
            p(
                r#"npx skills add https://github.com/acme/skills.git -a claude-code cursor -s "web design" 'api docs' -y"#
            ),
            vec![
                (
                    "web design".into(),
                    "acme/skills".into(),
                    Some("web design".into())
                ),
                (
                    "api docs".into(),
                    "acme/skills".into(),
                    Some("api docs".into())
                ),
            ]
        );
    }

    #[test]
    fn command_with_wildcard_or_no_skill_is_whole_source() {
        assert_eq!(
            p("npx skills add anthropics/skills --skill '*'"),
            one("skills", "anthropics/skills", None)
        );
        assert_eq!(
            p("bunx skills add anthropics/skills -g -y"),
            one("skills", "anthropics/skills", None)
        );
    }

    #[test]
    fn command_with_unknown_source_keeps_it_verbatim() {
        assert_eq!(
            p("npx skills add ./local/skills"),
            one("./local/skills", "./local/skills", None)
        );
        assert!(p("npx skills add").is_empty());
        assert!(p("npx skills add --skill x").is_empty());
    }

    #[test]
    fn garbage_is_empty() {
        assert!(p("").is_empty());
        assert!(p("hello world").is_empty());
        assert!(p("https://example.com/a/b").is_empty());
        assert!(p("owner/repo/extra").is_empty());
        assert!(p("owner/repo@").is_empty());
    }
}
