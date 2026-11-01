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
