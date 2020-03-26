use std::ffi::{CStr, CString, OsString};
use std::mem;
use std::os::unix::ffi::OsStringExt;
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
            return dirs::home_dir().ok_or_else(|| HomeDirError::not_found(None));
        }
        Some(user) => user,
    };

    // Turn user into a "c string," i.e. a null terminated vector of bytes,
    // whose lifetime will outlive the calls to the libc functions below.
    let user_c = CString::new(user).expect("User name contains an unexpected zero byte");

    // Ask libc for the user's home directory with `getpwnam_r`.
    const BUFLEN: usize = 1024;
    let mut buf: [u8; BUFLEN] = [0; BUFLEN];
    // Safety: All `getpwnam_r` requires is that `pwd` has enough space to hold a passwd struct.
    let mut pwd: libc::passwd = unsafe { mem::zeroed() };
    // Safety: All `getpwnam_r` requires is that `result` has enough space to hold a passwd struct.
    let mut result: libc::passwd = unsafe { mem::zeroed() };
    let mut result_ptr = &mut result as *mut libc::passwd;
    let result_ptr_ptr = &mut result_ptr as *mut *mut libc::passwd;
    let err = unsafe {
        libc::getpwnam_r(
            /* const char*     name   */ user_c.as_ptr() as *const libc::c_char,
            /* struct passwd*  pwd    */ &mut pwd as *mut libc::passwd,
            /* char*           buf    */ buf.as_mut_ptr() as *mut libc::c_char,
            /* size_t          buflen */ BUFLEN as libc::size_t,
            /* struct passwd** result */ result_ptr_ptr,
        )
    };

    // Check libc's error, if any.
    if err != 0 {
        // If the error is due to insufficient buffer space, that's our fault.
        // Panic with the following message.
        if err == libc::ERANGE {
            panic!("libc error while looking up home directory: Insufficient buffer space supplied. This is an implementation error and should be unreachable. If you see this message, please file an issue at https://github.com/netvl/shellexpand.")
        }

        // Otherwise, ask libc for the message associated with this error.
        #[rustfmt::skip]
        let err = unsafe {
            libc::strerror_r(
                /* int errnum    */ err,
                /* char* buf     */ buf.as_mut_ptr() as *mut libc::c_char,
                /* size_t buflen */ BUFLEN,
            )
        };

        // If the call to `strerror_r` itself fails, return an unknown libc error.
        if err != 0 {
            return Err(HomeDirError::libc_error(None));
        }

        // Otherwise, convert the error message into a rust &str and return it.
        let msg = CStr::from_bytes_with_nul(&buf[..])
            .map_err(|_| HomeDirError::libc_error(None))?
            .to_string_lossy();
        return Err(HomeDirError::libc_error(Some(&msg)));
    }

    // If `results_ptr_ptr` is null, it means libc was unable to locate the home directory.
    // Return a not found error.
    if result_ptr_ptr.is_null() {
        return Err(HomeDirError::not_found(Some(user)));
    }

    // Libc should ensure that the `pw.pwdir` pointer is always valid; if
    // for some reason it doesn't, return an unknown libc error.
    if pwd.pw_dir.is_null() {
        return Err(HomeDirError::libc_error(None));
    }

    // We have found the user's home directory. Convert the `pwd.pw_dir` pointer, which
    // we know is valid, into a rust `PathBuf` and return it.
    let home_dir = {
        // Safety: Safe because we check above that `pwd.pw_dir` is valid.
        let bytes = unsafe { CStr::from_ptr(pwd.pw_dir) }.to_bytes().to_vec();
        let os_string = OsString::from_vec(bytes);
        PathBuf::from(os_string)
    };
    Ok(home_dir)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_home_dir() {
        // Test for the current user.
        let _ = home_dir(None).unwrap();
        // Test for a different user. `root` is the only user account
        // I can think of that should be on all *nix systems.
        let path = home_dir(Some("root")).unwrap();
        let expected = if cfg!(target_os = "macos") {
            Path::new("/var/root")
        } else {
            Path::new("/root")
        };
        assert_eq!(path, expected);
    }
}
