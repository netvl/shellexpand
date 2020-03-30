use std::path::PathBuf;

use super::{HomeDirError, HomeDirErrorKind};

/// Returns the home directory of the current user if `user` is `None` or
/// an empty string.
///
/// In the future, may also return the home directory of the provided user if
/// `user` is anything else, but that is not currently implemented for this
/// platform.
pub(crate) fn home_dir(user: Option<&str>) -> Result<PathBuf, HomeDirError> {
    match user {
        None | Some("") => dirs::home_dir().ok_or_else(|| HomeDirError::not_found(None)),
        Some(_user) => Err(HomeDirError(HomeDirErrorKind::Unimplemented)),
    }
}
