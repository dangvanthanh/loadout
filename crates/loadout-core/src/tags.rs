//! Tags for favorites, and ranking against stack + context (U2). Deterministic:
//! stack matches first, then context boosts, then the rest, ties in the user's order.

use crate::context::Context;
use crate::model::Favorite;
use crate::stack::StackTag;

/// Name words → tags, for favorites saved without tags (e.g. imported from the prototype).
const KEYWORDS: &[(&str, &[&str])] = &[
    ("react", &["react"]),
    ("next", &["nextjs", "react"]),
    ("nextjs", &["nextjs", "react"]),
    ("vercel", &["nextjs"]),
    ("vue", &["vue"]),
    ("nuxt", &["nuxt", "vue"]),
    ("svelte", &["svelte"]),
    ("angular", &["angular"]),
    ("tailwind", &["tailwind", "frontend"]),
    ("frontend", &["frontend"]),
    ("ui", &["frontend"]),
    ("ux", &["frontend"]),
    ("design", &["frontend"]),
    ("css", &["frontend"]),
    ("web", &["frontend"]),
    ("animation", &["frontend"]),
    ("animations", &["frontend"]),
    ("accessibility", &["frontend"]),
    ("tdd", &["testing"]),
    ("test", &["testing"]),
    ("tests", &["testing"]),
    ("testing", &["testing"]),
    ("playwright", &["testing", "playwright"]),
    ("vitest", &["testing", "vitest"]),
    ("jest", &["testing", "jest"]),
    ("swift", &["swift"]),
    ("swiftui", &["swift"]),
    ("ios", &["swift"]),
    ("rust", &["rust"]),
    ("cargo", &["rust"]),
    ("gpui", &["gpui", "rust"]),
    ("python", &["python"]),
    ("django", &["django", "python"]),
    ("fastapi", &["fastapi", "python"]),
    ("go", &["go"]),
    ("golang", &["go"]),
    ("rails", &["rails", "ruby"]),
    ("ruby", &["ruby"]),
    ("laravel", &["laravel", "php"]),
    ("flutter", &["flutter", "dart"]),
    ("database", &["database"]),
    ("sql", &["database"]),
    ("postgres", &["database"]),
    ("supabase", &["database"]),
    ("prisma", &["database", "prisma"]),
    ("drizzle", &["database", "drizzle"]),
    ("docker", &["devops", "docker"]),
    ("deploy", &["devops"]),
    ("ci", &["devops"]),
    ("kubernetes", &["devops"]),
    ("docs", &["docs"]),
    ("writing", &["docs"]),
    ("typescript", &["typescript"]),
];

/// Stack tags imply broader ones: a React app is a frontend project.
const IMPLIED: &[(&str, &[&str])] = &[
    ("nextjs", &["frontend"]),
    ("nuxt", &["frontend"]),
    ("sveltekit", &["frontend", "svelte"]),
    ("astro", &["frontend"]),
    ("remix", &["frontend"]),
    ("react", &["frontend"]),
    ("react-native", &["frontend"]),
    ("expo", &["frontend"]),
    ("vue", &["frontend"]),
    ("svelte", &["frontend"]),
    ("angular", &["frontend"]),
    ("solid", &["frontend"]),
    ("tailwind", &["frontend"]),
    ("prisma", &["database"]),
    ("drizzle", &["database"]),
    ("vitest", &["testing"]),
    ("jest", &["testing"]),
    ("playwright", &["testing"]),
    ("docker", &["devops"]),
    ("gpui", &["rust"]),
];

/// Guessed from the favorite's name, skill, and source words; in table order, no duplicates.
pub fn suggest_tags(favorite: &Favorite) -> Vec<String> {
    let text = [
        favorite.name.as_str(),
        favorite.skill.as_deref().unwrap_or(""),
        &favorite.source,
    ]
    .join(" ")
    .to_ascii_lowercase();
    let words: Vec<&str> = text.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    let mut tags: Vec<String> = Vec::new();
    for (keyword, implied) in KEYWORDS {
        if words.contains(keyword) {
            for tag in *implied {
                if !tags.iter().any(|t| t == tag) {
                    tags.push(tag.to_string());
                }
            }
        }
    }
    tags
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// Index into the ranked slice.
    pub index: usize,
    pub score: u32,
    /// The first stack tag that matched; such items are pre-selected.
    pub stack_match: Option<&'static str>,
    /// The first context tag that matched.
    pub context_match: Option<&'static str>,
}

impl Suggestion {
    pub fn preselect(&self) -> bool {
        self.stack_match.is_some()
    }
}

