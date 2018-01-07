use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::io::{self, Read};
use std::path::Path;
use std::process::{self, Stdio};

use globset::{Glob, GlobSet, GlobSetBuilder};

/// A decompression command, contains the command to be spawned as well as any
/// necessary CLI args.
#[derive(Clone, Copy, Debug)]
struct DecompressionCommand {
    cmd: &'static str,
    args: &'static [&'static str],
}

impl DecompressionCommand {
    /// Create a new decompress command
    fn new(
        cmd: &'static str,
        args: &'static [&'static str],
    ) -> DecompressionCommand {
        DecompressionCommand {
            cmd, args
        }
    }
}

impl fmt::Display for DecompressionCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.cmd, self.args.join(" "))
    }
}

lazy_static! {
    static ref DECOMPRESSION_COMMANDS: HashMap<
        &'static str,
        DecompressionCommand,
    > = {
        let mut m = HashMap::new();

        const ARGS: &[&str] = &["-d", "-c"];
        m.insert("gz", DecompressionCommand::new("gzip", ARGS));
        m.insert("bz2", DecompressionCommand::new("bzip2", ARGS));
        m.insert("xz", DecompressionCommand::new("xz", ARGS));

        const LZMA_ARGS: &[&str] = &["--format=lzma", "-d", "-c"];
        m.insert("lzma", DecompressionCommand::new("xz", LZMA_ARGS));

        m
    };
    static ref SUPPORTED_COMPRESSION_FORMATS: GlobSet = {
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new("*.gz").unwrap());
        builder.add(Glob::new("*.bz2").unwrap());
        builder.add(Glob::new("*.xz").unwrap());
        builder.add(Glob::new("*.lzma").unwrap());
        builder.build().unwrap()
    };
    static ref TAR_ARCHIVE_FORMATS: GlobSet = {
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new("*.tar.gz").unwrap());
        builder.add(Glob::new("*.tar.xz").unwrap());
        builder.add(Glob::new("*.tar.bz2").unwrap());
        builder.add(Glob::new("*.tgz").unwrap());
        builder.add(Glob::new("*.txz").unwrap());
        builder.add(Glob::new("*.tbz2").unwrap());
        builder.build().unwrap()
    };
}

/// DecompressionReader provides an `io::Read` implementation for a limited
/// set of compression formats.
#[derive(Debug)]
pub struct DecompressionReader {
    cmd: DecompressionCommand,
    child: process::Child,
    done: bool,
}

impl DecompressionReader {
    /// Returns a handle to the stdout of the spawned decompression process for
    /// `path`, which can be directly searched in the worker. When the returned
    /// value is exhausted, the underlying process is reaped. If the underlying
    /// process fails, then its stderr is read and converted into a normal
    /// io::Error.
    ///
    /// If there is any error in spawning the decompression command, then
    /// return `None`, after outputting any necessary debug or error messages.
    pub fn from_path(path: &Path) -> Option<DecompressionReader> {
        if is_tar_archive(path) {
            debug!("{}: skipping tar archive", path.display());
            return None;
        }
        let extension = match path.extension().and_then(OsStr::to_str) {
            Some(extension) => extension,
            None => {
                debug!(
                    "{}: failed to get compresson extension", path.display());
                return None;
            }
        };
        let decompression_cmd = match DECOMPRESSION_COMMANDS.get(extension) {
            Some(cmd) => cmd,
            None => {
                debug!(
                    "{}: failed to get decompression command", path.display());
                return None;
            }
        };
        let cmd = process::Command::new(decompression_cmd.cmd)
            .args(decompression_cmd.args)
            .arg(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let child = match cmd {
            Ok(process) => process,
            Err(_) => {
                debug!(
                    "{}: decompression command '{}' not found",
                    path.display(), decompression_cmd.cmd);
                return None;
            }
        };
        Some(DecompressionReader::new(*decompression_cmd, child))
    }

    fn new(
        cmd: DecompressionCommand,
        child: process::Child,
    ) -> DecompressionReader {
        DecompressionReader {
            cmd: cmd,
            child: child,
            done: false,
        }
    }

    fn read_error(&mut self) -> io::Result<io::Error> {
        let mut errbytes = vec![];
        self.child.stderr.as_mut().unwrap().read_to_end(&mut errbytes)?;
        let errstr = String::from_utf8_lossy(&errbytes);
        let errstr = errstr.trim();

        Ok(if errstr.is_empty() {
            let msg = format!("decompression command failed: '{}'", self.cmd);
            io::Error::new(io::ErrorKind::Other, msg)
        } else {
            let msg = format!(
                "decompression command '{}' failed: {}", self.cmd, errstr);
            io::Error::new(io::ErrorKind::Other, msg)
        })
    }
}

impl io::Read for DecompressionReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        let nread = self.child.stdout.as_mut().unwrap().read(buf)?;
        if nread == 0 {
            self.done = true;
            // Reap the child now that we're done reading.
            // If the command failed, report stderr as an error.
            if !self.child.wait()?.success() {
                return Err(self.read_error()?);
            }
        }
        Ok(nread)
    }
}

/// Returns true if the given path contains a supported compression format or
/// is a TAR archive.
pub fn is_compressed(path: &Path) -> bool {
    is_supported_compression_format(path) || is_tar_archive(path)
}

/// Returns true if the given path matches any one of the supported compression
/// formats
fn is_supported_compression_format(path: &Path) -> bool {
    SUPPORTED_COMPRESSION_FORMATS.is_match(path)
}

/// Returns true if the given path matches any of the known TAR file formats.
fn is_tar_archive(path: &Path) -> bool {
    TAR_ARCHIVE_FORMATS.is_match(path)
}
