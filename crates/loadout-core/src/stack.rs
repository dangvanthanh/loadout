//! Stack tags for a project root, each with the evidence that produced it (U2).
//! Heuristics over manifest files; deterministic order: frameworks and libraries first, then
//! the language, per ecosystem.
// shortcut: reads only the root's manifests; scan workspace members if monorepos rank poorly.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackTag {
    pub tag: &'static str,
    /// Why, e.g. "next in package.json".
    pub evidence: String,
}

/// Tag → display label.
const LABELS: &[(&str, &str)] = &[
    ("nextjs", "Next.js"),
    ("nuxt", "Nuxt"),
    ("sveltekit", "SvelteKit"),
    ("astro", "Astro"),
    ("remix", "Remix"),
    ("expo", "Expo"),
    ("react-native", "React Native"),
    ("react", "React"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("angular", "Angular"),
    ("solid", "Solid"),
    ("electron", "Electron"),
    ("nestjs", "NestJS"),
    ("express", "Express"),
    ("hono", "Hono"),
    ("tailwind", "Tailwind"),
    ("prisma", "Prisma"),
    ("drizzle", "Drizzle"),
    ("vite", "Vite"),
    ("vitest", "Vitest"),
    ("jest", "Jest"),
    ("playwright", "Playwright"),
    ("storybook", "Storybook"),
    ("typescript", "TypeScript"),
    ("node", "Node.js"),
    ("deno", "Deno"),
    ("gpui", "GPUI"),
    ("tauri", "Tauri"),
    ("bevy", "Bevy"),
    ("leptos", "Leptos"),
    ("dioxus", "Dioxus"),
    ("axum", "Axum"),
    ("actix", "Actix"),
    ("tokio", "Tokio"),
    ("rust", "Rust"),
    ("django", "Django"),
    ("fastapi", "FastAPI"),
    ("flask", "Flask"),
    ("pytorch", "PyTorch"),
    ("streamlit", "Streamlit"),
    ("python", "Python"),
    ("gin", "Gin"),
    ("go", "Go"),
    ("rails", "Rails"),
    ("ruby", "Ruby"),
    ("laravel", "Laravel"),
    ("php", "PHP"),
    ("flutter", "Flutter"),
    ("dart", "Dart"),
    ("swift", "Swift"),
    ("docker", "Docker"),
];

/// package.json dependency → tag; a trailing `*` matches a prefix.
const NPM: &[(&str, &str)] = &[
    ("next", "nextjs"),
    ("nuxt", "nuxt"),
    ("@sveltejs/kit", "sveltekit"),
    ("astro", "astro"),
    ("@remix-run/react", "remix"),
    ("expo", "expo"),
    ("react-native", "react-native"),
    ("react", "react"),
    ("vue", "vue"),
    ("svelte", "svelte"),
    ("@angular/core", "angular"),
    ("solid-js", "solid"),
    ("electron", "electron"),
    ("@nestjs/core", "nestjs"),
    ("express", "express"),
    ("hono", "hono"),
    ("tailwindcss", "tailwind"),
    ("prisma", "prisma"),
    ("@prisma/client", "prisma"),
    ("drizzle-orm", "drizzle"),
    ("vite", "vite"),
    ("vitest", "vitest"),
    ("jest", "jest"),
    ("@playwright/test", "playwright"),
    ("storybook", "storybook"),
    ("@storybook/*", "storybook"),
    ("typescript", "typescript"),
];

const CARGO: &[(&str, &str)] = &[
    ("gpui", "gpui"),
    ("gpui-kit", "gpui"),
    ("gpui-component", "gpui"),
    ("tauri", "tauri"),
    ("bevy", "bevy"),
    ("leptos", "leptos"),
    ("dioxus", "dioxus"),
    ("axum", "axum"),
    ("actix-web", "actix"),
    ("tokio", "tokio"),
];

/// Package names matched as whole words in Python manifests.
const PYTHON: &[(&str, &str)] = &[
    ("django", "django"),
    ("fastapi", "fastapi"),
    ("flask", "flask"),
    ("torch", "pytorch"),
    ("streamlit", "streamlit"),
];

const PYTHON_FILES: &[&str] = &["pyproject.toml", "requirements.txt", "setup.py", "Pipfile"];

static CARGO_DEP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z0-9_-]+)\s*(?:=|\.)").expect("valid regex"));
static RAILS_GEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"gem\s+["']rails["']"#).expect("valid regex"));

#[derive(Default)]
struct Found(Vec<StackTag>);

impl Found {
    fn add(&mut self, tag: &'static str, evidence: impl Into<String>) {
        if !self.0.iter().any(|t| t.tag == tag) {
            self.0.push(StackTag {
                tag,
                evidence: evidence.into(),
            });
        }
    }
}

