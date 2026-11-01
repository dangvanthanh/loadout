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
    fn identical_text_has_no_lines() {
        assert!(line_diff("same\n", "same\n", 3).is_empty());
    }
}
