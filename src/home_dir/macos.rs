use std::path::PathBuf;

use super::HomeDirError;

/// Returns the home directory of:
/// * the current user if `user` is `None` or an empty string, or
/// * the provided user if `user` is anything else.
pub(crate) fn home_dir(user: Option<&str>) -> Result<PathBuf, HomeDirError> {
    let user = match user {
        None | Some("") => {
            // When user is `None` or an empty string, let the `dirs` crate
            // do the work.
            dirs::home_dir().ok_or_else(|| HomeDirError::not_found(None))
        }
        Some(user) => user,
    };
    todo!()
}
