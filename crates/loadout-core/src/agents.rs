//! Agent ids the `skills` CLI understands, and where they keep skills.
//!
//! Copied from `skills` 1.7.4 (`npx skills add -a <id>`). The CLI knows ~60 agents; this table
//! holds the common ones. Unknown ids still pass through to `-a` untouched.
// shortcut: ignores env overrides (CLAUDE_CONFIG_DIR, XDG_CONFIG_HOME); read them if users report misses.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Agent {
    pub id: &'static str,
    pub name: &'static str,
    /// Relative to the project root.
    pub project_dir: &'static str,
    /// Relative to the home folder.
    pub global_dir: &'static str,
}

const fn agent(
    id: &'static str,
    name: &'static str,
    project_dir: &'static str,
    global_dir: &'static str,
) -> Agent {
    Agent {
        id,
        name,
        project_dir,
        global_dir,
    }
}

pub const AGENTS: &[Agent] = &[
    agent("amp", "Amp", ".agents/skills", ".config/agents/skills"),
    agent(
        "claude-code",
        "Claude Code",
        ".claude/skills",
        ".claude/skills",
    ),
    agent("cline", "Cline", ".agents/skills", ".agents/skills"),
    agent("codex", "Codex", ".agents/skills", ".agents/skills"),
    agent("cursor", "Cursor", ".agents/skills", ".cursor/skills"),
    agent(
        "gemini-cli",
        "Gemini CLI",
        ".agents/skills",
        ".gemini/skills",
    ),
    agent(
        "github-copilot",
        "GitHub Copilot",
        ".agents/skills",
        ".copilot/skills",
    ),
    agent("goose", "Goose", ".goose/skills", ".config/goose/skills"),
    agent("junie", "Junie", ".junie/skills", ".junie/skills"),
    agent("kiro-cli", "Kiro CLI", ".kiro/skills", ".kiro/skills"),
    agent(
        "opencode",
        "OpenCode",
        ".agents/skills",
        ".config/opencode/skills",
    ),
    agent("pi", "Pi", ".agents/skills", ".agents/skills"),
    agent("qwen-code", "Qwen Code", ".qwen/skills", ".qwen/skills"),
    agent("roo", "Roo Code", ".roo/skills", ".roo/skills"),
    agent("trae", "Trae", ".trae/skills", ".trae/skills"),
    agent("warp", "Warp", ".agents/skills", ".agents/skills"),
    agent(
        "windsurf",
        "Windsurf",
        ".windsurf/skills",
        ".codeium/windsurf/skills",
    ),
    agent("zed", "Zed", ".agents/skills", ".agents/skills"),
];

pub fn find(id: &str) -> Option<&'static Agent> {
    AGENTS.iter().find(|agent| agent.id == id)
}

/// `Settings.agents` ("claude-code, cursor") → ids. Commas or whitespace separate;
/// empties and duplicates are dropped.
pub fn parse_list(agents: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in agents
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|id| !id.is_empty())
    {
        if !ids.iter().any(|seen| seen == id) {
            ids.push(id.to_string());
        }
    }
    ids
}

/// Project skill folders for these agents, without duplicates, in order. Unknown ids are skipped.
pub fn project_dirs(ids: &[String]) -> Vec<&'static str> {
    let mut dirs = Vec::new();
    for agent in ids.iter().filter_map(|id| find(id)) {
        if !dirs.contains(&agent.project_dir) {
            dirs.push(agent.project_dir);
        }
    }
    dirs
}

/// An agent the user typed: its id or name, any case ("Claude Code", "pi").
pub fn resolve(text: &str) -> Option<&'static Agent> {
    let wanted = text.trim().to_lowercase().replace(' ', "-");
    AGENTS
        .iter()
        .find(|a| a.id == wanted || a.name.to_lowercase().replace(' ', "-") == wanted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_ids_and_names() {
        assert_eq!(resolve(" Pi ").unwrap().id, "pi");
        assert_eq!(resolve("claude code").unwrap().id, "claude-code");
        assert_eq!(resolve("CURSOR").unwrap().id, "cursor");
        assert_eq!(resolve("Claude-Code").unwrap().id, "claude-code");
        assert_eq!(resolve("vim"), None);
        assert_eq!(resolve(""), None);
    }

    #[test]
    fn finds_known_agents() {
        assert_eq!(find("claude-code").unwrap().project_dir, ".claude/skills");
        assert_eq!(find("pi").unwrap().global_dir, ".agents/skills");
        assert_eq!(find("nope"), None);
    }

    #[test]
    fn parses_agent_list() {
        assert_eq!(
            parse_list(" claude-code, cursor,,claude-code ,pi "),
            vec!["claude-code", "cursor", "pi"]
        );
        assert_eq!(
            parse_list("codex cursor\tpi"),
            vec!["codex", "cursor", "pi"]
        );
        assert!(parse_list("").is_empty());
        assert!(parse_list(" , ").is_empty());
    }

    #[test]
    fn project_dirs_dedupe_and_skip_unknown() {
        let ids = parse_list("cursor,claude-code,codex,mystery");
        assert_eq!(project_dirs(&ids), vec![".agents/skills", ".claude/skills"]);
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = AGENTS.iter().map(|a| a.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), AGENTS.len());
    }
}
