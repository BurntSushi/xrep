use std::error;
use std::fmt;
use std::io::{self, Read};
use std::iter;
use std::process;
use std::thread::{self, JoinHandle};

/// An error that can occur while running a command and reading its output.
///
/// This error can be seamlessly converted to an `io::Error` via a `From`
/// implementation.
#[derive(Debug)]
pub struct CommandError {
    kind: CommandErrorKind,
}

#[derive(Debug)]
enum CommandErrorKind {
    Io(io::Error),
    Stderr(Vec<u8>),
}

impl CommandError {
    /// Create an error from an I/O error.
    pub(crate) fn io(ioerr: io::Error) -> CommandError {
        CommandError { kind: CommandErrorKind::Io(ioerr) }
    }

    /// Create an error from the contents of stderr (which may be empty).
    pub(crate) fn stderr(bytes: Vec<u8>) -> CommandError {
        CommandError { kind: CommandErrorKind::Stderr(bytes) }
    }
}

impl error::Error for CommandError {
    fn description(&self) -> &str {
        "command error"
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            CommandErrorKind::Io(ref e) => e.fmt(f),
            CommandErrorKind::Stderr(ref bytes) => {
                let msg = String::from_utf8_lossy(bytes);
                if msg.trim().is_empty() {
                    write!(f, "<stderr is empty>")
                } else {
                    let div = iter::repeat('-').take(79).collect::<String>();
                    write!(
                        f,
                        "\n{div}\n{msg}\n{div}",
                        div = div,
                        msg = msg.trim()
                    )
                }
            }
        }
    }
}

impl From<io::Error> for CommandError {
    fn from(ioerr: io::Error) -> CommandError {
        CommandError { kind: CommandErrorKind::Io(ioerr) }
    }
}

impl From<CommandError> for io::Error {
    fn from(cmderr: CommandError) -> io::Error {
        match cmderr.kind {
            CommandErrorKind::Io(ioerr) => ioerr,
            CommandErrorKind::Stderr(_) => {
                io::Error::new(io::ErrorKind::Other, cmderr)
            }
        }
    }
}

/// Configures and builds a streaming reader for process output.
#[derive(Clone, Debug, Default)]
pub struct CommandReaderBuilder {
    async_stderr: bool,
}

impl CommandReaderBuilder {
    /// Create a new builder with the default configuration.
    pub fn new() -> CommandReaderBuilder {
        CommandReaderBuilder::default()
    }

    /// Build a new streaming reader for the given command's output.
    ///
    /// The caller should set everything that's required on the given command
    /// before building a reader, such as its arguments, environment and
    /// current working directory. Settings such as the stdout and stderr (but
    /// not stdin) pipes will be overridden so that they can be controlled by
    /// the reader.
    ///
    /// If there was a problem spawning the given command, then its error is
    /// returned.
    pub fn build(
        &self,
        command: &mut process::Command,
    ) -> Result<CommandReader, CommandError> {
        let mut child = command
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        let stderr = if self.async_stderr {
            StderrReader::async(child.stderr.take().unwrap())
        } else {
            StderrReader::sync(child.stderr.take().unwrap())
        };
        Ok(CommandReader {
            child: child,
            stdout: stdout,
            stderr: stderr,
            done: false,
        })
    }

    /// When enabled, the reader will asynchronously read the contents of the
    /// command's stderr output. When disabled, stderr is only read after the
    /// stdout stream has been exhausted (or if the process quits with an error
    /// code).
    ///
    /// Note that when enabled, this may require launching an additional
    /// thread in order to read stderr. This is done so that the process being
    /// executed is never blocked from writing to stdout or stderr. If this is
    /// disabled, then it is possible for the process to fill up the stderr
    /// buffer and deadlock.
    ///
    /// This is enabled by default.
    pub fn async_stderr(&mut self, yes: bool) -> &mut CommandReaderBuilder {
        self.async_stderr = yes;
        self
    }
}

