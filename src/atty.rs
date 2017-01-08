/*!
This atty module contains functions for detecting whether ripgrep is being fed
from (or to) a terminal. Windows and Unix do this differently, so implement
both here.
*/

#[cfg(windows)]
use winapi::minwindef::DWORD;
#[cfg(windows)]
use winapi::winnt::HANDLE;

#[cfg(unix)]
pub fn stdin_is_readable() -> bool {
    use std::os::unix::fs::FileTypeExt;
    use same_file::Handle;

    let ft = match Handle::stdin().and_then(|h| h.as_file().metadata()) {
        Err(_) => return false,
        Ok(md) => md.file_type(),
    };
    ft.is_file() || ft.is_fifo()
}

#[cfg(windows)]
pub fn stdin_is_readable() -> bool {
    // ???
    true
}

/// Returns true if there is a tty on stdin.
#[cfg(unix)]
pub fn on_stdin() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDIN_FILENO) }
}

/// Returns true if there is a tty on stdout.
#[cfg(unix)]
pub fn on_stdout() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDOUT_FILENO) }
}

/// Returns true if there is a tty on stdin.
#[cfg(windows)]
pub fn on_stdin() -> bool {
    use kernel32::GetStdHandle;
    use winapi::winbase::{
        STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE,
    };

    unsafe {
        let stdin = GetStdHandle(STD_INPUT_HANDLE);
        if console_on_handle(stdin) {
            // False positives aren't possible. If we got a console then
            // we definitely have a tty on stdin.
            return true;
        }
        // Otherwise, it's possible to get a false negative. If we know that
        // there's a console on stdout or stderr however, then this is a true
        // negative.
        if console_on_fd(STD_OUTPUT_HANDLE)
            || console_on_fd(STD_ERROR_HANDLE) {
            return false;
        }
        // Otherwise, we can't really tell, so we do a weird hack.
        msys_tty_on_handle(stdin)
    }
}

/// Returns true if there is a tty on stdout.
#[cfg(windows)]
pub fn on_stdout() -> bool {
    use kernel32::GetStdHandle;
    use winapi::winbase::{
        STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE,
    };

    unsafe {
        let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
        if console_on_handle(stdout) {
            // False positives aren't possible. If we got a console then
            // we definitely have a tty on stdout.
            return true;
        }
        // Otherwise, it's possible to get a false negative. If we know that
        // there's a console on stdin or stderr however, then this is a true
        // negative.
        if console_on_fd(STD_INPUT_HANDLE) || console_on_fd(STD_ERROR_HANDLE) {
            return false;
        }
        // Otherwise, we can't really tell, so we do a weird hack.
        msys_tty_on_handle(stdout)
    }
}

/// Returns true if there is an MSYS tty on the given handle.
#[cfg(windows)]
unsafe fn msys_tty_on_handle(handle: HANDLE) -> bool {
    use std::ffi::OsString;
    use std::mem;
    use std::os::raw::c_void;
    use std::os::windows::ffi::OsStringExt;
    use std::slice;

    use kernel32::{GetFileInformationByHandleEx};
    use winapi::fileapi::FILE_NAME_INFO;
    use winapi::minwinbase::FileNameInfo;
    use winapi::minwindef::MAX_PATH;

    let size = mem::size_of::<FILE_NAME_INFO>();
    let mut name_info_bytes = vec![0u8; size + MAX_PATH];
    let res = GetFileInformationByHandleEx(
        handle,
        FileNameInfo,
        &mut *name_info_bytes as *mut _ as *mut c_void,
        name_info_bytes.len() as u32);
    if res == 0 {
        return true;
    }
    let name_info: FILE_NAME_INFO =
        *(name_info_bytes[0..size].as_ptr() as *const FILE_NAME_INFO);
    let name_bytes =
        &name_info_bytes[size..size + name_info.FileNameLength as usize];
    let name_u16 = slice::from_raw_parts(
        name_bytes.as_ptr() as *const u16, name_bytes.len() / 2);
    let name = OsString::from_wide(name_u16)
        .as_os_str().to_string_lossy().into_owned();
    name.contains("msys-") || name.contains("-pty")
}

/// Returns true if there is a console on the given file descriptor.
#[cfg(windows)]
unsafe fn console_on_fd(fd: DWORD) -> bool {
    use kernel32::GetStdHandle;
    console_on_handle(GetStdHandle(fd))
}

/// Returns true if there is a console on the given handle.
#[cfg(windows)]
unsafe fn console_on_handle(handle: HANDLE) -> bool {
    use kernel32::GetConsoleMode;
    let mut out = 0;
    GetConsoleMode(handle, &mut out) != 0
}
