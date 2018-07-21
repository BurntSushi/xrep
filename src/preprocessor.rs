use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{self, Stdio};

/// `cmd` is the str argument to -P,--Preprocessor option.

/// PreprocessorReader provides an `io::Read` impl to read kids output.
#[derive(Debug)]
pub struct PreprocessorReader {
    cmd: PathBuf,
    child: process::Child,
    done: bool,
}

impl PreprocessorReader {
    /// Returns a handle to the stdout of the spawned preprocessor process for
    /// `path`, which can be directly searched in the worker. When the returned
    /// value is exhausted, the underlying process is reaped. If the underlying
    /// process fails, then its stderr is read and converted into a normal
    /// io::Error.
    ///
    /// If there is any error in spawning the preprocessor command, then
    /// return `None`, after outputting any necessary debug or error messages.
    pub fn from_cmd_path(c: PathBuf, path: &Path) -> Option<PreprocessorReader> {
        let cmd = process::Command::new(c.clone())
            .arg(path)
            //.stdin(Stdio::open(path)) //Unnecessary since stdin inherited
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let child = match cmd {
            Ok(process) => process,
            Err(_) => {
                debug!(
                    "{}: preprocessor command '{}' not found",
                    path.display(), c.display());
                return None;
            }
        };
        Some(PreprocessorReader::new(c, child))
    }

    fn new(
        cmd: PathBuf,
        child: process::Child,
    ) -> PreprocessorReader {
        PreprocessorReader {
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
            let msg = format!("preprocessor command failed: '{}'", self.cmd.display());
            io::Error::new(io::ErrorKind::Other, msg)
        } else {
            let msg = format!(
                "preprocessor command '{}' failed: {}", self.cmd.display(), errstr);
            io::Error::new(io::ErrorKind::Other, msg)
        })
    }
}

impl io::Read for PreprocessorReader {
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
