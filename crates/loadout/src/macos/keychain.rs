//! The GitHub token, kept in the login Keychain (never in data.json). Blocking: the Keychain
//! may ask the user to allow access, so call these off the main thread.

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

const SERVICE: &str = "local.loadout.app";
const ACCOUNT: &str = "github-token";
/// errSecItemNotFound: no token saved yet.
const NOT_FOUND: i32 = -25300;

pub fn get() -> Option<String> {
    let bytes = get_generic_password(SERVICE, ACCOUNT).ok()?;
    String::from_utf8(bytes).ok().filter(|t| !t.is_empty())
}

/// Saves `token`, or deletes it when empty. Errors are phrased for Settings.
pub fn set(token: &str) -> Result<(), String> {
    let result = if token.is_empty() {
        match delete_generic_password(SERVICE, ACCOUNT) {
            Err(err) if err.code() == NOT_FOUND => Ok(()),
            other => other,
        }
    } else {
        set_generic_password(SERVICE, ACCOUNT, token.as_bytes())
    };
    result.map_err(|err| format!("Couldn't save to the Keychain: {err}"))
}
