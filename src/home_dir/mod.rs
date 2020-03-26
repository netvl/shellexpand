/*
 * *******************
 * OS-specific modules
 * *******************
*/

// unixes
#[cfg(all(unix, feature = "home-dir"))]
mod nix;

// others
#[cfg(not(all(unix, feature = "home-dir")))]
mod other;

/*
 * *******************
 * OS-specific exports
 * *******************
*/

// unixes
#[cfg(all(unix, feature = "home-dir"))]
pub(crate) use self::nix::home_dir;

// all others
#[cfg(not(all(unix, feature = "home-dir")))]
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

impl HomeDirError {
    #[allow(unused)]
    fn libc_error(msg: Option<&str>) -> Self {
        let kind = HomeDirErrorKind::Libc(msg.map(|s| s.to_string()));
        Self(kind)
    }

    #[allow(unused)]
    fn not_found(user: Option<&str>) -> Self {
        let kind = HomeDirErrorKind::NotFound(user.map(|s| s.to_string()));
        Self(kind)
    }

    #[allow(unused)]
    fn unimplemented() -> Self {
        let kind = HomeDirErrorKind::Unimplemented;
        Self(kind)
    }
}

impl fmt::Display for HomeDirError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::HomeDirErrorKind::*;
        match &self.0 {
            Libc(Some(msg)) => write!(f, "libc error while looking up home directory: {}", msg),
            Libc(None) => write!(f, "libc error while looking up home directory"),
            NotFound(Some(user)) => write!(f, "Unable to find home directory for user {}", user),
            NotFound(None) => write!(f, "Unable to find home directory for current user"),
            Unimplemented => write!(f, "Identifying the home directory of a user other than the current user is not yet implemented for this platform"),
        }
    }
}

impl Error for HomeDirError {}

#[derive(Debug)]
pub(crate) enum HomeDirErrorKind {
    Libc(Option<String>),
    NotFound(Option<String>),
    Unimplemented,
}