pub fn detect(root: &Path) -> Vec<StackTag> {
    let read = |name: &str| std::fs::read_to_string(root.join(name)).ok();
    let exists = |name: &str| root.join(name).exists();
    let mut found = Found::default();

    if let Some(text) = read("package.json") {
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let deps: Vec<&str> = ["dependencies", "devDependencies", "peerDependencies"]
            .iter()
            .filter_map(|key| json.get(key)?.as_object())
            .flat_map(|deps| deps.keys().map(String::as_str))
            .collect();
        for (pattern, tag) in NPM {
            let hit = deps.iter().find(|dep| match pattern.strip_suffix('*') {
                Some(prefix) => dep.starts_with(prefix),
                None => *dep == pattern,
            });
            if let Some(dep) = hit {
                found.add(tag, format!("{dep} in package.json"));
            }
        }
        if exists("tsconfig.json") {
            found.add("typescript", "tsconfig.json");
        }
        for config in [
            "tailwind.config.js",
            "tailwind.config.ts",
            "tailwind.config.mjs",
            "tailwind.config.cjs",
        ] {
            if exists(config) {
                found.add("tailwind", config);
            }
        }
        found.add("node", "package.json");
    }
    if exists("deno.json") || exists("deno.jsonc") {
        found.add("deno", "deno.json");
    }

    if let Some(text) = read("Cargo.toml") {
        let deps = cargo_dependencies(&text);
        for (name, tag) in CARGO {
            if deps.contains(name) {
                found.add(tag, format!("{name} in Cargo.toml"));
            }
        }
        found.add("rust", "Cargo.toml");
    }

    let python: Vec<(&str, String)> = PYTHON_FILES
        .iter()
        .filter_map(|name| Some((*name, read(name)?)))
        .collect();
    for (package, tag) in PYTHON {
        if let Some((file, _)) = python.iter().find(|(_, text)| has_word(text, package)) {
            found.add(tag, format!("{package} in {file}"));
        }
    }
    if let Some((file, _)) = python.first() {
        found.add("python", *file);
    }

    if let Some(text) = read("go.mod") {
        if text.contains("github.com/gin-gonic/gin") {
            found.add("gin", "gin in go.mod");
        }
        found.add("go", "go.mod");
    }
    if let Some(text) = read("Gemfile") {
        if RAILS_GEM.is_match(&text) {
            found.add("rails", "rails in Gemfile");
        }
        found.add("ruby", "Gemfile");
    }
    if let Some(text) = read("composer.json") {
        if text.contains("\"laravel/framework\"") {
            found.add("laravel", "laravel/framework in composer.json");
        }
        found.add("php", "composer.json");
    }
    if let Some(text) = read("pubspec.yaml") {
        if text.lines().any(|line| line.trim() == "flutter:") {
            found.add("flutter", "flutter in pubspec.yaml");
        }
        found.add("dart", "pubspec.yaml");
    }
    if exists("Package.swift") {
        found.add("swift", "Package.swift");
    } else if let Some(project) = xcodeproj(root) {
        found.add("swift", project);
    }
    for file in ["Dockerfile", "compose.yaml", "docker-compose.yml"] {
        if exists(file) {
            found.add("docker", file);
        }
    }
    found.0
}

/// Names under `[dependencies]`, `[dev-dependencies]`, `[workspace.dependencies]`,
/// `[target.….dependencies]`, and `[dependencies.<name>]` headers.
fn cargo_dependencies(text: &str) -> Vec<&str> {
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(header) = trimmed.strip_prefix('[') {
            let header = header.trim_end_matches(']');
            in_deps = header.ends_with("dependencies");
            if let Some((_, name)) = header.split_once("dependencies.") {
                deps.push(name);
            }
        } else if in_deps && let Some(caps) = CARGO_DEP.captures(line) {
            deps.push(caps.get(1).map_or("", |m| m.as_str()));
        }
    }
    deps
}

/// Case-insensitive; `-`, `_`, and alphanumerics continue a name ("djangorestframework" isn't "django").
fn has_word(text: &str, word: &str) -> bool {
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    let text = text.to_ascii_lowercase();
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_name) && !after.is_some_and(is_name)
    })
}

fn xcodeproj(root: &Path) -> Option<String> {
    std::fs::read_dir(root).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        name.ends_with(".xcodeproj").then_some(name)
    })
}

pub fn label(tag: &str) -> &str {
    LABELS
        .iter()
        .find(|(t, _)| *t == tag)
        .map_or(tag, |(_, label)| label)
}

/// "Next.js, React" — the first `max` labels, for the Into line.
pub fn summary(tags: &[StackTag], max: usize) -> String {
    tags.iter()
        .take(max)
        .map(|t| label(t.tag))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(fixture: &str) -> Vec<&'static str> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/projects")
            .join(fixture);
        detect(&root).into_iter().map(|t| t.tag).collect()
    }

    #[test]
    fn nextjs_app() {
        assert_eq!(
            tags("nextjs-app"),
            [
                "nextjs",
                "react",
                "tailwind",
                "prisma",
                "playwright",
                "storybook",
                "typescript",
                "node"
            ]
        );
    }

    #[test]
    fn vite_vue() {
        assert_eq!(tags("vite-vue"), ["vue", "vite", "vitest", "node"]);
    }

    #[test]
    fn rust_gpui_reads_only_dependency_sections() {
        assert_eq!(tags("rust-gpui"), ["gpui", "tokio", "rust"]);
    }

    #[test]
    fn python_django_matches_whole_words() {
        assert_eq!(tags("python-django"), ["django", "python"]);
    }

    #[test]
    fn go_and_ruby() {
        assert_eq!(tags("go-gin"), ["gin", "go"]);
        assert_eq!(tags("rails-app"), ["rails", "ruby", "docker"]);
    }

    #[test]
    fn plain_folder_and_missing_folder() {
        assert!(tags("plain").is_empty());
        assert!(detect(Path::new("/nonexistent/loadout")).is_empty());
    }

    #[test]
    fn evidence_names_the_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/nextjs-app");
        let found = detect(&root);
        assert_eq!(found[0].evidence, "next in package.json");
        assert_eq!(
            found
                .iter()
                .find(|t| t.tag == "typescript")
                .unwrap()
                .evidence,
            "typescript in package.json"
        );
        let rust = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/rust-gpui");
        assert_eq!(detect(&rust)[0].evidence, "gpui-kit in Cargo.toml");
    }

    #[test]
    fn labels_and_summary() {
        assert_eq!(label("nextjs"), "Next.js");
        assert_eq!(label("mystery"), "mystery");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/nextjs-app");
        assert_eq!(summary(&detect(&root), 2), "Next.js, React");
        assert_eq!(summary(&[], 2), "");
    }
}
