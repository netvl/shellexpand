use std::borrow::BorrowMut;
use std::cell::{RefCell, RefMut};
use std::ffi::OsString;
use std::path::PathBuf;

use winapi::shared::minwindef::DWORD;
use winapi::um::winnt::WCHAR;

use super::{HomeDirError, HomeDirErrorKind};

thread_local! {
    // Allocate required buffers once per thread instead of each time 'home_dir'
    // is called so that users who call 'home_dir' multiple times on the same
    // thread are spared uncessary allocations
    static BUF_DWORD: RefCell<Vec<DWORD>> = RefCell::new(vec![0; 1024]);
    static BUF_U8:    RefCell<Vec<u8>>    = RefCell::new(vec![0; 1024]);
    static BUF_WCHAR: RefCell<Vec<WCHAR>> = RefCell::new(vec![0; 1024]);
}

/// Returns the home directory of:
/// * the current user if `user` is `None` or an empty string,
/// * the Default user is `user` is `Some("Default")`, or
/// * the provided user if `user` is anything else.
///
/// On Windows, querying the home directory of any user other than the
/// current user or the Default user requires:
/// * Elevated priviliges (i.e. `Run as Administrator`),
/// * That the other user is logged in
#[rustfmt::skip]
pub(crate) fn home_dir(user: Option<&str>) -> Result<PathBuf, HomeDirError> {
    BUF_DWORD.with(move |buf_dword| {
        BUF_U8.with(move |buf_u8| {
            BUF_WCHAR.with(move |buf_wchar| {
                let mut buf_dword: RefMut<Vec<DWORD>> = buf_dword.borrow_mut();
                let mut buf_u8:    RefMut<Vec<u8>>    = buf_u8.borrow_mut();
                let mut buf_wchar: RefMut<Vec<WCHAR>> = buf_wchar.borrow_mut();

                let buf_dword:     &mut Vec<DWORD>    = buf_dword.borrow_mut();
                let buf_u8:        &mut Vec<u8>       = buf_u8.borrow_mut();
                let buf_wchar:     &mut Vec<WCHAR>    = buf_wchar.borrow_mut();

                match user {
                    Some("") | None => get_profile_directory(None, buf_dword, buf_u8, buf_wchar),
                    Some("Default") => sys::get_default_user_profile_directory(buf_wchar),
                    Some(user) => get_profile_directory(Some(user), buf_dword, buf_u8, buf_wchar),
                }
            })
        })
    })
}

