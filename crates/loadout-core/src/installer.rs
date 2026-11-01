//! Builds and runs `npx skills add/remove` (U6, DECISIONS 001).
//! Every user value goes through `shq`; commands always pass `-y`; stdin is null.
//! Never `skills check` / `skills update`: they change files (DECISIONS 003).

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;

use crate::agents;
use crate::model::{Favorite, Settings};

/// Printed by the shell preamble when `npx` can't be found.
const NPX_MISSING: &str = "npx not found. Install Node.js (brew install node) and try again.";

/// Login-shell preamble so Homebrew, Volta, and nvm installs of Node are found.
const PREAMBLE: &str = concat!(
    r#"export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.volta/bin:$PATH"; "#,
    r#"[ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh" >/dev/null 2>&1; "#,
);

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("npx not found. Install Node.js (brew install node) and try again.")]
    NpxMissing,
    #[error("refusing to run `{0}`: skills check/update change files")]
    Forbidden(String),
    #[error("couldn't start the shell: {0}")]
    Spawn(std::io::Error),
}

/// Single-quotes a value for zsh.
pub fn shq(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// `Settings.agents` → the ids passed with `-a`.
pub fn agent_list(settings: &Settings) -> Vec<String> {
    agents::parse_list(&settings.agents)
}

/// `npx -y skills add '<source>' [--skill '<skill>'] [-a '<agent>']… [-g] -y`
pub fn install_command(favorite: &Favorite, settings: &Settings) -> String {
    let mut command = format!("npx -y skills add {}", shq(&favorite.source));
    if let Some(skill) = favorite.skill.as_deref().filter(|s| !s.is_empty()) {
        command += &format!(" --skill {}", shq(skill));
    }
    command + &scope_flags(settings)
}

/// `-a` per agent, `-g` for global, and the always-on `-y`.
fn scope_flags(settings: &Settings) -> String {
    let mut flags: String = agent_list(settings)
        .iter()
        .map(|agent| format!(" -a {}", shq(agent)))
        .collect();
    if settings.global {
        flags += " -g";
    }
    flags + " -y"
}

/// `npx -y skills remove '<name>'… [-a '<agent>']… [-g] -y`. `None` without names: the CLI
/// would ask interactively.
pub fn remove_command(skill_names: &[&str], settings: &Settings) -> Option<String> {
    if skill_names.is_empty() {
        return None;
    }
    let names: String = skill_names
        .iter()
        .map(|name| format!(" {}", shq(name)))
        .collect();
    Some(format!(
        "npx -y skills remove{names}{}",
        scope_flags(settings)
    ))
}

/// Runs `command` through a login shell in `dir` (home if `None`), streaming each output line
/// (stdout and stderr, ANSI stripped) to `on_line`. Blocking. Returns the exit code.
pub fn run(
    command: &str,
    dir: Option<&Path>,
    on_line: impl FnMut(&str),
) -> Result<i32, InstallError> {
    let words: Vec<&str> = command.split_whitespace().collect();
    if words
        .windows(2)
        .any(|w| w[0] == "skills" && matches!(w[1], "check" | "update"))
    {
        return Err(InstallError::Forbidden(command.to_string()));
    }
    let script = format!(
        "{PREAMBLE}command -v npx >/dev/null || {{ echo '{NPX_MISSING}'; exit 127; }}; {command}"
    );
    run_script(&script, dir, on_line)
}

fn run_script(
    script: &str,
    dir: Option<&Path>,
    mut on_line: impl FnMut(&str),
) -> Result<i32, InstallError> {
    let (mut reader, writer) = std::io::pipe().map_err(InstallError::Spawn)?;
    let mut child = {
        // Scoped: the Command holds the pipe's write ends; they must close for EOF.
        let mut cmd = Command::new("/bin/zsh");
        cmd.args(["-lc", script])
            .env("NO_COLOR", "1")
            .env("FORCE_COLOR", "0")
            .env("DO_NOT_TRACK", "1")
            .stdin(Stdio::null())
            .stdout(writer.try_clone().map_err(InstallError::Spawn)?)
            .stderr(writer);
        if let Some(dir) = dir.map(Path::to_path_buf).or_else(dirs::home_dir) {
            cmd.current_dir(dir);
        }
        cmd.spawn().map_err(InstallError::Spawn)?
    };

    let mut npx_missing = false;
    let mut emit = |bytes: &[u8]| {
        let line = strip_ansi(&String::from_utf8_lossy(bytes));
        let line = line.trim_end();
        if !line.is_empty() {
            npx_missing |= line == NPX_MISSING;
            on_line(line);
        }
    };
    // Split on \n and \r (progress output rewrites lines with \r); both are single UTF-8 bytes.
    let mut pending = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        pending.extend_from_slice(&chunk[..read]);
        while let Some(end) = pending.iter().position(|&b| b == b'\n' || b == b'\r') {
            emit(&pending[..end]);
            pending.drain(..=end);
        }
    }
    emit(&pending);

    let code = child
        .wait()
        .map_err(InstallError::Spawn)?
        .code()
        .unwrap_or(-1);
    if npx_missing && code == 127 {
        return Err(InstallError::NpxMissing);
    }
    Ok(code)
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1B\[[0-9;?]*[ -/]*[@-~]|\x1B\][^\x07\x1B]*(\x07|\x1B\\)|\x1B[@-Z\\-_]")
        .expect("valid regex")
});

