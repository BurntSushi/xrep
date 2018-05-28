use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;

/// The final component of the path, if it is a normal file.
///
/// If the path terminates in ., .., or consists solely of a root of prefix,
/// file_name will return None.
#[cfg(unix)]
pub fn file_name<'a, P: AsRef<Path> + ?Sized>(
    path: &'a P,
) -> Option<&'a OsStr> {
    use std::os::unix::ffi::OsStrExt;
    use memchr::memrchr;

    let path = path.as_ref().as_os_str().as_bytes();
    if path.is_empty() {
        return None;
    } else if path.len() == 1 && path[0] == b'.' {
        return None;
    } else if path.last() == Some(&b'.') {
        return None;
    } else if path.len() >= 2 && &path[path.len() - 2..] == &b".."[..] {
        return None;
    }
    let last_slash = memrchr(b'/', path).map(|i| i + 1).unwrap_or(0);
    Some(OsStr::from_bytes(&path[last_slash..]))
}

/// The final component of the path, if it is a normal file.
///
/// If the path terminates in ., .., or consists solely of a root of prefix,
/// file_name will return None.
#[cfg(not(unix))]
pub fn file_name<'a, P: AsRef<Path> + ?Sized>(
    path: &'a P,
) -> Option<&'a OsStr> {
    path.as_ref().file_name()
}

/// Return a file extension given a path's file name.
///
/// Note that this does NOT match the semantics of std::path::Path::extension.
/// Namely, the extension includes the `.` and matching is otherwise more
/// liberal. Specifically, the extenion is:
///
/// * None, if the file name given is empty;
/// * None, if there is no embedded `.`;
/// * Otherwise, the portion of the file name starting with the final `.`.
///
/// e.g., A file name of `.rs` has an extension `.rs`.
///
/// N.B. This is done to make certain glob match optimizations easier. Namely,
/// a pattern like `*.rs` is obviously trying to match files with a `rs`
/// extension, but it also matches files like `.rs`, which doesn't have an
/// extension according to std::path::Path::extension.
pub fn file_name_ext(name: &OsStr) -> Option<Cow<[u8]>> {
    if name.is_empty() {
        return None;
    }
    let name = os_str_bytes(name);
    let last_dot_at = {
        let result = name
            .iter().enumerate().rev()
            .find(|&(_, &b)| b == b'.')
            .map(|(i, _)| i);
        match result {
            None => return None,
            Some(i) => i,
        }
    };
    Some(match name {
        Cow::Borrowed(name) => Cow::Borrowed(&name[last_dot_at..]),
        Cow::Owned(mut name) => {
            name.drain(..last_dot_at);
            Cow::Owned(name)
        }
    })
}

/// Return raw bytes of a path, transcoded to UTF-8 if necessary.
pub fn path_bytes(path: &Path) -> Cow<[u8]> {
    os_str_bytes(path.as_os_str())
}

/// Return the raw bytes of the given OS string, possibly transcoded to UTF-8.
#[cfg(unix)]
pub fn os_str_bytes(s: &OsStr) -> Cow<[u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(s.as_bytes())
}

/// Return the raw bytes of the given OS string, possibly transcoded to UTF-8.
#[cfg(not(unix))]
pub fn os_str_bytes(s: &OsStr) -> Cow<[u8]> {
    // TODO(burntsushi): On Windows, OS strings are WTF-8, which is a superset
    // of UTF-8, so even if we could get at the raw bytes, they wouldn't
    // be useful. We *must* convert to UTF-8 before doing path matching.
    // Unfortunate, but necessary.
    match s.to_string_lossy() {
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
    }
}

/// Normalizes a path to use `/` as a separator everywhere, even on platforms
/// that recognize other characters as separators.
#[cfg(unix)]
pub fn normalize_path(path: Cow<[u8]>) -> Cow<[u8]> {
    // UNIX only uses /, so we're good.
    path
}

/// Normalizes a path to use `/` as a separator everywhere, even on platforms
/// that recognize other characters as separators.
#[cfg(not(unix))]
pub fn normalize_path(mut path: Cow<[u8]>) -> Cow<[u8]> {
    use std::path::is_separator;

    for i in 0..path.len() {
        if path[i] == b'/' || !is_separator(path[i] as char) {
            continue;
        }
        path.to_mut()[i] = b'/';
    }
    path
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::ffi::OsStr;

    use super::{file_name_ext, normalize_path};

    macro_rules! ext {
        ($name:ident, $file_name:expr, $ext:expr) => {
            #[test]
            fn $name() {
                let got = file_name_ext(OsStr::new($file_name));
                assert_eq!($ext.map(|s| Cow::Borrowed(s.as_bytes())), got);
            }
        };
    }

    ext!(ext1, "foo.rs", Some(".rs"));
    ext!(ext2, ".rs", Some(".rs"));
    ext!(ext3, "..rs", Some(".rs"));
    ext!(ext4, "", None::<&str>);
    ext!(ext5, "foo", None::<&str>);

    macro_rules! normalize {
        ($name:ident, $path:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let got = normalize_path(Cow::Owned($path.to_vec()));
                assert_eq!($expected.to_vec(), got.into_owned());
            }
        };
    }

    normalize!(normal1, b"foo", b"foo");
    normalize!(normal2, b"foo/bar", b"foo/bar");
    #[cfg(unix)]
    normalize!(normal3, b"foo\\bar", b"foo\\bar");
    #[cfg(not(unix))]
    normalize!(normal3, b"foo\\bar", b"foo/bar");
    #[cfg(unix)]
    normalize!(normal4, b"foo\\bar/baz", b"foo\\bar/baz");
    #[cfg(not(unix))]
    normalize!(normal4, b"foo\\bar/baz", b"foo/bar/baz");
}