/// Returns the profile directory of the provided user.
fn get_profile_directory(
    user: Option<&str>,
    buf_dword: &mut Vec<DWORD>,
    buf_u8: &mut Vec<u8>,
    buf_wchar: &mut Vec<WCHAR>,
) -> Result<PathBuf, HomeDirError> {
    let mut current_process = sys::get_current_process()?;
    let current_user = get_user(&mut current_process, buf_u8, buf_wchar)?;
    let mut current_token = sys::open_process_token(&mut current_process, buf_wchar)?;

    let user = match user {
        None => {
            let path = sys::get_user_profile_directory(&mut current_token, buf_wchar)?;
            return Ok(path);
        }
        Some(user) if user == current_user => {
            let path = sys::get_user_profile_directory(&mut current_token, buf_wchar)?;
            return Ok(path);
        }
        Some(user) => user,
    };

    // If we reach here, we're looking for the home directory of another user.
    // On Windows unfortunatley this requires:
    //
    // 1) That we have elevated priviliges, and
    // 2) The other user is logged in
    //
    // because we need one of their token handles and the only way to get one
    // of those is through a handle to a running process that they are the user
    // of, which we cannot read unless we have elevated priviliges.

    // If the user doesn't have elevated priviliges, return a PermissionDenied error.
    let has_elevated_priviliges =
        sys::get_token_information_token_elevation(&mut current_token, buf_wchar)?;
    if !has_elevated_priviliges {
        return Err(HomeDirError::permission_denied(user));
    }

    // Now we fill `buf_dword` with a list of the pids of all running processes.
    sys::enum_processes(buf_dword, buf_wchar)?;

    // For each pid, we first try to get the username of the process' user.
    // If that username matches the username we're looking for, we then try to
    // get that user's home directory. If this doesn't work for any pid, we
    // return a not found error.

    fn for_each_pid(
        pid: DWORD,
        user: &str,
        buf_u8: &mut Vec<u8>,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Option<PathBuf> {
        if pid == 0 {
            return None;
        }
        let mut process = sys::open_process(pid, buf_wchar).ok()?;
        let mut token = sys::open_process_token(&mut process, buf_wchar).ok()?;
        let sid = sys::get_token_information_token_user(&mut token, buf_u8, buf_wchar).ok()?;
        let s = sys::lookup_account_sid(sid, buf_wchar).ok()?;
        if &s == user {
            let path = sys::get_user_profile_directory(&mut token, buf_wchar).ok()?;
            return Some(path);
        }
        None
    }

    for &pid in buf_dword.iter() {
        match for_each_pid(pid, user, buf_u8, buf_wchar) {
            Some(path) => return Ok(path),
            None => continue,
        }
    }
    Err(HomeDirError::not_found(Some(user)))
}

/// Returns the username of the user associated with the provided process.
fn get_user(
    process: &mut dyn handles::Process,
    buf_u8: &mut Vec<u8>,
    buf_wchar: &mut Vec<WCHAR>,
) -> Result<OsString, HomeDirError> {
    let mut token = sys::open_process_token(process, buf_wchar)?;
    let sid = sys::get_token_information_token_user(&mut token, buf_u8, buf_wchar)?;
    let user = sys::lookup_account_sid(sid, buf_wchar)?;
    Ok(user)
}

impl HomeDirError {
    fn os(buf_wchar: &mut Vec<WCHAR>) -> Self {
        let errnum = sys::get_last_error();
        let msg = sys::format_message(errnum, buf_wchar);
        Self(HomeDirErrorKind::OS(Some(msg)))
    }

    fn os_from_errnum(errnum: DWORD, buf_wchar: &mut Vec<WCHAR>) -> Self {
        let msg = sys::format_message(errnum, buf_wchar);
        Self(HomeDirErrorKind::OS(Some(msg)))
    }

    fn os_from_str<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self(HomeDirErrorKind::OS(Some(msg.into())))
    }

    fn permission_denied<S>(user: S) -> Self
    where
        S: Into<String>,
    {
        Self(HomeDirErrorKind::PermissionDenied(user.into()))
    }
}

/// Safe wrappers around raw winapi C functions
mod sys {
    use std::ffi::OsString;
    use std::mem;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::ptr::NonNull;

