use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::ffi::{CStr, OsString};
use std::mem;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use super::{HomeDirError, HomeDirErrorKind};

thread_local! {
    // Allocate required buffers once per thread instead of each time 'home_dir'
    // is called so that users who call 'home_dir' multiple times on the same
    // thread are spared uncessary allocations
    //
    // Note: It's prudent here to use heap-allocated buffers instead of stack-
    // allocated arrays because it's quite difficult (in a cross-platform, 100%
    // portable way) to ask libc for the maximum possible lengths of usernames
    // and home directories. There doesn't appear to be a good POSIX standard
    // for this that all *nix systems adhere to.
    static BUF0: RefCell<Vec<u8>> = RefCell::new(vec![0; 1024]);
    static BUF1: RefCell<Vec<u8>> = RefCell::new(vec![0; 1]);
}

/// Returns the home directory of:
/// * the current user if `user` is `None` or an empty string, or
/// * the provided user if `user` is anything else.
pub(crate) fn home_dir(user: Option<&str>) -> Result<PathBuf, HomeDirError> {
    BUF0.with(move |buf0| {
        BUF1.with(move |buf1| {
            let mut buf0 = buf0.borrow_mut();
            let mut buf1 = buf1.borrow_mut();
            _home_dir(user, buf0.borrow_mut(), buf1.borrow_mut())
        })
    })
}

fn _home_dir(
    user: Option<&str>,
    buf0: &mut Vec<u8>,
    buf1: &mut Vec<u8>,
) -> Result<PathBuf, HomeDirError> {
    let user = match user {
        None | Some("") => {
            // When user is `None` or an empty string, let the `dirs` crate do the work.
            return dirs::home_dir().ok_or_else(|| HomeDirError::not_found(None));
        }
        Some(user) => user,
    };

    // Copy c-string version of user into buf0
    copy_into_buffer(user, buf0.borrow_mut());

    // Initialze out parameters of 'libc::getpwnam_r'
    let mut pwd: libc::passwd = unsafe { mem::zeroed() };
    let mut result: libc::passwd = unsafe { mem::zeroed() };
    let mut result_ptr = &mut result as *mut libc::passwd;
    let result_ptr_ptr = &mut result_ptr as *mut *mut libc::passwd;

    loop {
        // Call 'libc::getpwnam_r' to write the user's home directory into buf1
        let ret = unsafe {
            libc::getpwnam_r(
                /* const char*     name   */ buf0.as_ptr() as *const libc::c_char,
                /* struct passwd*  pwd    */ &mut pwd as *mut libc::passwd,
                /* char*           buf    */ buf1.as_mut_ptr() as *mut libc::c_char,
                /* size_t          buflen */ buf1.len() as libc::size_t,
                /* struct passwd** result */ result_ptr_ptr,
            )
        };
        match ret {
            // If successful, break
            0 => break,
            // If buf1 was too small to hold the user's home directory,
            // double the size of buf1 and try again
            libc::ERANGE => {
                buf1.resize(buf1.len() * 2, 0);
                continue;
            }
            // If unsuccessful due to any other error, return a libc error
            errnum => return Err(HomeDirError::os(Some(errnum))),
        }
    }

    // If `results_ptr_ptr` is null, it means libc was unable to locate the home
    // directory.  Return a not found error.
    if result_ptr_ptr.is_null() {
        return Err(HomeDirError::not_found(Some(user)));
    }

    // Libc should ensure that the `pw.pwdir` pointer is always valid; if
    // for some reason it doesn't, return an unknown libc error.
    if pwd.pw_dir.is_null() {
        return Err(HomeDirError::os(None));
    }

    // We have found the user's home directory. Convert the `pwd.pw_dir` pointer,
    // which we know is valid, into a rust `PathBuf` and return it.
    let home_dir = {
        // Safety: Safe because we check above that `pwd.pw_dir` is valid.
        let bytes = unsafe { CStr::from_ptr(pwd.pw_dir) }.to_bytes().to_vec();
        let os_string = OsString::from_vec(bytes);
        PathBuf::from(os_string)
    };

    Ok(home_dir)
}

fn copy_into_buffer(src: &str, dst: &mut Vec<u8>) {
    let len = src.len();
    // Ensure dst is large enough to hold src bytes plus a NULL byte
    dst.resize(len + 1, 0);
    // Copy src bytes into dst
    (&mut dst[..len]).copy_from_slice(src.as_bytes());
    // Add NULL byte at the end
    dst[len] = b'\0';
}

impl HomeDirError {
    /// Converts an optional errnum from C into a HomeDirError
    fn os(errnum: Option<libc::c_int>) -> Self {
        if errnum.is_none() {
            let kind = HomeDirErrorKind::OS(None);
            return Self(kind);
        }

        let errnum = errnum.unwrap();

        // Initialize a c-string buffer on the stack large enough to hold
        // the error message
        let mut buf = [0u8; 1024];

        // Use `libc::strerror_r` to get the error message
        #[rustfmt::skip]
        let ret = unsafe {
            libc::strerror_r(
                /* int errnum    */ errnum,
                /* char* buf     */ buf.as_mut_ptr() as *mut libc::c_char,
                /* size_t buflen */ buf.len(),
            )
        };

        // If `libc::strerror_r` fails, return an unknown libc error.
        if ret != 0 {
            let kind = HomeDirErrorKind::OS(None);
            return Self(kind);
        }

        // Otherwise, convert the message and return it.
        let kind = match CStr::from_bytes_with_nul(&buf[..]) {
            Ok(msg) => HomeDirErrorKind::OS(Some(msg.to_string_lossy().into())),
            Err(_) => HomeDirErrorKind::OS(None),
        };

        Self(kind)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_home_dir() {
        // Spawn many threads to ensure thread-safety
        const NTHREADS: usize = 100;
        let mut handles = Vec::with_capacity(NTHREADS);
        for _ in 0..NTHREADS {
            let handle = std::thread::spawn(|| {
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
            });
            handles.push(handle);
        }
        for handle in handles {
            handle.join().unwrap();
        }
    }
}