pub fn strip_ansi(text: &str) -> String {
    ANSI.replace_all(text, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(agents: &str, global: bool) -> Settings {
        Settings {
            agents: agents.into(),
            global,
            ..Default::default()
        }
    }

    fn fav(source: &str, skill: Option<&str>) -> Favorite {
        crate::parser::favorite("x", source, skill)
    }

    #[test]
    fn shq_quotes_everything() {
        assert_eq!(shq("anthropics/skills"), "'anthropics/skills'");
        assert_eq!(shq(""), "''");
        assert_eq!(shq("it's; rm -rf ~"), r"'it'\''s; rm -rf ~'");
    }

    #[test]
    fn install_one_skill_for_agents() {
        assert_eq!(
            install_command(
                &fav("anthropics/skills", Some("frontend-design")),
                &settings("claude-code, cursor", false)
            ),
            "npx -y skills add 'anthropics/skills' --skill 'frontend-design' -a 'claude-code' -a 'cursor' -y"
        );
    }

    #[test]
    fn install_whole_source_globally_without_agents() {
        assert_eq!(
            install_command(&fav("https://github.com/a/b", None), &settings("", true)),
            "npx -y skills add 'https://github.com/a/b' -g -y"
        );
        assert_eq!(
            install_command(&fav("a/b", Some("")), &settings("", false)),
            "npx -y skills add 'a/b' -y"
        );
    }

    #[test]
    fn install_escapes_hostile_values() {
        let cmd = install_command(
            &fav("a/b'; touch /tmp/x; '", Some("$(id)")),
            &settings("pi", false),
        );
        assert_eq!(
            cmd,
            r"npx -y skills add 'a/b'\''; touch /tmp/x; '\''' --skill '$(id)' -a 'pi' -y"
        );
    }

    #[test]
    fn remove_by_name() {
        assert_eq!(
            remove_command(&["tdd", "web design"], &settings("codex", true)).unwrap(),
            "npx -y skills remove 'tdd' 'web design' -a 'codex' -g -y"
        );
        assert_eq!(remove_command(&[], &settings("codex", false)), None);
    }

    #[test]
    fn strips_ansi_sequences() {
        assert_eq!(strip_ansi("\x1B[32m✓\x1B[0m done"), "✓ done");
        assert_eq!(strip_ansi("\x1B[2K\x1B[1Gspinning\x1B[?25l"), "spinning");
        assert_eq!(strip_ansi("\x1B]8;;https://x\x07link\x1B]8;;\x07"), "link");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn run_streams_lines_and_returns_code() {
        let dir = std::env::temp_dir();
        let mut lines = Vec::new();
        let code = run(
            "pwd; echo out; echo err >&2; printf 'a\\rb\\n\\033[31mred\\033[0m\\n'; exit 3",
            Some(&dir),
            |line| lines.push(line.to_string()),
        )
        .unwrap();
        assert_eq!(code, 3);
        let canonical = dir.canonicalize().unwrap();
        assert!(
            Path::new(&lines[0]).canonicalize().unwrap() == canonical,
            "{lines:?}"
        );
        assert_eq!(&lines[1..], ["out", "err", "a", "b", "red"]);
    }

    #[test]
    fn run_reports_missing_npx() {
        let script = format!("echo '{NPX_MISSING}'; exit 127");
        let err = run_script(&script, None, |_| {}).unwrap_err();
        assert!(matches!(err, InstallError::NpxMissing));
    }

    #[test]
    fn run_refuses_check_and_update() {
        for cmd in [
            "npx -y skills check",
            "npx skills update -y",
            "npx  skills   check",
        ] {
            assert!(
                matches!(run(cmd, None, |_| {}), Err(InstallError::Forbidden(_))),
                "{cmd}"
            );
        }
    }

    #[test]
    fn run_missing_dir_is_spawn_error() {
        let err = run("true", Some(Path::new("/nonexistent/loadout")), |_| {}).unwrap_err();
        assert!(matches!(err, InstallError::Spawn(_)));
    }
}
