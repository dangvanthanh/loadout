//! Lightweight "what you're working on" signals for ranking (U2, DECISIONS 016).
//! Best-effort: every source may be missing; nothing here blocks or fails loudly.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

/// Folders never worth walking.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "vendor",
    "Pods",
    ".venv",
    "venv",
    "__pycache__",
    ".turbo",
    "coverage",
    "DerivedData",
];
const MAX_DEPTH: usize = 4;
const MAX_ENTRIES: usize = 3000;
const RECENT: usize = 20;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    /// Relative to the root: the open document first, then git changes, then recent files.
    pub files: Vec<String>,
    /// `git status` reported changes.
    pub dirty: bool,
}

/// Collects signals for `root`. `document` is the editor's open file, if known.
pub fn gather(root: &Path, document: Option<&Path>) -> Context {
    if !root.is_dir() {
        return Context::default();
    }
    let mut files = Vec::new();
    if let Some(doc) = document.and_then(|doc| doc.strip_prefix(root).ok()) {
        files.push(doc.to_string_lossy().into_owned());
    }
    let changed = git_changes(root);
    let dirty = !changed.is_empty();
    files.extend(
        changed
            .into_iter()
            .filter(|f| !in_skipped_dir(Path::new(f))),
    );
    files.extend(recent_files(root));

    let mut seen = std::collections::HashSet::new();
    files.retain(|f| seen.insert(f.clone()));
    Context { files, dirty }
}

fn git_changes(root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(out) if out.status.success() => parse_porcelain(&String::from_utf8_lossy(&out.stdout)),
        _ => Vec::new(),
    }
}

/// The most recently modified files in a bounded, shallow walk.
fn recent_files(root: &Path) -> Vec<String> {
    let mut found: Vec<(SystemTime, PathBuf)> = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0)];
    let mut visited = 0;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_ENTRIES {
                break;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if depth < MAX_DEPTH && !is_skipped_dir(&entry.file_name()) {
                    stack.push((path, depth + 1));
                }
            } else if let Ok(modified) = meta.modified() {
                found.push((modified, path));
            }
        }
    }
    // Newest first; path breaks ties so the order is deterministic.
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    found
        .into_iter()
        .take(RECENT)
        .filter_map(|(_, path)| Some(path.strip_prefix(root).ok()?.to_string_lossy().into_owned()))
        .collect()
}

/// Whether a file sits inside a folder `is_skipped_dir` rejects.
fn in_skipped_dir(path: &Path) -> bool {
    path.parent()
        .is_some_and(|dir| dir.components().any(|c| is_skipped_dir(c.as_os_str())))
}

/// Build output, dependencies, and hidden tool folders (`.claude/skills` holds installed skills,
/// not the user's work). `.github` stays: CI config is a signal.
fn is_skipped_dir(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    SKIP_DIRS.contains(&name.as_ref()) || (name.starts_with('.') && name != ".github")
}

/// `git status --porcelain` lines → paths (rename targets for `R  old -> new`).
pub fn parse_porcelain(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|path| path.rsplit(" -> ").next().unwrap_or(path))
        // shortcut: drops quotes but doesn't unescape git's octal escapes for unusual names.
        .map(|path| path.trim_matches('"').to_string())
        .filter(|path| !path.is_empty())
        .collect()
}

/// Context tags a file suggests, e.g. `src/Button.test.tsx` → testing, frontend.
pub fn tags_for_path(path: &str) -> Vec<&'static str> {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    let in_dir =
        |dir: &str| lower.starts_with(&format!("{dir}/")) || lower.contains(&format!("/{dir}/"));

    let mut tags = Vec::new();
    if name.contains(".test.")
        || name.contains(".spec.")
        || name.starts_with("test_")
        || ["tests", "test", "__tests__", "spec"]
            .iter()
            .any(|d| in_dir(d))
    {
        tags.push("testing");
    }
    let by_ext = match ext {
        "tsx" | "jsx" | "css" | "scss" | "sass" | "less" | "vue" | "svelte" | "html" | "astro" => {
            Some("frontend")
        }
        "sql" | "prisma" => Some("database"),
        "swift" => Some("swift"),
        "rs" => Some("rust"),
        "py" => Some("python"),
        "go" => Some("go"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "dart" => Some("dart"),
        "md" | "mdx" => Some("docs"),
        "tf" => Some("devops"),
        _ => None,
    };
    tags.extend(by_ext);
    if in_dir("migrations") && !tags.contains(&"database") {
        tags.push("database");
    }
    if (in_dir(".github/workflows")
        || matches!(name, "dockerfile" | "compose.yaml" | "docker-compose.yml"))
        && !tags.contains(&"devops")
    {
        tags.push("devops");
    }
    tags
}