    use winapi::ctypes::c_void;
    use winapi::shared::minwindef::DWORD;
    use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcess, OpenProcessToken};
    use winapi::um::psapi::EnumProcesses;
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::userenv::{GetDefaultUserProfileDirectoryW, GetUserProfileDirectoryW};
    use winapi::um::winbase::{FormatMessageW, LookupAccountSidW, FORMAT_MESSAGE_FROM_SYSTEM};
    use winapi::um::winnt::{
        TokenElevation, TokenUser, PROCESS_QUERY_INFORMATION, SID_NAME_USE, TOKEN_ELEVATION,
        TOKEN_QUERY, TOKEN_USER,
    };
    use winapi::um::winnt::{HANDLE, WCHAR};

    use super::handles::{NonNullDrop, Process, ProcessCurrent, ProcessOther, Sid, Token};
    use super::HomeDirError;

    /// Fills `buf_dword` with the process identifier for each process object in the system.
    ///
    /// https://docs.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-enumprocesses
    pub(crate) fn enum_processes(
        buf_dword: &mut Vec<DWORD>,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<(), HomeDirError> {
        loop {
            let nbytes = (buf_dword.len() * mem::size_of::<DWORD>()) as DWORD;
            let mut nbytes_filled: DWORD = 0;
            let ret = unsafe {
                EnumProcesses(
                    /* DWORD*   lpidProcess */ buf_dword.as_mut_ptr(),
                    /* DWORD    cb          */ nbytes,
                    /* LPDWORD  lpcbNeeded  */ &mut nbytes_filled as *mut DWORD,
                )
            };
            if ret == 0 {
                return Err(HomeDirError::os(buf_wchar));
            }
            if nbytes == nbytes_filled {
                buf_dword.resize(buf_dword.len() * 2, 0);
                continue;
            }
            let len = nbytes_filled as usize / mem::size_of::<DWORD>();
            buf_dword.resize(len, 0);
            break;
        }
        Ok(())
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-formatmessagew
    pub(crate) fn format_message(errnum: DWORD, buf_wchar: &mut Vec<WCHAR>) -> String {
        let mut len;
        loop {
            len = unsafe {
                FormatMessageW(
                    /* DWORD    dwFlags      */ FORMAT_MESSAGE_FROM_SYSTEM,
                    /* LPCVOID  lpSource     */ std::ptr::null(),
                    /* DWORD    dwMessageId  */ errnum,
                    /* DWORD    dwLanguageId */ 0,
                    /* LPWSTR   lpBuffer     */ buf_wchar.as_mut_ptr(),
                    /* DWORD    nSize        */ buf_wchar.len() as DWORD,
                    /* va_list* Arguments    */ std::ptr::null_mut(),
                )
            };
            if len != 0 {
                break;
            }
            buf_wchar.resize(buf_wchar.len() * 2, 0);
        }
        OsString::from_wide(&buf_wchar[..len as usize])
            .to_string_lossy()
            .trim()
            .to_string()
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess
    pub(crate) fn get_current_process() -> Result<ProcessCurrent, HomeDirError> {
        let handle = unsafe { GetCurrentProcess() };
        Ok(NonNull::new(handle).ok_or_else(|| {
            HomeDirError::os_from_str("GetCurrentProcess unexpectedly returned a null pointer.")
        })?)
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getdefaultuserprofiledirectoryw
    pub(crate) fn get_default_user_profile_directory(
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<PathBuf, HomeDirError> {
        let mut len: DWORD = buf_wchar.len() as DWORD;
        loop {
            let ret = unsafe {
                GetDefaultUserProfileDirectoryW(
                    /* LPWSTR  lpProfileDir */ buf_wchar.as_mut_ptr() as *mut WCHAR,
                    /* LPDWORD lpcchSize    */ &mut len as *mut DWORD,
                )
            };
            if ret == 0 {
                match unsafe { GetLastError() } {
                    ERROR_INSUFFICIENT_BUFFER => {
                        buf_wchar.resize(len as usize, 0);
                        continue;
                    }
                    errnum => return Err(HomeDirError::os_from_errnum(errnum, buf_wchar)),
                }
            }
            break;
        }
        let slice = &buf_wchar[..len as usize - 1];
        let path = PathBuf::from(OsString::from_wide(slice));
        Ok(path)
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-getlasterror
    pub(crate) fn get_last_error() -> DWORD {
        unsafe { GetLastError() }
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation
    pub(crate) fn get_token_information_token_elevation(
        token: &mut Token,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<bool, HomeDirError> {
        let mut elevation = unsafe { mem::zeroed::<TOKEN_ELEVATION>() };
        let mut nbytes = mem::size_of::<TOKEN_ELEVATION>() as DWORD;
        let ret = unsafe {
            GetTokenInformation(
                token.as_ptr(),
                TokenElevation,
                &mut elevation as *mut TOKEN_ELEVATION as *mut c_void,
                mem::size_of::<TOKEN_ELEVATION>() as DWORD,
                &mut nbytes as *mut DWORD,
            )
        };
        if ret == 0 {
            return Err(HomeDirError::os(buf_wchar));
        }
        let is_elevated = if elevation.TokenIsElevated == 0 {
            false
        } else {
            true
        };
        Ok(is_elevated)
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation
    pub(crate) fn get_token_information_token_user(
        token: &mut Token,
        buf_u8: &mut Vec<u8>,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<Sid, HomeDirError> {
        let mut nbytes: DWORD = 0;
        loop {
            #[rustfmt::skip]
            let ret = unsafe {
                GetTokenInformation(
                    /* HANDLE                  TokenHandle            */ token.as_ptr(),
                    /* TOKEN_INFORMATION_CLASS TokenInformationClass  */ TokenUser,
                    /* LPVOID                  TokenInformation       */ buf_u8.as_mut_ptr() as *mut c_void,
                    /* DWORD                   TokenInformationLength */ buf_u8.len() as DWORD,
                    /* PDWORD                  ReturnLength           */ &mut nbytes as *mut DWORD,
                )
            };
            if ret == 0 {
                match get_last_error() {
                    ERROR_INSUFFICIENT_BUFFER => {
                        buf_u8.resize(buf_u8.len() * 2, 0);
                        continue;
                    }
                    errnum => return Err(HomeDirError::os_from_errnum(errnum, buf_wchar)),
                }
            }
            break;
        }
        let token_user_ptr: *const TOKEN_USER = unsafe { mem::transmute(buf_u8.as_ptr()) };
        if token_user_ptr.is_null() {
            return Err(HomeDirError::os_from_str(
                "GetTokenInformation returned an invalid pointer.",
            ));
        }
        let sid_ptr = unsafe { *token_user_ptr }.User.Sid;
        let sid = NonNull::new(sid_ptr).ok_or_else(|| {
            HomeDirError::os_from_str("GetTokenInformation return an invalid pointer.")
        })?;
        Ok(sid)
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getuserprofiledirectoryw
    pub(crate) fn get_user_profile_directory(
        token: &mut Token,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<PathBuf, HomeDirError> {
        let mut len = buf_wchar.len() as DWORD;
        loop {
            let ret = unsafe {
                GetUserProfileDirectoryW(
                    /* HANDLE  hToken       */ token.as_ptr(),
                    /* LPWSTR  lpProfileDir */ buf_wchar.as_mut_ptr() as *mut WCHAR,
                    /* LPDWORD lpcchSize    */ &mut len as *mut DWORD,
                )
            };
            if ret == 0 {
                match unsafe { GetLastError() } {
                    ERROR_INSUFFICIENT_BUFFER => {
                        buf_wchar.resize(len as usize, 0);
                        continue;
                    }
                    errnum => {
                        return Err(HomeDirError::os_from_errnum(errnum, buf_wchar));
                    }
                }
            }
            break;
        }

        let slice = &buf_wchar[..len as usize - 1];
        let path = PathBuf::from(OsString::from_wide(slice));
        Ok(path)
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-lookupaccountsidw
    pub(crate) fn lookup_account_sid(
        mut sid: Sid,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<OsString, HomeDirError> {
        let mut buf_wchar_len: DWORD = buf_wchar.len() as DWORD;

        let mut buf_other: [WCHAR; 1024] = [0; 1024];
        let mut buf_other_len: DWORD = 1024;

        let mut sid_name_use: SID_NAME_USE = unsafe { mem::zeroed() };
        loop {
            #[rustfmt::skip]
            let ret = unsafe {
                LookupAccountSidW(
                    /*  LPCWSTR       lpSystemName            */ std::ptr::null_mut(),
                    /*  PSID          Sid                     */ sid.as_mut(),
                    /*  LPWSTR        Name                    */ buf_wchar.as_mut_ptr(),
                    /*  LPDWORD       cchName                 */ &mut buf_wchar_len as *mut DWORD,
                    /*  LPWSTR        ReferencedDomainName    */ buf_other.as_mut_ptr(),
                    /*  LPDWORD       cchReferencedDomainName */ &mut buf_other_len as *mut DWORD,
                    /*  PSID_NAME_USE peUse                   */ &mut sid_name_use as *mut SID_NAME_USE,
                )
            };
            if ret == 0 {
                match unsafe { GetLastError() } {
                    ERROR_INSUFFICIENT_BUFFER => {
                        buf_wchar.resize(buf_wchar_len as usize, 0);
                        continue;
                    }
                    errnum => {
                        return Err(HomeDirError::os_from_errnum(errnum, buf_wchar));
                    }
                }
            }
            break;
        }
        let len = match buf_wchar.iter().position(|&w| w == 0) {
            Some(len) => len,
            None => {
                return Err(HomeDirError::os_from_str(
                    "LookupAccountSid unexpectedly return c-string without a nul terminator.",
                ))
            }
        };
        let s = OsString::from_wide(&buf_wchar[..len]);
        Ok(s)
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess
    pub(crate) fn open_process(
        pid: DWORD,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<ProcessOther, HomeDirError> {
        let process_handle = unsafe {
            OpenProcess(
                /* DWORD dwDesiredAccess */ PROCESS_QUERY_INFORMATION,
                /* BOOL  bInheritHandle  */ 0,
                /* DWORD dwProcessId     */ pid,
            )
        };
        Ok(NonNullDrop::from(
            NonNull::new(process_handle).ok_or_else(|| HomeDirError::os(buf_wchar))?,
        ))
    }

    /// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocesstoken
    pub(crate) fn open_process_token<'a>(
        process: &'a mut dyn Process,
        buf_wchar: &mut Vec<WCHAR>,
    ) -> Result<Token<'a>, HomeDirError> {
        let mut token_handle = unsafe { mem::zeroed::<HANDLE>() };
        let ret = unsafe {
            OpenProcessToken(
                /* HANDLE  ProcessHandle */ process.as_ptr(),
                /* DWORD   DesiredAccess */ TOKEN_QUERY,
                /* PHANDLE TokenHandle   */ &mut token_handle as *mut HANDLE,
            )
        };
        if ret == 0 {
            return Err(HomeDirError::os(buf_wchar));
        }
        let ptr = NonNullDrop::from(NonNull::new(token_handle).ok_or_else(|| {
            HomeDirError::os_from_str("OpenProcessHandle unexpectedly returned a null pointer.")
        })?);
        Ok(Token { ptr, process })
    }
}

/// Safe wrappers for various winapi "HANDLE"s (void pointers)
pub(crate) mod handles {
    use std::ops::{Deref, DerefMut};
    use std::ptr::NonNull;

    use winapi::ctypes::c_void;
    use winapi::um::handleapi::CloseHandle;

    // Handles to either the current process or a SID do not need to be closed;
    // so we can simply represent them with std::ptr::NonNull<c_void>
    pub(crate) type ProcessCurrent = NonNull<c_void>;
    pub(crate) type Sid = NonNull<c_void>;

    // Handles to other process needs to be closed; so we represent them with
    // a custom type (`NonNullDrop`, see below) that closes the handle on drop.
    pub(crate) type ProcessOther = NonNullDrop<c_void>;

    #[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
    pub(crate) struct NonNullDrop<T>(NonNull<T>);

    impl<T> From<NonNull<T>> for NonNullDrop<T> {
        fn from(ptr: NonNull<T>) -> Self {
            Self(ptr)
        }
    }

    impl<T> Drop for NonNullDrop<T> {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0.as_ptr() as *mut c_void) };
        }
    }

    // Trait that enables functions to accept either a ProcessCurrent or a ProcessOther
    pub(crate) trait Process {
        fn as_ptr(&mut self) -> *mut c_void;
    }

    impl Process for NonNull<c_void> {
        fn as_ptr(&mut self) -> *mut c_void {
            Self::as_ptr(*self)
        }
    }

    impl Process for NonNullDrop<c_void> {
        fn as_ptr(&mut self) -> *mut c_void {
            self.0.as_ptr()
        }
    }

    // Wrapper for a token handle, which includes a reference to the process
    // it came from, as the process needs to outlive it (i.e. not be closed
    // before the token is closed)
    pub(crate) struct Token<'a> {
        pub(crate) ptr: NonNullDrop<c_void>,
        #[allow(unused)]
        pub(crate) process: &'a dyn Process,
    }

    impl<'a> Deref for Token<'a> {
        type Target = NonNullDrop<c_void>;
        fn deref(&self) -> &Self::Target {
            &self.ptr
        }
    }

    impl<'a> DerefMut for Token<'a> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.ptr
        }
    }
}
