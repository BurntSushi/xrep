use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::collections::HashMap;
use std::process::Command;
use std::process::Stdio;
use std::process::ChildStdout;
use globset::{Glob, GlobSet, GlobSetBuilder};

/// A decompression command, contains the command to be spawned as well as any
/// necessary CLI args
struct DecompressionCommand {
    cmd: &'static str,
    args: Vec<&'static str>
}

impl DecompressionCommand {
    /// Create a new decompress command
    fn new(cmd: &'static str, args: Vec<&'static str>) -> DecompressionCommand {
        DecompressionCommand {
            cmd, args
        }
    }
}

lazy_static! {
    static ref DECOMPRESSION_COMMANDS: HashMap<&'static str, DecompressionCommand> = {
        let mut m = HashMap::new();
        m.insert("gz", DecompressionCommand::new("gunzip", vec!["-c"]));
        m.insert("bz2", DecompressionCommand::new("bunzip2", vec!["-c"]));
        m.insert("xz", DecompressionCommand::new("unxz", vec!["-c"]));
        m.insert("lzma", DecompressionCommand::new("unlzma", vec!["-c"]));
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

/// Returns a handle to the stdout of the spawned decompression process for
/// `path`, which can be directly searched in the worker.
///
/// If there is any error in spawning the decompression command, then return
/// `None`, after outputting any necessary debug or error messages.
pub fn get_reader(path: &Path, no_messages: bool) -> Option<ChildStdout> {
    if is_tar_archive(path) {
        debug!("Tar archives are currently unsupported: {}", path.display());
        None
    } else {
        let extension = path.extension().and_then(OsStr::to_str).unwrap();
        let decompression_command = DECOMPRESSION_COMMANDS.get(extension).unwrap();
        let cmd = Command::new(decompression_command.cmd)
            .args(decompression_command.args.as_slice())
            .arg(path.as_os_str())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        match cmd {
            Ok(process) => {
                let mut error = Vec::new();
                // Unwrapping is safe here since we captured the stderr above
                let mut stderr = process.stderr.unwrap();
                // We drain the stderr of the child to ensure no error occurred,
                // and then return the handle to stdout if that is the case
                stderr.read_to_end(&mut error).unwrap();
                if error.is_empty() {
                    process.stdout
                } else {
                    if !no_messages {
                        eprintln!("Error occurred while trying to decompress path: \
                                   {}: {}", path.display(),
                                  String::from_utf8_lossy(error.as_slice()));
                    }
                    None
                }
            },
            Err(_) => {
                debug!("Decompress command '{}' not found for path: {}",
                       decompression_command.cmd, path.display());
                None
            }
        }
    }
}

/// Returns true if the given path contains a supported compression format or is a
/// TAR archive
pub fn is_compressed(path: &Path) -> bool {
    is_supported_compression_format(path) || is_tar_archive(path)
}

/// Returns true if the given path matches any one of the supported compression
/// formats
fn is_supported_compression_format(path: &Path) -> bool {
    SUPPORTED_COMPRESSION_FORMATS.is_match(path)
}

/// Returns true if the given path matches any of the known TAR file formats
fn is_tar_archive(path: &Path) -> bool {
    TAR_ARCHIVE_FORMATS.is_match(path)
}
