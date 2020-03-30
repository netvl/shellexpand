use std::path::PathBuf;

use redox_users::All;

use super::HomeDirError;

/// Returns the home directory of:
/// * the current user if `user` is `None` or an empty string, or
/// * the provided user if `user` is anything else.
pub(crate) fn home_dir(user: Option<&str>) -> Result<PathBuf, HomeDirError> {
    _home_dir(user).ok_or_else(|| HomeDirError::not_found(user))
}

fn _home_dir(user: Option<&str>) -> Option<PathBuf> {
    let config = redox_users::Config::default();
    let users = redox_users::AllUsers::new(config).ok()?;
    match user {
        None | Some("") => {
            let uid = redox_users::get_uid().ok()?;
            let user = users.get_by_id(uid)?;
            Some(PathBuf::from(user.home.clone()))
        }
        Some(user) => {
            let user = users.get_by_name(user)?;
            Some(PathBuf::from(user.home.clone()))
        }
    }
}
