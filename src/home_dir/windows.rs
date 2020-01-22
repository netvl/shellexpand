use std::path::PathBuf;

use super::HomeDirError;

/// Returns the home directory of the current user if `user` is `None` or
/// an empty string. In the future, may return the home directory of the
/// provided user if `user` is anything else, but that is not yet implemented
/// for non-unix platforms.
pub(crate) fn home_dir(user: Option<&str>) -> Result<PathBuf, HomeDirError> {
    match user {
        None | Some("") => {
            // When user is `None` or an empty string, let the `dirs` crate
            // do the work.
            dirs::home_dir().ok_or_else(|| HomeDirError::not_found(None))
        }
        Some(user) => {
            // Finding the home directory of a user other than the current
            // user is not yet implemented on windows.
            Err(HomeDirError::Unimplemented),
        }
    }
}