/// Context tags over all files, first occurrence wins, with the file as evidence.
pub fn context_tags(context: &Context) -> Vec<(&'static str, String)> {
    let mut tags: Vec<(&'static str, String)> = Vec::new();
    for file in &context.files {
        for tag in tags_for_path(file) {
            if !tags.iter().any(|(seen, _)| *seen == tag) {
                tags.push((tag, file.clone()));
            }
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_paths() {
        let out = " M src/app/page.tsx\n?? tests/login.spec.ts\nR  old.sql -> db/migrations/001.sql\nA  \"docs/my notes.md\"\n\n";
        assert_eq!(
            parse_porcelain(out),
            [
                "src/app/page.tsx",
                "tests/login.spec.ts",
                "db/migrations/001.sql",
                "docs/my notes.md"
            ]
        );
    }

    #[test]
    fn path_tags() {
        assert_eq!(
            tags_for_path("src/Button.test.tsx"),
            ["testing", "frontend"]
        );
        assert_eq!(tags_for_path("tests/api.py"), ["testing", "python"]);
        assert_eq!(tags_for_path("prisma/schema.prisma"), ["database"]);
        assert_eq!(tags_for_path("db/migrations/001.sql"), ["database"]);
        assert_eq!(tags_for_path(".github/workflows/ci.yml"), ["devops"]);
        assert_eq!(tags_for_path("Dockerfile"), ["devops"]);
        assert_eq!(tags_for_path("docs/guide.mdx"), ["docs"]);
        assert_eq!(tags_for_path("Sources/App/View.swift"), ["swift"]);
        assert!(tags_for_path("Cargo.lock").is_empty());
    }

    #[test]
    fn context_tags_dedupe_in_order() {
        let ctx = Context {
            files: vec![
                "src/a.test.ts".into(),
                "src/b.spec.ts".into(),
                "app.css".into(),
            ],
            dirty: true,
        };
        assert_eq!(
            context_tags(&ctx),
            [
                ("testing", "src/a.test.ts".to_string()),
                ("frontend", "app.css".to_string())
            ]
        );
    }

    #[test]
    fn gather_from_git_repo_document_first() {
        let root = std::env::temp_dir().join(format!("loadout-context-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("node_modules/pkg/index.test.js"), "").unwrap();
        std::fs::write(root.join("src/app.tsx"), "").unwrap();
        std::fs::write(root.join("schema.sql"), "").unwrap();
        // Installed skills and tool folders aren't the user's work; CI config is.
        std::fs::create_dir_all(root.join(".claude/skills/x")).unwrap();
        std::fs::write(root.join(".claude/skills/x/SKILL.md"), "").unwrap();
        std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
        std::fs::write(root.join(".github/workflows/ci.yml"), "").unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&root)
                .stdout(Stdio::null())
                .status()
                .unwrap()
        };
        git(&["init", "-q"]);

        let ctx = gather(&root, Some(&root.join("src/app.tsx")));
        assert!(ctx.dirty);
        assert_eq!(ctx.files[0], "src/app.tsx");
        assert!(
            ctx.files.contains(&"schema.sql".to_string()),
            "{:?}",
            ctx.files
        );
        assert!(
            !ctx.files.iter().any(|f| f.contains("node_modules")),
            "{:?}",
            ctx.files
        );
        assert!(
            !ctx.files.iter().any(|f| f.starts_with(".claude")),
            "{:?}",
            ctx.files
        );
        assert!(
            ctx.files.contains(&".github/workflows/ci.yml".to_string()),
            "{:?}",
            ctx.files
        );
        assert_eq!(ctx.files.iter().filter(|f| *f == "src/app.tsx").count(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn gather_without_git_or_folder_is_quiet() {
        let ctx = gather(Path::new("/nonexistent/loadout"), None);
        assert_eq!(ctx, Context::default());
    }
}
