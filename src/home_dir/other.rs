use std::env;
use std::path::PathBuf;

use super::HomeDirError;

/// If `user` is `None` or an empty string, returns the value of the `$HOME`,
/// environment variable, if any. Otherwise, returns None.
pub(crate) fn home_dir(user: Option<&str>) -> Result<PathBuf, HomeDirError> {
    if user.is_none() || user == Some("") {
        if let Some(o) = env::var_os("HOME") {
            if !o.is_empty() {
                return Ok(PathBuf::from(o));
            }
        }
    }
    Err(HomeDirError::not_found(user))
}
