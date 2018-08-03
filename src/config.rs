// This module provides routines for reading ripgrep config "rc" files. The
// primary output of these routines is a sequence of arguments, where each
// argument corresponds precisely to one shell argument.

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use Result;

/// Return a sequence of arguments derived from ripgrep rc configuration files.
pub fn args() -> Vec<OsString> {
    let config_path = match env::var_os("RIPGREP_CONFIG_PATH") {
        None => return vec![],
        Some(config_path) => {
            if config_path.is_empty() {
                return vec![];
            }
            PathBuf::from(config_path)
        }
    };
    let (args, errs) = match parse(&config_path) {
        Ok((args, errs)) => (args, errs),
        Err(err) => {
            message!("{}", err);
            return vec![];
        }
    };
    if !errs.is_empty() {
        for err in errs {
            message!("{}:{}", config_path.display(), err);
        }
    }
    debug!(
        "{}: arguments loaded from config file: {:?}",
        config_path.display(),
        args
    );
    args
}

/// Parse a single ripgrep rc file from the given path.
///
/// On success, this returns a set of shell arguments, in order, that should
/// be pre-pended to the arguments given to ripgrep at the command line.
///
/// If the file could not be read, then an error is returned. If there was
/// a problem parsing one or more lines in the file, then errors are returned
/// for each line in addition to successfully parsed arguments.
fn parse<P: AsRef<Path>>(
    path: P,
) -> Result<(Vec<OsString>, Vec<Box<Error>>)> {
    let path = path.as_ref();
    match File::open(&path) {
        Ok(file) => parse_reader(file),
        Err(err) => Err(From::from(format!("{}: {}", path.display(), err))),
    }
}

/// Parse a single ripgrep rc file from the given reader.
///
/// Callers should not provided a buffered reader, as this routine will use its
/// own buffer internally.
///
/// On success, this returns a set of shell arguments, in order, that should
/// be pre-pended to the arguments given to ripgrep at the command line.
///
/// If the reader could not be read, then an error is returned. If there was a
/// problem parsing one or more lines, then errors are returned for each line
/// in addition to successfully parsed arguments.
fn parse_reader<R: io::Read>(
    rdr: R,
) -> Result<(Vec<OsString>, Vec<Box<Error>>)> {
    let mut bufrdr = io::BufReader::new(rdr);
    let (mut args, mut errs) = (vec![], vec![]);
    let mut line = vec![];
    let mut line_number = 0;
    while {
        line.clear();
        line_number += 1;
        bufrdr.read_until(b'\n', &mut line)? > 0
    } {
        trim(&mut line);
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        match bytes_to_os_string(&line) {
            Ok(osstr) => {
                args.push(osstr);
            }
            Err(err) => {
                errs.push(format!("{}: {}", line_number, err).into());
            }
        }
    }
    Ok((args, errs))
}

/// Trim the given bytes of whitespace according to the ASCII definition.
fn trim(x: &mut Vec<u8>) {
    let upto = x.iter().take_while(|b| is_space(**b)).count();
    x.drain(..upto);
    let revto = x.len() - x.iter().rev().take_while(|b| is_space(**b)).count();
    x.drain(revto..);
}

/// Returns true if and only if the given byte is an ASCII space character.
fn is_space(b: u8) -> bool {
    b == b'\t'
    || b == b'\n'
    || b == b'\x0B'
    || b == b'\x0C'
    || b == b'\r'
    || b == b' '
}

/// On Unix, get an OsString from raw bytes.
#[cfg(unix)]
fn bytes_to_os_string(bytes: &[u8]) -> Result<OsString> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes.to_vec()))
}

/// On non-Unix (like Windows), require UTF-8.
#[cfg(not(unix))]
fn bytes_to_os_string(bytes: &[u8]) -> Result<OsString> {
    String::from_utf8(bytes.to_vec()).map(OsString::from).map_err(From::from)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use super::parse_reader;

    #[test]
    fn basic() {
        let (args, errs) = parse_reader(&b"\
# Test
--context=0
   --smart-case
-u


   # --bar
--foo
"[..]).unwrap();
        assert!(errs.is_empty());
        let args: Vec<String> =
            args.into_iter().map(|s| s.into_string().unwrap()).collect();
        assert_eq!(args, vec![
            "--context=0", "--smart-case", "-u", "--foo",
        ]);
    }

    // We test that we can handle invalid UTF-8 on Unix-like systems.
    #[test]
    #[cfg(unix)]
    fn error() {
        use std::os::unix::ffi::OsStringExt;

        let (args, errs) = parse_reader(&b"\
quux
foo\xFFbar
baz
"[..]).unwrap();
        assert!(errs.is_empty());
        assert_eq!(args, vec![
            OsString::from("quux"),
            OsString::from_vec(b"foo\xFFbar".to_vec()),
            OsString::from("baz"),
        ]);
    }

    // ... but test that invalid UTF-8 fails on Windows.
    #[test]
    #[cfg(not(unix))]
    fn error() {
        let (args, errs) = parse_reader(&b"\
quux
foo\xFFbar
baz
"[..]).unwrap();
        assert_eq!(errs.len(), 1);
        assert_eq!(args, vec![
            OsString::from("quux"),
            OsString::from("baz"),
        ]);
    }
}
