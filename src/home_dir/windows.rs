use std::path::PathBuf;

use super::HomeDirError;

/// Returns the home directory of:
/// * the current user if `user` is `None` or an empty string,
/// * the Default user if `user` is `Some("Default")`, or
/// * the provided user if `user` is anything else.
pub(crate) fn home_dir(_: Option<&str>) -> Result<PathBuf, HomeDirError> {
    todo!()
}