/// A streaming reader for a command's output.
///
/// The purpose of this reader is to provide an easy way to execute processes
/// whose stdout is read in a streaming way while also making the processes'
/// stderr available when the process fails with an exit code. This makes it
/// possible to execute processes while surfacing the underlying failure mode
/// in the case of an error.
///
/// Moreover, by default, this reader will asynchronously read the processes'
/// stderr. This prevents subtle deadlocking bugs for noisy processes that
/// write a lot to stderr. Currently, the entire contents of stderr is read
/// on to the heap.
///
/// # Example
///
/// This example shows how to invoke `gzip` to decompress the contents of a
/// file. If the `gzip` command reports a failing exit status, then its stderr
/// is returned as an error.
///
/// ```no_run
/// use std::io::Read;
/// use std::process::Command;
/// use grep_cli::CommandReader;
///
/// # fn example() -> Result<(), Box<::std::error::Error>> {
/// let mut cmd = Command::new("gzip");
/// cmd.arg("-d").arg("-c").arg("/usr/share/man/man1/ls.1.gz");
///
/// let mut rdr = CommandReader::new(&mut cmd)?;
/// let mut contents = vec![];
/// rdr.read_to_end(&mut contents)?;
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct CommandReader {
    child: process::Child,
    stdout: process::ChildStdout,
    stderr: StderrReader,
    done: bool,
}

impl CommandReader {
    /// Create a new streaming reader for the given command using the default
    /// configuration.
    ///
    /// The caller should set everything that's required on the given command
    /// before building a reader, such as its arguments, environment and
    /// current working directory. Settings such as the stdout and stderr (but
    /// not stdin) pipes will be overridden so that they can be controlled by
    /// the reader.
    ///
    /// If there was a problem spawning the given command, then its error is
    /// returned.
    ///
    /// If the caller requires additional configuration for the reader
    /// returned, then use
    /// [`CommandReaderBuilder`](struct.CommandReaderBuilder.html).
    pub fn new(
        cmd: &mut process::Command,
    ) -> Result<CommandReader, CommandError> {
        CommandReaderBuilder::new().build(cmd)
    }
}

impl io::Read for CommandReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        let nread = self.stdout.read(buf)?;
        if nread == 0 {
            self.done = true;
            // Reap the child now that we're done reading. If the command
            // failed, report stderr as an error.
            if !self.child.wait()?.success() {
                return Err(io::Error::from(self.stderr.read_to_end()));
            }
        }
        Ok(nread)
    }
}

/// A reader that encapsulates the asynchronous or synchronous reading of
/// stderr.
#[derive(Debug)]
enum StderrReader {
    Async(Option<JoinHandle<CommandError>>),
    Sync(process::ChildStderr),
}

impl StderrReader {
    /// Create a reader for stderr that reads contents asynchronously.
    fn async(mut stderr: process::ChildStderr) -> StderrReader {
        let handle =
            thread::spawn(move || stderr_to_command_error(&mut stderr));
        StderrReader::Async(Some(handle))
    }

    /// Create a reader for stderr that reads contents synchronously.
    fn sync(stderr: process::ChildStderr) -> StderrReader {
        StderrReader::Sync(stderr)
    }

    /// Consumes all of stderr on to the heap and returns it as an error.
    ///
    /// If there was a problem reading stderr itself, then this returns an I/O
    /// command error.
    fn read_to_end(&mut self) -> CommandError {
        match *self {
            StderrReader::Async(ref mut handle) => {
                let handle = handle
                    .take()
                    .expect("read_to_end cannot be called more than once");
                handle.join().expect("stderr reading thread does not panic")
            }
            StderrReader::Sync(ref mut stderr) => {
                stderr_to_command_error(stderr)
            }
        }
    }
}

fn stderr_to_command_error(stderr: &mut process::ChildStderr) -> CommandError {
    let mut bytes = vec![];
    match stderr.read_to_end(&mut bytes) {
        Ok(_) => CommandError::stderr(bytes),
        Err(err) => CommandError::io(err),
    }
}
