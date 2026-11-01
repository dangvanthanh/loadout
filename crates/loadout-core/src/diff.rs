//! What changed between two versions of a skill folder (U3): files, and SKILL.md lines.

use similar::{ChangeTag, TextDiff};

use crate::updates::Tree;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
    /// Unchanged lines skipped between hunks.
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub text: String,
}

/// What changed in a skill folder between two versions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub files: Vec<FileChange>,
    /// SKILL.md line diff; empty when SKILL.md didn't change.
    pub skill_md: Vec<DiffLine>,
}

/// Unchanged lines kept around each SKILL.md change.
const CONTEXT_LINES: usize = 2;

/// Files and SKILL.md diff between two listings of the same folder. `blob` reads a file's text
/// by blob SHA; it's only called when SKILL.md changed.
pub fn skill_changes<E>(
    old: &Tree,
    new: &Tree,
    mut blob: impl FnMut(&str) -> Result<String, E>,
) -> Result<Changes, E> {
    let files = file_changes(old, new);
    let skill_md = match files.iter().find(|f| f.path == "SKILL.md") {
        Some(_) => {
            let mut read = |tree: &Tree| match tree.blobs.get("SKILL.md") {
                Some(sha) => blob(sha),
                None => Ok(String::new()),
            };
            let (before, after) = (read(old)?, read(new)?);
            line_diff(&before, &after, CONTEXT_LINES)
        }
        None => Vec::new(),
    };
    Ok(Changes { files, skill_md })
}

/// Compares two listings of the same folder (each fetched at that folder's tree SHA).
/// Sorted by path.
pub fn file_changes(old: &Tree, new: &Tree) -> Vec<FileChange> {
    let mut paths: Vec<&String> = old.blobs.keys().chain(new.blobs.keys()).collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| {
            let kind = match (old.blobs.get(path), new.blobs.get(path)) {
                (None, Some(_)) => ChangeKind::Added,
                (Some(_), None) => ChangeKind::Removed,
                (Some(a), Some(b)) if a != b => ChangeKind::Modified,
                _ => return None,
            };
            Some(FileChange {
                path: path.clone(),
                kind,
            })
        })
        .collect()
}

/// Line diff with `context` unchanged lines around each change.
pub fn line_diff(old: &str, new: &str, context: usize) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    for (i, group) in diff.grouped_ops(context).iter().enumerate() {
        if i > 0 {
            lines.push(DiffLine {
                kind: LineKind::Gap,
                text: String::new(),
            });
        }
        for change in group.iter().flat_map(|op| diff.iter_changes(op)) {
            let kind = match change.tag() {
                ChangeTag::Equal => LineKind::Context,
                ChangeTag::Insert => LineKind::Added,
                ChangeTag::Delete => LineKind::Removed,
            };
            let text = change.value().trim_end_matches(['\n', '\r']).to_string();
            lines.push(DiffLine { kind, text });
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> Tree {
        Tree {
            blobs: files
                .iter()
                .map(|(p, s)| (p.to_string(), s.to_string()))
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn lists_added_removed_modified() {
        let old = tree(&[
            ("SKILL.md", "a"),
            ("LICENSE.txt", "l"),
            ("scripts/old.sh", "o"),
        ]);
        let new = tree(&[
            ("SKILL.md", "b"),
            ("LICENSE.txt", "l"),
            ("scripts/new.py", "n"),
        ]);
        assert_eq!(
            file_changes(&old, &new),
            [
                FileChange {
                    path: "SKILL.md".into(),
                    kind: ChangeKind::Modified
                },
                FileChange {
                    path: "scripts/new.py".into(),
                    kind: ChangeKind::Added
                },
                FileChange {
                    path: "scripts/old.sh".into(),
                    kind: ChangeKind::Removed
                },
            ]
        );
        assert!(file_changes(&old, &old).is_empty());
    }

    #[test]
    fn line_diff_with_context_and_gaps() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let new = "a\nB\nc\nd\ne\nf\ng\nh\ni\n";
        let diff = line_diff(old, new, 1);
        let lines: Vec<(LineKind, &str)> = diff.iter().map(|l| (l.kind, l.text.as_str())).collect();
        assert_eq!(
            lines,
            [
                (LineKind::Context, "a"),
                (LineKind::Removed, "b"),
                (LineKind::Added, "B"),
                (LineKind::Context, "c"),
                (LineKind::Gap, ""),
                (LineKind::Context, "h"),
                (LineKind::Added, "i"),
            ]
        );
    }

    #[test]
    fn skill_changes_reads_skill_md_only_when_it_changed() {
        let old = tree(&[("SKILL.md", "s1"), ("ref.md", "r1")]);
        let new = tree(&[
            ("SKILL.md", "s2"),
            ("ref.md", "r1"),
            ("scripts/run.sh", "n"),
        ]);
        let mut reads = Vec::new();
        let changes = skill_changes(&old, &new, |sha| {
            reads.push(sha.to_string());
            Ok::<_, ()>(match sha {
                "s1" => "# Skill\nold rule\n".to_string(),
                _ => "# Skill\nnew rule\n".to_string(),
            })
        })
        .unwrap();
        assert_eq!(reads, ["s1", "s2"]);
        assert_eq!(changes.files.len(), 2);
        let kinds: Vec<_> = changes.skill_md.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [LineKind::Context, LineKind::Removed, LineKind::Added]
        );

        let unchanged = tree(&[("SKILL.md", "s1"), ("ref.md", "r2")]);
        let changes = skill_changes(&old, &unchanged, |_| -> Result<String, ()> {
            panic!("no reads")
        })
        .unwrap();
        assert!(changes.skill_md.is_empty());
        assert_eq!(
            changes.files,
            [FileChange {
                path: "ref.md".into(),
                kind: ChangeKind::Modified
            }]
        );
    }

    #[test]
    fn skill_changes_new_skill_md_and_errors() {
        let old = tree(&[("README.md", "r")]);
        let new = tree(&[("README.md", "r"), ("SKILL.md", "s")]);
        let changes = skill_changes(&old, &new, |_| Ok::<_, ()>("line\n".to_string())).unwrap();
        assert_eq!(
            changes.skill_md,
            [DiffLine {
                kind: LineKind::Added,
                text: "line".into()
            }]
        );
        assert_eq!(
            skill_changes(&old, &new, |_| Err::<String, _>("offline")),
            Err("offline")
        );
    }

    #[test]
    fn identical_text_has_no_lines() {
        assert!(line_diff("same\n", "same\n", 3).is_empty());
    }
}
