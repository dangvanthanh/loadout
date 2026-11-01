//! Finding the project you're in (U1).

use std::path::{Path, PathBuf};

/// Turns an `AXDocument` value (a `file://` URL) or a plain absolute path into a path.
pub fn path_from_document(document: &str) -> Option<PathBuf> {
    let path = match document.strip_prefix("file://") {
        Some(rest) => percent_decode(rest.strip_prefix("localhost").unwrap_or(rest))?,
        None => document.to_string(),
    };
    if !path.starts_with('/') {
        return None;
    }
    let trimmed = path.trim_end_matches('/');
    Some(PathBuf::from(if trimmed.is_empty() {
        "/"
    } else {
        trimmed
    }))
}

/// The folder a document lives in: the path itself if it is a directory, else its parent.
/// `None` if the path no longer exists.
pub fn folder_for(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        Some(path.to_path_buf())
    } else if path.exists() {
        path.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

/// Files that mark a project root when there's no `.git`.
const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "Gemfile",
    "composer.json",
    "Package.swift",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "deno.json",
    "mix.exs",
    "pubspec.yaml",
    "skills-lock.json",
];

/// The project root for a folder: the nearest ancestor with `.git`, else the nearest with a
/// manifest, else the folder itself. `home` and its ancestors never count, so a dotfiles
/// repo in `~` doesn't swallow every folder.
pub fn find_root(start: &Path, home: Option<&Path>) -> PathBuf {
    let mut manifest = None;
    for dir in start.ancestors() {
        if home.is_some_and(|home| home.starts_with(dir)) {
            break;
        }
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        if manifest.is_none() && has_manifest(dir) {
            manifest = Some(dir.to_path_buf());
        }
    }
    manifest.unwrap_or_else(|| start.to_path_buf())
}

fn has_manifest(dir: &Path) -> bool {
    MANIFESTS.iter().any(|name| dir.join(name).exists())
        || std::fs::read_dir(dir).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "xcodeproj")
            })
        })
}

fn percent_decode(s: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(s.len());
    let mut rest = s.as_bytes();
    while let Some((&b, tail)) = rest.split_first() {
        if b == b'%' {
            let hex = std::str::from_utf8(tail.get(..2)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &tail[2..];
        } else {
            bytes.push(b);
            rest = tail;
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url() {
        assert_eq!(
            path_from_document("file:///Users/me/Code/shop/src/app.tsx"),
            Some(PathBuf::from("/Users/me/Code/shop/src/app.tsx"))
        );
    }

    #[test]
    fn file_url_percent_encoded_and_localhost() {
        assert_eq!(
            path_from_document("file://localhost/Users/me/My%20Proj/caf%C3%A9/"),
            Some(PathBuf::from("/Users/me/My Proj/café"))
        );
    }

    #[test]
    fn plain_path() {
        assert_eq!(
            path_from_document("/Users/me/Code/shop"),
            Some(PathBuf::from("/Users/me/Code/shop"))
        );
    }

    #[test]
    fn rejects_other_urls_and_bad_encoding() {
        assert_eq!(path_from_document("https://example.com/a"), None);
        assert_eq!(path_from_document("untitled"), None);
        assert_eq!(path_from_document(""), None);
        assert_eq!(path_from_document("file:///a/%ZZ"), None);
        assert_eq!(path_from_document("file:///a/%FF"), None);
    }

    /// A fresh temp tree (canonical, so it compares equal to walked paths).
    fn tree(name: &str, dirs: &[&str], files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("loadout-root-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        for dir in dirs {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        root
    }

    #[test]
    fn root_is_nearest_git_over_nested_manifests() {
        let t = tree(
            "git",
            &["repo/.git", "repo/packages/app/src"],
            &["repo/packages/app/package.json"],
        );
        assert_eq!(
            find_root(&t.join("repo/packages/app/src"), None),
            t.join("repo")
        );
        // A worktree or submodule has a `.git` file instead of a folder.
        let w = tree("worktree", &["wt/src"], &["wt/.git"]);
        assert_eq!(find_root(&w.join("wt/src"), None), w.join("wt"));
    }

    #[test]
    fn root_without_git_is_nearest_manifest() {
        let t = tree(
            "manifest",
            &["app/src/components", "ios/App.xcodeproj/x"],
            &["app/Cargo.toml"],
        );
        assert_eq!(
            find_root(&t.join("app/src/components"), None),
            t.join("app")
        );
        assert_eq!(find_root(&t.join("ios"), None), t.join("ios"));
    }

    #[test]
    fn root_without_markers_is_the_folder() {
        let t = tree("plain", &["notes/2026"], &[]);
        assert_eq!(find_root(&t.join("notes/2026"), None), t.join("notes/2026"));
    }

    #[test]
    fn root_ignores_dotfiles_repo_in_home() {
        let home = tree(
            "home",
            &[".git", "Code/notes", "Code/shop/src"],
            &["Code/shop/package.json"],
        );
        assert_eq!(
            find_root(&home.join("Code/notes"), Some(&home)),
            home.join("Code/notes")
        );
        assert_eq!(
            find_root(&home.join("Code/shop/src"), Some(&home)),
            home.join("Code/shop")
        );
        assert_eq!(find_root(&home, Some(&home)), home);
    }

    #[test]
    fn folder_of_file_and_dir() {
        let dir = std::env::temp_dir().join(format!("loadout-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("main.rs");
        std::fs::write(&file, "").unwrap();

        assert_eq!(folder_for(&file), Some(dir.clone()));
        assert_eq!(folder_for(&dir), Some(dir.clone()));
        assert_eq!(folder_for(&dir.join("missing.rs")), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
