//! `loadout://` links and files opened with Loadout ("Open With" in Finder). AppKit delivers
//! them before GPUI's App exists, so they wait here until the main loop drains them.

use std::path::Path;
use std::sync::Mutex;

use loadout_core::project::path_from_document;

static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Larger files aren't loadouts (the share format caps at 256 KB).
const MAX_FILE: u64 = 256 * 1024;

pub fn push(urls: Vec<String>) {
    if let Ok(mut opened) = OPENED.lock() {
        opened.extend(urls);
    }
}

/// Opened URLs since the last call, as import text: links as they are, files read.
pub fn take() -> Vec<String> {
    let urls = OPENED
        .lock()
        .map(|mut o| std::mem::take(&mut *o))
        .unwrap_or_default();
    urls.into_iter()
        .filter_map(|url| match url.strip_prefix("file://") {
            Some(_) => read_small(&path_from_document(&url)?),
            None => url.starts_with("loadout:").then_some(url),
        })
        .collect()
}

/// A file's text if it's small enough to be a loadout.
pub fn read_small(path: &Path) -> Option<String> {
    let size = std::fs::metadata(path).ok()?.len();
    (size <= MAX_FILE).then(|| std::fs::read_to_string(path).ok())?
}
