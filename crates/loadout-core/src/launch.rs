//! Opening the preferred agent after equipping (U9). Pure helpers; the app runs them.
//! Terminal agents open through a `.command` file, which Terminal (or the user's default for
//! `.command`) runs: no Automation permission needed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agents;
use crate::installer::shq;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    /// `open -a <app> <dir>`.
    App(&'static str),
    /// A CLI started in a new terminal window, in the project folder.
    Terminal(&'static str),
    /// The user's own shell command, `{dir}` already filled in.
    Custom(String),
}

/// Default launchers by agent id: CLIs in a terminal, editors with `open -a`.
const DEFAULTS: &[(&str, Launch)] = &[
    ("amp", Launch::Terminal("amp")),
    ("claude-code", Launch::Terminal("claude")),
    ("cline", Launch::App("Visual Studio Code")),
    ("codex", Launch::Terminal("codex")),
    ("cursor", Launch::App("Cursor")),
    ("gemini-cli", Launch::Terminal("gemini")),
    ("github-copilot", Launch::App("Visual Studio Code")),
    ("goose", Launch::Terminal("goose")),
    ("kiro-cli", Launch::Terminal("kiro-cli")),
    ("opencode", Launch::Terminal("opencode")),
    ("pi", Launch::Terminal("pi")),
    ("qwen-code", Launch::Terminal("qwen")),
    ("roo", Launch::App("Visual Studio Code")),
    ("trae", Launch::App("Trae")),
    ("warp", Launch::App("Warp")),
    ("windsurf", Launch::App("Windsurf")),
    ("zed", Launch::App("Zed")),
];

/// How to open `agent` in `dir`: a custom command from settings first, then the default.
pub fn launcher(agent: &str, dir: &Path, custom: &BTreeMap<String, String>) -> Option<Launch> {
    if let Some(template) = custom.get(agent).filter(|t| !t.trim().is_empty()) {
        let dir = shq(&dir.to_string_lossy());
        return Some(Launch::Custom(template.replace("{dir}", &dir)));
    }
    DEFAULTS
        .iter()
        .find(|(id, _)| *id == agent)
        .map(|(_, launch)| launch.clone())
}

/// The agent to open: the first preferred agent among `loadout_agents`, else the first configured
/// in `settings_agents` ("claude-code, cursor").
pub fn pick_agent(loadout_agents: &[Option<String>], settings_agents: &str) -> Option<String> {
    loadout_agents
        .iter()
        .flatten()
        .next()
        .cloned()
        .or_else(|| agents::parse_list(settings_agents).into_iter().next())
}

/// The `.command` file that opens `command` in `dir` in a login shell.
pub fn terminal_script(command: &str, dir: &Path) -> String {
    format!(
        "#!/bin/zsh -l\ncd {} || exit 1\nclear\nexec {command}\n",
        shq(&dir.to_string_lossy())
    )
}

/// Where the `.command` file for `agent` goes.
pub fn script_path(agent: &str) -> Option<PathBuf> {
    Some(
        dirs::cache_dir()?
            .join("Loadout")
            .join(format!("open-{agent}.command")),
    )
}

/// "Pi", "Claude Code", or the id itself.
pub fn agent_name(agent: &str) -> &str {
    agents::find(agent).map_or(agent, |a| a.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn default_launchers() {
        let dir = Path::new("/Code/shop");
        assert_eq!(launcher("pi", dir, &none()), Some(Launch::Terminal("pi")));
        assert_eq!(
            launcher("claude-code", dir, &none()),
            Some(Launch::Terminal("claude"))
        );
        assert_eq!(
            launcher("cursor", dir, &none()),
            Some(Launch::App("Cursor"))
        );
        assert_eq!(launcher("junie", dir, &none()), None);
        assert_eq!(launcher("mystery", dir, &none()), None);
    }

    #[test]
    fn custom_launchers_win_and_quote_the_folder() {
        let custom = BTreeMap::from([
            (
                "pi".to_string(),
                "open -a Ghostty --args -e pi --cwd {dir}".to_string(),
            ),
            ("junie".to_string(), "idea {dir}".to_string()),
        ]);
        let dir = Path::new("/Code/it's mine");
        assert_eq!(
            launcher("pi", dir, &custom),
            Some(Launch::Custom(
                r"open -a Ghostty --args -e pi --cwd '/Code/it'\''s mine'".into()
            ))
        );
        assert_eq!(
            launcher("junie", dir, &custom),
            Some(Launch::Custom(r"idea '/Code/it'\''s mine'".into()))
        );
        // A blank custom command falls back to the default.
        let blank = BTreeMap::from([("cursor".to_string(), "  ".to_string())]);
        assert_eq!(launcher("cursor", dir, &blank), Some(Launch::App("Cursor")));
    }

    #[test]
    fn picks_loadout_agent_then_settings() {
        assert_eq!(
            pick_agent(
                &[None, Some("pi".into()), Some("cursor".into())],
                "claude-code"
            ),
            Some("pi".into())
        );
        assert_eq!(pick_agent(&[None], " codex, cursor"), Some("codex".into()));
        assert_eq!(pick_agent(&[], ""), None);
    }

    #[test]
    fn terminal_script_cds_then_execs() {
        let script = terminal_script("pi", Path::new("/Code/my shop"));
        assert_eq!(
            script,
            "#!/bin/zsh -l\ncd '/Code/my shop' || exit 1\nclear\nexec pi\n"
        );
    }

    #[test]
    fn names() {
        assert_eq!(agent_name("pi"), "Pi");
        assert_eq!(agent_name("claude-code"), "Claude Code");
        assert_eq!(agent_name("mystery"), "mystery");
    }
}
