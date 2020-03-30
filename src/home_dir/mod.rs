/*
 * *******************
 * OS-specific modules
 * *******************
*/

// nix (with libc feature)
#[cfg(all(unix, not(target_os = "redox"), feature = "libc"))]
mod nix;

// redox
#[cfg(target_os = "redox")]
mod redox;

// windows
#[cfg(windows)]
mod windows;

// all others
#[cfg(not(any(
    all(unix, not(target_os = "redox"), feature = "libc"),
    target_os = "redox",
    windows,
)))]
mod other;

/*
 * *******************
 * OS-specific exports
 * *******************
*/

// nix (with libc feature enabled)
#[cfg(all(unix, not(target_os = "redox"), feature = "libc"))]
pub(crate) use self::nix::home_dir;

// redox
#[cfg(target_os = "redox")]
pub(crate) use self::redox::home_dir;

// windows
#[cfg(windows)]
pub(crate) use self::windows::home_dir;

// all others
#[cfg(not(any(
    all(unix, not(target_os = "redox"), feature = "libc"),
    target_os = "redox",
    windows,
)))]
pub(crate) use self::other::home_dir;

/*
 * *******************
 * Common
 * *******************
*/

use std::error::Error;
use std::fmt;

/// Internal error type used for debugging. Not exposed publicly.
#[derive(Debug)]
pub(crate) struct HomeDirError(HomeDirErrorKind);

impl fmt::Display for HomeDirError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::HomeDirErrorKind::*;
        match &self.0 {
            NotFound(Some(user)) => write!(f, "Unable to find home directory for user {}", user),
            NotFound(None) => write!(f, "Unable to find home directory for current user"),
            OS(Some(msg)) => write!(f, "libc error while looking up home directory: {}", msg),
            OS(None) => write!(f, "libc error while looking up home directory"),
            Unimplemented => write!(f, "Identifying the home directory of a user other than the current user is not yet implemented for this platform"),
        }
    }
}

impl HomeDirError {
    fn not_found(user: Option<&str>) -> Self {
        let kind = HomeDirErrorKind::NotFound(user.map(|s| s.to_string()));
        Self(kind)
    }
}

impl Error for HomeDirError {}

#[derive(Debug)]
pub(crate) enum HomeDirErrorKind {
    #[allow(unused)]
    NotFound(Option<String>),
    #[allow(unused)]
    OS(Option<String>),
    #[allow(unused)]
    Unimplemented,
}
