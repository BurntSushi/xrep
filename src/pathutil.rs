/*!
The pathutil module provides platform specific operations on paths that are
typically faster than the same operations as provided in `std::path`. In
particular, we really want to avoid the costly operation of parsing the path
into its constituent components. We give up on Windows, but on Unix, we deal
with the raw bytes directly.

On large repositories (like chromium), this can have a ~25% performance
improvement on just listing the files to search (!).
*/
use std::path::Path;

/// Strip `prefix` from the `path` and return the remainder.
///
/// If `path` doesn't have a prefix `prefix`, then return `None`.
#[cfg(unix)]
pub fn strip_prefix<'a, P: AsRef<Path> + ?Sized>(
    prefix: &'a P,
    path: &'a Path,
) -> Option<&'a Path> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let prefix = prefix.as_ref().as_os_str().as_bytes();
    let path = path.as_os_str().as_bytes();
    if prefix.len() > path.len() || prefix != &path[0..prefix.len()] {
        None
    } else {
        Some(Path::new(OsStr::from_bytes(&path[prefix.len()..])))
    }
}

/// Strip `prefix` from the `path` and return the remainder.
///
/// If `path` doesn't have a prefix `prefix`, then return `None`.
#[cfg(not(unix))]
pub fn strip_prefix<'a, P: AsRef<Path> + ?Sized>(
    prefix: &'a P,
    path: &'a Path,
) -> Option<&'a Path> {
    path.strip_prefix(prefix).ok()
}