fn stack_with_implied(stack: &[StackTag]) -> Vec<&'static str> {
    let mut tags: Vec<&'static str> = Vec::new();
    for tag in stack.iter().map(|t| t.tag) {
        let implied = IMPLIED
            .iter()
            .find(|(t, _)| *t == tag)
            .map_or(&[][..], |(_, i)| *i);
        for tag in std::iter::once(tag).chain(implied.iter().copied()) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

/// Stack matches count 100 each, context matches 10 each.
fn score_against(
    tags: &[String],
    stack: &[&'static str],
    context: &[&'static str],
) -> (u32, Option<&'static str>, Option<&'static str>) {
    let mut tags: Vec<String> = tags.iter().map(|t| t.to_ascii_lowercase()).collect();
    tags.dedup();
    let matches = |set: &[&'static str]| -> Vec<&'static str> {
        tags.iter()
            .filter_map(|t| set.iter().find(|s| **s == t.as_str()).copied())
            .collect()
    };
    let (in_stack, in_context) = (matches(stack), matches(context));
    let score = 100 * in_stack.len() as u32 + 10 * in_context.len() as u32;
    (
        score,
        in_stack.first().copied(),
        in_context.first().copied(),
    )
}

/// Ranks favorites; untagged ones use `suggest_tags`.
pub fn rank_favorites(
    favorites: &[Favorite],
    stack: &[StackTag],
    context: &Context,
) -> Vec<Suggestion> {
    rank(
        favorites.iter().map(|f| {
            if f.tags.is_empty() {
                suggest_tags(f)
            } else {
                f.tags.clone()
            }
        }),
        stack,
        context,
    )
}

/// Ranks any tag lists (favorites, loadouts). Stable: equal scores keep input order.
pub fn rank(
    items: impl IntoIterator<Item = Vec<String>>,
    stack: &[StackTag],
    context: &Context,
) -> Vec<Suggestion> {
    let stack_tags = stack_with_implied(stack);
    let context_tags: Vec<&'static str> = crate::context::context_tags(context)
        .into_iter()
        .map(|(tag, _)| tag)
        .collect();
    let mut ranked: Vec<Suggestion> = items
        .into_iter()
        .enumerate()
        .map(|(index, tags)| {
            let (score, stack_match, context_match) =
                score_against(&tags, &stack_tags, &context_tags);
            Suggestion {
                index,
                score,
                stack_match,
                context_match,
            }
        })
        .collect();
    // Stable: equal scores keep the user's order.
    ranked.sort_by_key(|s| std::cmp::Reverse(s.score));
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::favorite;

    fn tagged(name: &str, tags: &[&str]) -> Favorite {
        let mut f = favorite(name, "acme/skills", Some(name));
        f.tags = tags.iter().map(|t| t.to_string()).collect();
        f
    }

    fn stack(tags: &[&'static str]) -> Vec<StackTag> {
        tags.iter()
            .map(|tag| StackTag {
                tag,
                evidence: String::new(),
            })
            .collect()
    }

    fn names<'a>(favs: &'a [Favorite], ranked: &[Suggestion]) -> Vec<&'a str> {
        ranked.iter().map(|s| favs[s.index].name.as_str()).collect()
    }

    #[test]
    fn suggests_tags_from_words() {
        let f = favorite(
            "vercel-react-best-practices",
            "vercel-labs/agent-skills",
            Some("vercel-react-best-practices"),
        );
        assert_eq!(suggest_tags(&f), ["react", "nextjs"]);
        assert_eq!(
            suggest_tags(&favorite("tdd", "mattpocock/skills", Some("tdd"))),
            ["testing"]
        );
        assert_eq!(
            suggest_tags(&favorite("frontend-design", "anthropics/skills", None)),
            ["frontend"]
        );
        assert_eq!(
            suggest_tags(&favorite("SwiftUI_Patterns", "x/y", None)),
            ["swift"]
        );
        assert!(suggest_tags(&favorite("grill-me", "mattpocock/skills", None)).is_empty());
    }

    #[test]
    fn stack_first_then_context_then_rest() {
        let favs = vec![
            tagged("grill-me", &[]),
            tagged("tdd", &["testing"]),
            tagged("swiftui", &["swift"]),
            tagged("postgres", &["database"]),
            tagged("frontend-design", &["frontend"]),
            tagged("next-best", &["nextjs", "react"]),
        ];
        let stack = stack(&["nextjs", "react", "tailwind", "typescript", "node"]);
        let context = Context {
            files: vec!["tests/login.spec.ts".into()],
            dirty: true,
        };

        let ranked = rank_favorites(&favs, &stack, &context);
        assert_eq!(
            names(&favs, &ranked),
            [
                "next-best",
                "frontend-design",
                "tdd",
                "grill-me",
                "swiftui",
                "postgres"
            ]
        );
        assert_eq!(ranked[0].score, 200);
        assert_eq!(ranked[0].stack_match, Some("nextjs"));
        assert_eq!(ranked[1].stack_match, Some("frontend"));
        assert!(ranked[1].preselect());
        assert_eq!(ranked[2].context_match, Some("testing"));
        assert!(!ranked[2].preselect());
        assert_eq!(ranked[3].score, 0);
    }

    #[test]
    fn context_boost_breaks_stack_ties() {
        let favs = vec![
            tagged("a", &["frontend"]),
            tagged("b", &["frontend", "testing"]),
        ];
        let context = Context {
            files: vec!["src/x.test.tsx".into()],
            dirty: false,
        };
        let ranked = rank_favorites(&favs, &stack(&["react"]), &context);
        assert_eq!(names(&favs, &ranked), ["b", "a"]);
        // b: frontend in stack (100) + frontend and testing in context (10 + 10); a: 100 + 10.
        assert_eq!((ranked[0].score, ranked[1].score), (120, 110));
    }

    #[test]
    fn no_signals_keeps_user_order() {
        let favs = vec![
            tagged("a", &["swift"]),
            tagged("b", &[]),
            tagged("c", &["rust"]),
        ];
        let ranked = rank_favorites(&favs, &[], &Context::default());
        assert_eq!(names(&favs, &ranked), ["a", "b", "c"]);
        assert!(ranked.iter().all(|s| s.score == 0 && !s.preselect()));
    }

    #[test]
    fn untagged_favorites_use_suggested_tags() {
        let favs = vec![
            favorite("grill-me", "m/s", None),
            favorite("vercel-react-best-practices", "v/a", None),
        ];
        let ranked = rank_favorites(&favs, &stack(&["nextjs", "react"]), &Context::default());
        assert_eq!(ranked[0].index, 1);
        assert!(ranked[0].preselect());
    }

    #[test]
    fn tags_match_case_insensitively() {
        let favs = vec![tagged("x", &["NextJS"])];
        let ranked = rank_favorites(&favs, &stack(&["nextjs"]), &Context::default());
        assert_eq!(ranked[0].score, 100);
    }
}
