use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{self, Stdio};

/// PreprocessorReader provides an `io::Read` impl to read kids output.
#[derive(Debug)]
pub struct PreprocessorReader {
    cmd: PathBuf,
    path: PathBuf,
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
    /// return the corresponding error.
    pub fn from_cmd_path(
        cmd: PathBuf,
        path: &Path,
    ) -> io::Result<PreprocessorReader> {
        let child = process::Command::new(&cmd)
            .arg(path)
            .stdin(Stdio::from(File::open(path)?))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "error running preprocessor command '{}': {}",
                        cmd.display(),
                        err,
                    ),
                )
            })?;
        Ok(PreprocessorReader {
            cmd: cmd,
            path: path.to_path_buf(),
            child: child,
            done: false,
        })
    }

    fn read_error(&mut self) -> io::Result<io::Error> {
        let mut errbytes = vec![];
        self.child.stderr.as_mut().unwrap().read_to_end(&mut errbytes)?;
        let errstr = String::from_utf8_lossy(&errbytes);
        let errstr = errstr.trim();

        Ok(if errstr.is_empty() {
            let msg = format!(
                "preprocessor command failed: '{} {}'",
                self.cmd.display(),
                self.path.display(),
            );
            io::Error::new(io::ErrorKind::Other, msg)
        } else {
            let msg = format!(
                "preprocessor command failed: '{} {}': {}",
                self.cmd.display(),
                self.path.display(),
                errstr,
            );
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
