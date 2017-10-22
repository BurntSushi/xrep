/*!
This crate provides a cross platform abstraction for writing colored text to
a terminal. Colors are written using either ANSI escape sequences or by
communicating with a Windows console. Much of this API was motivated by use
inside command line applications, where colors or styles can be configured
by the end user and/or the environment.

This crate also provides platform independent support for writing colored text
to an in memory buffer. While this is easy to do with ANSI escape sequences
(because they are in the buffer themselves), it is trickier to do with the
Windows console API, which requires synchronous communication.

# Organization

The `WriteColor` trait extends the `io::Write` trait with methods for setting
colors or resetting them.

`StandardStream` and `StandardStreamLock` both satisfy `WriteColor` and are
analogous to `std::io::Stdout` and `std::io::StdoutLock`, or `std::io::Stderr`
and `std::io::StderrLock`.

`Buffer` is an in memory buffer that supports colored text. In a parallel
program, each thread might write to its own buffer. A buffer can be printed to
using a `BufferWriter`. The advantage of this design is that each thread can
work in parallel on a buffer without having to synchronize access to global
resources such as the Windows console. Moreover, this design also prevents
interleaving of buffer output.

`Ansi` and `NoColor` both satisfy `WriteColor` for arbitrary implementors of
`io::Write`. These types are useful when you know exactly what you need. An
analogous type for the Windows console is not provided since it cannot exist.

# Example: using `StandardStream`

The `StandardStream` type in this crate works similarly to `std::io::Stdout`,
except it is augmented with methods for coloring by the `WriteColor` trait.
For example, to write some green text:

```rust,no_run
# fn test() -> Result<(), Box<::std::error::Error>> {
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

let mut stdout = StandardStream::stdout(ColorChoice::Always);
try!(stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))));
try!(writeln!(&mut stdout, "green text!"));
# Ok(()) }
```

# Example: using `BufferWriter`

A `BufferWriter` can create buffers and write buffers to stdout or stderr. It
does *not* implement `io::Write` or `WriteColor` itself. Instead, `Buffer`
implements `io::Write` and `io::WriteColor`.

This example shows how to print some green text to stderr.

```rust,no_run
# fn test() -> Result<(), Box<::std::error::Error>> {
use std::io::Write;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

let mut bufwtr = BufferWriter::stderr(ColorChoice::Always);
let mut buffer = bufwtr.buffer();
try!(buffer.set_color(ColorSpec::new().set_fg(Some(Color::Green))));
try!(writeln!(&mut buffer, "green text!"));
try!(bufwtr.print(&buffer));
# Ok(()) }
```
*/

#![deny(missing_docs)]

#[cfg(windows)]
extern crate wincolor;

use std::env;
use std::error;
use std::fmt;
use std::io::{self, Write};
use std::str::FromStr;
#[cfg(windows)]
use std::sync::{Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, Ordering};

/// This trait describes the behavior of writers that support colored output.
pub trait WriteColor: io::Write {
    /// Returns true if and only if the underlying writer supports colors.
    fn supports_color(&self) -> bool;

    /// Set the color settings of the writer.
    ///
    /// Subsequent writes to this writer will use these settings until either
    /// `reset` is called or new color settings are set.
    ///
    /// If there was a problem setting the color settings, then an error is
    /// returned.
    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()>;

    /// Reset the current color settings to their original settings.
    ///
    /// If there was a problem resetting the color settings, then an error is
    /// returned.
    fn reset(&mut self) -> io::Result<()>;
}

impl<'a, T: WriteColor> WriteColor for &'a mut T {
    fn supports_color(&self) -> bool { (&**self).supports_color() }
    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        (&mut **self).set_color(spec)
    }
    fn reset(&mut self) -> io::Result<()> { (&mut **self).reset() }
}

/// ColorChoice represents the color preferences of an end user.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorChoice {
    /// Try very hard to emit colors. This includes emitting ANSI colors
    /// on Windows if the console API is unavailable.
    Always,
    /// AlwaysAnsi is like Always, except it never tries to use anything other
    /// than emitting ANSI color codes.
    AlwaysAnsi,
    /// Try to use colors, but don't force the issue. If the console isn't
    /// available on Windows, or if TERM=dumb, for example, then don't use
    /// colors.
    Auto,
    /// Never emit colors.
    Never,
}

impl ColorChoice {
    /// Returns true if we should attempt to write colored output.
    #[cfg(not(windows))]
    fn should_attempt_color(&self) -> bool {
        match *self {
            ColorChoice::Always => true,
            ColorChoice::AlwaysAnsi => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                match env::var("TERM") {
                    Err(_) => false,
                    Ok(k) => k != "dumb",
                }
            }
        }
    }

    /// Returns true if we should attempt to write colored output.
    #[cfg(windows)]
    fn should_attempt_color(&self) -> bool {
        match *self {
            ColorChoice::Always => true,
            ColorChoice::AlwaysAnsi => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                match env::var("TERM") {
                    Err(_) => true,
                    Ok(k) => k != "dumb",
                }
            }
        }
    }

    /// Returns true if this choice should forcefully use ANSI color codes.
    ///
    /// It's possible that ANSI is still the correct choice even if this
    /// returns false.
    #[cfg(windows)]
    fn should_ansi(&self) -> bool {
        match *self {
            ColorChoice::Always => false,
            ColorChoice::AlwaysAnsi => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                match env::var("TERM") {
                    Err(_) => false,
                    // cygwin doesn't seem to support ANSI escape sequences
                    // and instead has its own variety. However, the Windows
                    // console API may be available.
                    Ok(k) => k != "dumb" && k != "cygwin",
                }
            }
        }
    }
}

/// `std::io` implements `Stdout` and `Stderr` (and their `Lock` variants) as
/// separate types, which makes it difficult to abstract over them. We use
/// some simple internal enum types to work around this.

enum StandardStreamType {
    Stdout,
    Stderr,
}

enum IoStandardStream {
    Stdout(io::Stdout),
    Stderr(io::Stderr),
}

impl IoStandardStream {
    fn new(sty: StandardStreamType) -> IoStandardStream {
        match sty {
            StandardStreamType::Stdout => {
                IoStandardStream::Stdout(io::stdout())
            }
            StandardStreamType::Stderr => {
                IoStandardStream::Stderr(io::stderr())
            }
        }
    }

    fn lock(&self) -> IoStandardStreamLock {
        match *self {
            IoStandardStream::Stdout(ref s) => {
                IoStandardStreamLock::StdoutLock(s.lock())
            }
            IoStandardStream::Stderr(ref s) => {
                IoStandardStreamLock::StderrLock(s.lock())
            }
        }
    }
}

impl io::Write for IoStandardStream {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match *self {
            IoStandardStream::Stdout(ref mut s) => s.write(b),
            IoStandardStream::Stderr(ref mut s) => s.write(b),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            IoStandardStream::Stdout(ref mut s) => s.flush(),
            IoStandardStream::Stderr(ref mut s) => s.flush(),
        }
    }
}

/// Same rigmarole for the locked variants of the standard streams.

enum IoStandardStreamLock<'a> {
    StdoutLock(io::StdoutLock<'a>),
    StderrLock(io::StderrLock<'a>),
}

impl<'a> io::Write for IoStandardStreamLock<'a> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match *self {
            IoStandardStreamLock::StdoutLock(ref mut s) => s.write(b),
            IoStandardStreamLock::StderrLock(ref mut s) => s.write(b),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            IoStandardStreamLock::StdoutLock(ref mut s) => s.flush(),
            IoStandardStreamLock::StderrLock(ref mut s) => s.flush(),
        }
    }
}

/// Satisfies `io::Write` and `WriteColor`, and supports optional coloring
/// to either of the standard output streams, stdout and stderr.
pub struct StandardStream {
    wtr: LossyStandardStream<WriterInner<IoStandardStream>>,
}

/// `StandardStreamLock` is a locked reference to a `StandardStream`.
///
/// This implements the `io::Write` and `WriteColor` traits, and is constructed
/// via the `Write::lock` method.
///
/// The lifetime `'a` refers to the lifetime of the corresponding
/// `StandardStream`.
pub struct StandardStreamLock<'a> {
    wtr: LossyStandardStream<WriterInnerLock<'a, IoStandardStreamLock<'a>>>,
}

/// WriterInner is a (limited) generic representation of a writer. It is
/// limited because W should only ever be stdout/stderr on Windows.
enum WriterInner<W> {
    NoColor(NoColor<W>),
    Ansi(Ansi<W>),
    #[cfg(windows)]
    Windows { wtr: W, console: Mutex<wincolor::Console> },
}

/// WriterInnerLock is a (limited) generic representation of a writer. It is
/// limited because W should only ever be stdout/stderr on Windows.
enum WriterInnerLock<'a, W> {
    NoColor(NoColor<W>),
    Ansi(Ansi<W>),
    /// What a gross hack. On Windows, we need to specify a lifetime for the
    /// console when in a locked state, but obviously don't need to do that
    /// on Unix, which makes the `'a` unused. To satisfy the compiler, we need
    /// a PhantomData.
    #[allow(dead_code)]
    Unreachable(::std::marker::PhantomData<&'a ()>),
    #[cfg(windows)]
    Windows { wtr: W, console: MutexGuard<'a, wincolor::Console> },
}

impl StandardStream {
    /// Create a new `StandardStream` with the given color preferences.
    ///
    /// The specific color/style settings can be configured when writing via
    /// the `WriteColor` trait.
    #[cfg(not(windows))]
    fn create(sty: StandardStreamType, choice: ColorChoice) -> StandardStream {
        let wtr =
            if choice.should_attempt_color() {
                WriterInner::Ansi(Ansi(IoStandardStream::new(sty)))
            } else {
                WriterInner::NoColor(NoColor(IoStandardStream::new(sty)))
            };
        StandardStream { wtr: LossyStandardStream::new(wtr) }
    }

    /// Create a new `StandardStream` with the given color preferences.
    ///
    /// If coloring is desired and a Windows console could not be found, then
    /// ANSI escape sequences are used instead.
    ///
    /// The specific color/style settings can be configured when writing via
    /// the `WriteColor` trait.
    #[cfg(windows)]
    fn create(sty: StandardStreamType, choice: ColorChoice) -> StandardStream {
        let con = match sty {
            StandardStreamType::Stdout => wincolor::Console::stdout(),
            StandardStreamType::Stderr => wincolor::Console::stderr(),
        };
        let is_win_console = con.is_ok();
        let wtr =
            if choice.should_attempt_color() {
                if choice.should_ansi() {
                    WriterInner::Ansi(Ansi(IoStandardStream::new(sty)))
                } else if let Ok(console) = con {
                    WriterInner::Windows {
                        wtr: IoStandardStream::new(sty),
                        console: Mutex::new(console),
                    }
                } else {
                    WriterInner::Ansi(Ansi(IoStandardStream::new(sty)))
                }
            } else {
                WriterInner::NoColor(NoColor(IoStandardStream::new(sty)))
            };
        StandardStream {
            wtr: LossyStandardStream::new(wtr).is_console(is_win_console),
        }
    }

    /// Create a new `StandardStream` with the given color preferences that
    /// writes to standard output.
    ///
    /// On Windows, if coloring is desired and a Windows console could not be
    /// found, then ANSI escape sequences are used instead.
    ///
    /// The specific color/style settings can be configured when writing via
    /// the `WriteColor` trait.
    pub fn stdout(choice: ColorChoice) -> StandardStream {
        StandardStream::create(StandardStreamType::Stdout, choice)
    }

    /// Create a new `StandardStream` with the given color preferences that
    /// writes to standard error.
    ///
    /// On Windows, if coloring is desired and a Windows console could not be
    /// found, then ANSI escape sequences are used instead.
    ///
    /// The specific color/style settings can be configured when writing via
    /// the `WriteColor` trait.
    pub fn stderr(choice: ColorChoice) -> StandardStream {
        StandardStream::create(StandardStreamType::Stderr, choice)
    }

    /// Lock the underlying writer.
    ///
    /// The lock guard returned also satisfies `io::Write` and
    /// `WriteColor`.
    ///
    /// This method is **not reentrant**. It may panic if `lock` is called
    /// while a `StandardStreamLock` is still alive.
    pub fn lock(&self) -> StandardStreamLock {
        StandardStreamLock::from_stream(self)
    }
}

impl<'a> StandardStreamLock<'a> {
    #[cfg(not(windows))]
    fn from_stream(stream: &StandardStream) -> StandardStreamLock {
        let locked = match *stream.wtr.get_ref() {
            WriterInner::NoColor(ref w) => {
                WriterInnerLock::NoColor(NoColor(w.0.lock()))
            }
            WriterInner::Ansi(ref w) => {
                WriterInnerLock::Ansi(Ansi(w.0.lock()))
            }
        };
        StandardStreamLock { wtr: stream.wtr.wrap(locked) }
    }

    #[cfg(windows)]
    fn from_stream(stream: &StandardStream) -> StandardStreamLock {
        let locked = match *stream.wtr.get_ref() {
            WriterInner::NoColor(ref w) => {
                WriterInnerLock::NoColor(NoColor(w.0.lock()))
            }
            WriterInner::Ansi(ref w) => {
                WriterInnerLock::Ansi(Ansi(w.0.lock()))
            }
            #[cfg(windows)]
            WriterInner::Windows { ref wtr, ref console } => {
                WriterInnerLock::Windows {
                    wtr: wtr.lock(),
                    console: console.lock().unwrap(),
                }
            }
        };
        StandardStreamLock { wtr: stream.wtr.wrap(locked) }
    }
}

impl io::Write for StandardStream {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> { self.wtr.write(b) }
    fn flush(&mut self) -> io::Result<()> { self.wtr.flush() }
}

impl WriteColor for StandardStream {
    fn supports_color(&self) -> bool { self.wtr.supports_color() }
    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.wtr.set_color(spec)
    }
    fn reset(&mut self) -> io::Result<()> { self.wtr.reset() }
}

impl<'a> io::Write for StandardStreamLock<'a> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> { self.wtr.write(b) }
    fn flush(&mut self) -> io::Result<()> { self.wtr.flush() }
}

impl<'a> WriteColor for StandardStreamLock<'a> {
    fn supports_color(&self) -> bool { self.wtr.supports_color() }
    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.wtr.set_color(spec)
    }
    fn reset(&mut self) -> io::Result<()> { self.wtr.reset() }
}

impl<W: io::Write> io::Write for WriterInner<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            WriterInner::NoColor(ref mut wtr) => wtr.write(buf),
            WriterInner::Ansi(ref mut wtr) => wtr.write(buf),
            #[cfg(windows)]
            WriterInner::Windows { ref mut wtr, .. } => wtr.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            WriterInner::NoColor(ref mut wtr) => wtr.flush(),
            WriterInner::Ansi(ref mut wtr) => wtr.flush(),
            #[cfg(windows)]
            WriterInner::Windows { ref mut wtr, .. } => wtr.flush(),
        }
    }
}

impl<W: io::Write> WriteColor for WriterInner<W> {
    fn supports_color(&self) -> bool {
        match *self {
            WriterInner::NoColor(_) => false,
            WriterInner::Ansi(_) => true,
            #[cfg(windows)]
            WriterInner::Windows { .. } => true,
        }
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        match *self {
            WriterInner::NoColor(ref mut wtr) => wtr.set_color(spec),
            WriterInner::Ansi(ref mut wtr) => wtr.set_color(spec),
            #[cfg(windows)]
            WriterInner::Windows { ref mut wtr, ref console } => {
                try!(wtr.flush());
                let mut console = console.lock().unwrap();
                spec.write_console(&mut *console)
            }
        }
    }

    fn reset(&mut self) -> io::Result<()> {
        match *self {
            WriterInner::NoColor(ref mut wtr) => wtr.reset(),
            WriterInner::Ansi(ref mut wtr) => wtr.reset(),
            #[cfg(windows)]
            WriterInner::Windows { ref mut wtr, ref mut console } => {
                try!(wtr.flush());
                try!(console.lock().unwrap().reset());
                Ok(())
            }
        }
    }
}

impl<'a, W: io::Write> io::Write for WriterInnerLock<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            WriterInnerLock::Unreachable(_) => unreachable!(),
            WriterInnerLock::NoColor(ref mut wtr) => wtr.write(buf),
            WriterInnerLock::Ansi(ref mut wtr) => wtr.write(buf),
            #[cfg(windows)]
            WriterInnerLock::Windows { ref mut wtr, .. } => wtr.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            WriterInnerLock::Unreachable(_) => unreachable!(),
            WriterInnerLock::NoColor(ref mut wtr) => wtr.flush(),
            WriterInnerLock::Ansi(ref mut wtr) => wtr.flush(),
            #[cfg(windows)]
            WriterInnerLock::Windows { ref mut wtr, .. } => wtr.flush(),
        }
    }
}

impl<'a, W: io::Write> WriteColor for WriterInnerLock<'a, W> {
    fn supports_color(&self) -> bool {
        match *self {
            WriterInnerLock::Unreachable(_) => unreachable!(),
            WriterInnerLock::NoColor(_) => false,
            WriterInnerLock::Ansi(_) => true,
            #[cfg(windows)]
            WriterInnerLock::Windows { .. } => true,
        }
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        match *self {
            WriterInnerLock::Unreachable(_) => unreachable!(),
            WriterInnerLock::NoColor(ref mut wtr) => wtr.set_color(spec),
            WriterInnerLock::Ansi(ref mut wtr) => wtr.set_color(spec),
            #[cfg(windows)]
            WriterInnerLock::Windows { ref mut wtr, ref mut console } => {
                try!(wtr.flush());
                spec.write_console(console)
            }
        }
    }

    fn reset(&mut self) -> io::Result<()> {
        match *self {
            WriterInnerLock::Unreachable(_) => unreachable!(),
            WriterInnerLock::NoColor(ref mut wtr) => wtr.reset(),
            WriterInnerLock::Ansi(ref mut wtr) => wtr.reset(),
            #[cfg(windows)]
            WriterInnerLock::Windows { ref mut wtr, ref mut console } => {
                try!(wtr.flush());
                try!(console.reset());
                Ok(())
            }
        }
    }
}

/// Writes colored buffers to stdout or stderr.
///
/// Writable buffers can be obtained by calling `buffer` on a `BufferWriter`.
///
/// This writer works with terminals that support ANSI escape sequences or
/// with a Windows console.
///
/// It is intended for a `BufferWriter` to be put in an `Arc` and written to
/// from multiple threads simultaneously.
pub struct BufferWriter {
    stream: LossyStandardStream<IoStandardStream>,
    printed: AtomicBool,
    separator: Option<Vec<u8>>,
    color_choice: ColorChoice,
    #[cfg(windows)]
    console: Option<Mutex<wincolor::Console>>,
}

impl BufferWriter {
    /// Create a new `BufferWriter` that writes to a standard stream with the
    /// given color preferences.
    ///
    /// The specific color/style settings can be configured when writing to
    /// the buffers themselves.
    #[cfg(not(windows))]
    fn create(sty: StandardStreamType, choice: ColorChoice) -> BufferWriter {
        BufferWriter {
            stream: LossyStandardStream::new(IoStandardStream::new(sty)),
            printed: AtomicBool::new(false),
            separator: None,
            color_choice: choice,
        }
    }

    /// Create a new `BufferWriter` that writes to a standard stream with the
    /// given color preferences.
    ///
    /// If coloring is desired and a Windows console could not be found, then
    /// ANSI escape sequences are used instead.
    ///
    /// The specific color/style settings can be configured when writing to
    /// the buffers themselves.
    #[cfg(windows)]
    fn create(sty: StandardStreamType, choice: ColorChoice) -> BufferWriter {
        let con = match sty {
            StandardStreamType::Stdout => wincolor::Console::stdout(),
            StandardStreamType::Stderr => wincolor::Console::stderr(),
        }.ok().map(Mutex::new);
        let stream = LossyStandardStream::new(IoStandardStream::new(sty))
            .is_console(con.is_some());
        BufferWriter {
            stream: stream,
            printed: AtomicBool::new(false),
            separator: None,
            color_choice: choice,
            console: con,
        }
    }

    /// Create a new `BufferWriter` that writes to stdout with the given
    /// color preferences.
    ///
    /// On Windows, if coloring is desired and a Windows console could not be
    /// found, then ANSI escape sequences are used instead.
    ///
    /// The specific color/style settings can be configured when writing to
    /// the buffers themselves.
    pub fn stdout(choice: ColorChoice) -> BufferWriter {
        BufferWriter::create(StandardStreamType::Stdout, choice)
    }

    /// Create a new `BufferWriter` that writes to stderr with the given
    /// color preferences.
    ///
    /// On Windows, if coloring is desired and a Windows console could not be
    /// found, then ANSI escape sequences are used instead.
    ///
    /// The specific color/style settings can be configured when writing to
    /// the buffers themselves.
    pub fn stderr(choice: ColorChoice) -> BufferWriter {
        BufferWriter::create(StandardStreamType::Stderr, choice)
    }

    /// If set, the separator given is printed between buffers. By default, no
    /// separator is printed.
    ///
    /// The default value is `None`.
    pub fn separator(&mut self, sep: Option<Vec<u8>>) {
        self.separator = sep;
    }

    /// Creates a new `Buffer` with the current color preferences.
    ///
    /// A `Buffer` satisfies both `io::Write` and `WriteColor`. A `Buffer` can
    /// be printed using the `print` method.
    #[cfg(not(windows))]
    pub fn buffer(&self) -> Buffer {
        Buffer::new(self.color_choice)
    }

    /// Creates a new `Buffer` with the current color preferences.
    ///
    /// A `Buffer` satisfies both `io::Write` and `WriteColor`. A `Buffer` can
    /// be printed using the `print` method.
    #[cfg(windows)]
    pub fn buffer(&self) -> Buffer {
        Buffer::new(self.color_choice, self.console.is_some())
    }

    /// Prints the contents of the given buffer.
    ///
    /// It is safe to call this from multiple threads simultaneously. In
    /// particular, all buffers are written atomically. No interleaving will
    /// occur.
    pub fn print(&self, buf: &Buffer) -> io::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }
        let mut stream = self.stream.wrap(self.stream.get_ref().lock());
        if let Some(ref sep) = self.separator {
            if self.printed.load(Ordering::SeqCst) {
                try!(stream.write_all(sep));
                try!(stream.write_all(b"\n"));
            }
        }
        match buf.0 {
            BufferInner::NoColor(ref b) => try!(stream.write_all(&b.0)),
            BufferInner::Ansi(ref b) => try!(stream.write_all(&b.0)),
            #[cfg(windows)]
            BufferInner::Windows(ref b) => {
                // We guarantee by construction that we have a console here.
                // Namely, a BufferWriter is the only way to produce a Buffer.
                let console_mutex = self.console.as_ref()
                    .expect("got Windows buffer but have no Console");
                let mut console = console_mutex.lock().unwrap();
                try!(b.print(&mut *console, &mut stream));
            }
        }
        self.printed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Write colored text to memory.
///
/// `Buffer` is a platform independent abstraction for printing colored text to
/// an in memory buffer. When the buffer is printed using a `BufferWriter`, the
/// color information will be applied to the output device (a tty on Unix and a
/// console on Windows).
///
/// A `Buffer` is typically created by calling the `BufferWriter.buffer`
/// method, which will take color preferences and the environment into
/// account. However, buffers can also be manually created using `no_color`,
/// `ansi` or `console` (on Windows).
pub struct Buffer(BufferInner);

/// BufferInner is an enumeration of different buffer types.
enum BufferInner {
    /// No coloring information should be applied. This ignores all coloring
    /// directives.
    NoColor(NoColor<Vec<u8>>),
    /// Apply coloring using ANSI escape sequences embedded into the buffer.
    Ansi(Ansi<Vec<u8>>),
    /// Apply coloring using the Windows console APIs. This buffer saves
    /// color information in memory and only interacts with the console when
    /// the buffer is printed.
    #[cfg(windows)]
    Windows(WindowsBuffer),
}

impl Buffer {
    /// Create a new buffer with the given color settings.
    #[cfg(not(windows))]
    fn new(choice: ColorChoice) -> Buffer {
        if choice.should_attempt_color() {
            Buffer::ansi()
        } else {
            Buffer::no_color()
        }
    }

    /// Create a new buffer with the given color settings.
    ///
    /// On Windows, one can elect to create a buffer capable of being written
    /// to a console. Only enable it if a console is available.
    ///
    /// If coloring is desired and `console` is false, then ANSI escape
    /// sequences are used instead.
    #[cfg(windows)]
    fn new(choice: ColorChoice, console: bool) -> Buffer {
        if choice.should_attempt_color() {
            if !console || choice.should_ansi() {
                Buffer::ansi()
            } else {
                Buffer::console()
            }
        } else {
            Buffer::no_color()
        }
    }

    /// Create a buffer that drops all color information.
    pub fn no_color() -> Buffer {
        Buffer(BufferInner::NoColor(NoColor(vec![])))
    }

    /// Create a buffer that uses ANSI escape sequences.
    pub fn ansi() -> Buffer {
        Buffer(BufferInner::Ansi(Ansi(vec![])))
    }

    /// Create a buffer that can be written to a Windows console.
    #[cfg(windows)]
    pub fn console() -> Buffer {
        Buffer(BufferInner::Windows(WindowsBuffer::new()))
    }

    /// Returns true if and only if this buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the length of this buffer in bytes.
    pub fn len(&self) -> usize {
        match self.0 {
            BufferInner::NoColor(ref b) => b.0.len(),
            BufferInner::Ansi(ref b) => b.0.len(),
            #[cfg(windows)]
            BufferInner::Windows(ref b) => b.buf.len(),
        }
    }

    /// Clears this buffer.
    pub fn clear(&mut self) {
        match self.0 {
            BufferInner::NoColor(ref mut b) => b.0.clear(),
            BufferInner::Ansi(ref mut b) => b.0.clear(),
            #[cfg(windows)]
            BufferInner::Windows(ref mut b) => b.clear(),
        }
    }

    /// Consume this buffer and return the underlying raw data.
    ///
    /// On Windows, this unrecoverably drops all color information associated
    /// with the buffer.
    pub fn into_inner(self) -> Vec<u8> {
        match self.0 {
            BufferInner::NoColor(b) => b.0,
            BufferInner::Ansi(b) => b.0,
            #[cfg(windows)]
            BufferInner::Windows(b) => b.buf,
        }
    }

    /// Return the underlying data of the buffer.
    pub fn as_slice(&self) -> &[u8] {
        match self.0 {
            BufferInner::NoColor(ref b) => &b.0,
            BufferInner::Ansi(ref b) => &b.0,
            #[cfg(windows)]
            BufferInner::Windows(ref b) => &b.buf,
        }
    }

    /// Return the underlying data of the buffer as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        match self.0 {
            BufferInner::NoColor(ref mut b) => &mut b.0,
            BufferInner::Ansi(ref mut b) => &mut b.0,
            #[cfg(windows)]
            BufferInner::Windows(ref mut b) => &mut b.buf,
        }
    }
}

impl io::Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0 {
            BufferInner::NoColor(ref mut w) => w.write(buf),
            BufferInner::Ansi(ref mut w) => w.write(buf),
            #[cfg(windows)]
            BufferInner::Windows(ref mut w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.0 {
            BufferInner::NoColor(ref mut w) => w.flush(),
            BufferInner::Ansi(ref mut w) => w.flush(),
            #[cfg(windows)]
            BufferInner::Windows(ref mut w) => w.flush(),
        }
    }
}

impl WriteColor for Buffer {
    fn supports_color(&self) -> bool {
        match self.0 {
            BufferInner::NoColor(_) => false,
            BufferInner::Ansi(_) => true,
            #[cfg(windows)]
            BufferInner::Windows(_) => true,
        }
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        match self.0 {
            BufferInner::NoColor(ref mut w) => w.set_color(spec),
            BufferInner::Ansi(ref mut w) => w.set_color(spec),
            #[cfg(windows)]
            BufferInner::Windows(ref mut w) => w.set_color(spec),
        }
    }

    fn reset(&mut self) -> io::Result<()> {
        match self.0 {
            BufferInner::NoColor(ref mut w) => w.reset(),
            BufferInner::Ansi(ref mut w) => w.reset(),
            #[cfg(windows)]
            BufferInner::Windows(ref mut w) => w.reset(),
        }
    }
}

/// Satisfies `WriteColor` but ignores all color options.
pub struct NoColor<W>(W);

impl<W: Write> NoColor<W> {
    /// Create a new writer that satisfies `WriteColor` but drops all color
    /// information.
    pub fn new(wtr: W) -> NoColor<W> { NoColor(wtr) }

    /// Consume this `NoColor` value and return the inner writer.
    pub fn into_inner(self) -> W { self.0 }

    /// Return a reference to the inner writer.
    pub fn get_ref(&self) -> &W { &self.0 }

    /// Return a mutable reference to the inner writer.
    pub fn get_mut(&mut self) -> &mut W { &mut self.0 }
}

impl<W: io::Write> io::Write for NoColor<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<W: io::Write> WriteColor for NoColor<W> {
    fn supports_color(&self) -> bool { false }
    fn set_color(&mut self, _: &ColorSpec) -> io::Result<()> { Ok(()) }
    fn reset(&mut self) -> io::Result<()> { Ok(()) }
}

/// Satisfies `WriteColor` using standard ANSI escape sequences.
pub struct Ansi<W>(W);

impl<W: Write> Ansi<W> {
    /// Create a new writer that satisfies `WriteColor` using standard ANSI
    /// escape sequences.
    pub fn new(wtr: W) -> Ansi<W> { Ansi(wtr) }

    /// Consume this `Ansi` value and return the inner writer.
    pub fn into_inner(self) -> W { self.0 }

    /// Return a reference to the inner writer.
    pub fn get_ref(&self) -> &W { &self.0 }

    /// Return a mutable reference to the inner writer.
    pub fn get_mut(&mut self) -> &mut W { &mut self.0 }
}

impl<W: io::Write> io::Write for Ansi<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<W: io::Write> WriteColor for Ansi<W> {
    fn supports_color(&self) -> bool { true }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        try!(self.reset());
        if let Some(ref c) = spec.fg_color {
            try!(self.write_color(true, c, spec.intense));
        }
        if let Some(ref c) = spec.bg_color {
            try!(self.write_color(false, c, spec.intense));
        }
        if spec.bold {
            try!(self.write_str("\x1B[1m"));
        }
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.write_str("\x1B[m")
    }
}

impl<W: io::Write> Ansi<W> {
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write_all(s.as_bytes())
    }

    fn write_color(
        &mut self,
        fg: bool,
        c: &Color,
        intense: bool,
    ) -> io::Result<()> {
        macro_rules! write_intense {
            ($clr:expr) => {
                if fg {
                    self.write_str(concat!("\x1B[38;5;", $clr, "m"))
                } else {
                    self.write_str(concat!("\x1B[48;5;", $clr, "m"))
                }
            }
        }
        macro_rules! write_normal {
            ($clr:expr) => {
                if fg {
                    self.write_str(concat!("\x1B[3", $clr, "m"))
                } else {
                    self.write_str(concat!("\x1B[4", $clr, "m"))
                }
            }
        }
        if intense {
            match *c {
                Color::Black => write_intense!("8"),
                Color::Blue => write_intense!("12"),
                Color::Green => write_intense!("10"),
                Color::Red => write_intense!("9"),
                Color::Cyan => write_intense!("14"),
                Color::Magenta => write_intense!("13"),
                Color::Yellow => write_intense!("11"),
                Color::White => write_intense!("15"),
                Color::__Nonexhaustive => unreachable!(),
            }
        } else {
            match *c {
                Color::Black => write_normal!("0"),
                Color::Blue => write_normal!("4"),
                Color::Green => write_normal!("2"),
                Color::Red => write_normal!("1"),
                Color::Cyan => write_normal!("6"),
                Color::Magenta => write_normal!("5"),
                Color::Yellow => write_normal!("3"),
                Color::White => write_normal!("7"),
                Color::__Nonexhaustive => unreachable!(),
            }
        }
    }
}

/// An in-memory buffer that provides Windows console coloring.
///
/// This doesn't actually communicate with the Windows console. Instead, it
/// acts like a normal buffer but also saves the color information associated
/// with positions in the buffer. It is only when the buffer is written to the
/// console that coloring is actually applied.
///
/// This is roughly isomorphic to the ANSI based approach (i.e.,
/// `Ansi<Vec<u8>>`), except with ANSI, the color information is embedded
/// directly into the buffer.
///
/// Note that there is no way to write something generic like
/// `WindowsConsole<W: io::Write>` since coloring on Windows is tied
/// specifically to the console APIs, and therefore can't work on arbitrary
/// writers.
#[cfg(windows)]
#[derive(Clone, Debug)]
struct WindowsBuffer {
    /// The actual content that should be printed.
    buf: Vec<u8>,
    /// A sequence of position oriented color specifications. Namely, each
    /// element is a position and a color spec, where the color spec should
    /// be applied at the position inside of `buf`.
    ///
    /// A missing color spec implies the underlying console should be reset.
    colors: Vec<(usize, Option<ColorSpec>)>,
}

#[cfg(windows)]
impl WindowsBuffer {
    /// Create a new empty buffer for Windows console coloring.
    fn new() -> WindowsBuffer {
        WindowsBuffer {
            buf: vec![],
            colors: vec![],
        }
    }

    /// Push the given color specification into this buffer.
    ///
    /// This has the effect of setting the given color information at the
    /// current position in the buffer.
    fn push(&mut self, spec: Option<ColorSpec>) {
        let pos = self.buf.len();
        self.colors.push((pos, spec));
    }

    /// Print the contents to the given stream handle, and use the console
    /// for coloring.
    fn print(
        &self,
        console: &mut wincolor::Console,
        stream: &mut LossyStandardStream<IoStandardStreamLock>,
    ) -> io::Result<()> {
        let mut last = 0;
        for &(pos, ref spec) in &self.colors {
            try!(stream.write_all(&self.buf[last..pos]));
            try!(stream.flush());
            last = pos;
            match *spec {
                None => try!(console.reset()),
                Some(ref spec) => try!(spec.write_console(console)),
            }
        }
        try!(stream.write_all(&self.buf[last..]));
        stream.flush()
    }

    /// Clear the buffer.
    fn clear(&mut self) {
        self.buf.clear();
        self.colors.clear();
    }
}

#[cfg(windows)]
impl io::Write for WindowsBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
impl WriteColor for WindowsBuffer {
    fn supports_color(&self) -> bool { true }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.push(Some(spec.clone()));
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.push(None);
        Ok(())
    }
}

/// A color specification.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ColorSpec {
    fg_color: Option<Color>,
    bg_color: Option<Color>,
    bold: bool,
    intense: bool,
}

impl ColorSpec {
    /// Create a new color specification that has no colors or styles.
    pub fn new() -> ColorSpec {
        ColorSpec::default()
    }

    /// Get the foreground color.
    pub fn fg(&self) -> Option<&Color> { self.fg_color.as_ref() }

    /// Set the foreground color.
    pub fn set_fg(&mut self, color: Option<Color>) -> &mut ColorSpec {
        self.fg_color = color;
        self
    }

    /// Get the background color.
    pub fn bg(&self) -> Option<&Color> { self.bg_color.as_ref() }

    /// Set the background color.
    pub fn set_bg(&mut self, color: Option<Color>) -> &mut ColorSpec {
        self.bg_color = color;
        self
    }

    /// Get whether this is bold or not.
    ///
    /// Note that the bold setting has no effect in a Windows console.
    pub fn bold(&self) -> bool { self.bold }

    /// Set whether the text is bolded or not.
    ///
    /// Note that the bold setting has no effect in a Windows console.
    pub fn set_bold(&mut self, yes: bool) -> &mut ColorSpec {
        self.bold = yes;
        self
    }

    /// Get whether this is intense or not.
    pub fn intense(&self) -> bool { self.intense }

    /// Set whether the text is intense or not.
    pub fn set_intense(&mut self, yes: bool) -> &mut ColorSpec {
        self.intense = yes;
        self
    }

    /// Returns true if this color specification has no colors or styles.
    pub fn is_none(&self) -> bool {
        self.fg_color.is_none() && self.bg_color.is_none() && !self.bold
    }

    /// Clears this color specification so that it has no color/style settings.
    pub fn clear(&mut self) {
        self.fg_color = None;
        self.bg_color = None;
        self.bold = false;
    }

    /// Writes this color spec to the given Windows console.
    #[cfg(windows)]
    fn write_console(
        &self,
        console: &mut wincolor::Console,
    ) -> io::Result<()> {
        use wincolor::Intense;

        let intense = if self.intense { Intense::Yes } else { Intense::No };
        if let Some(color) = self.fg_color.as_ref().map(|c| c.to_windows()) {
            try!(console.fg(intense, color));
        }
        if let Some(color) = self.bg_color.as_ref().map(|c| c.to_windows()) {
            try!(console.bg(intense, color));
        }
        Ok(())
    }
}

/// The set of available English colors for the terminal foreground/background.
///
/// Note that this set may expand over time.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Color {
    Black,
    Blue,
    Green,
    Red,
    Cyan,
    Magenta,
    Yellow,
    White,
    #[doc(hidden)]
    __Nonexhaustive,
}

#[cfg(windows)]
impl Color {
    /// Translate this color to a wincolor::Color.
    fn to_windows(&self) -> wincolor::Color {
        match *self {
            Color::Black => wincolor::Color::Black,
            Color::Blue => wincolor::Color::Blue,
            Color::Green => wincolor::Color::Green,
            Color::Red => wincolor::Color::Red,
            Color::Cyan => wincolor::Color::Cyan,
            Color::Magenta => wincolor::Color::Magenta,
            Color::Yellow => wincolor::Color::Yellow,
            Color::White => wincolor::Color::White,
            Color::__Nonexhaustive => unreachable!(),
        }
    }
}

/// An error from parsing an invalid color name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseColorError(String);

impl ParseColorError {
    /// Return the string that couldn't be parsed as a valid color.
    pub fn invalid(&self) -> &str { &self.0 }
}

impl error::Error for ParseColorError {
    fn description(&self) -> &str { "unrecognized color name" }
}

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unrecognized color name '{}'. Choose from: \
                black, blue, green, red, cyan, magenta, yellow, white.",
                self.0)
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Color, ParseColorError> {
        match &*s.to_lowercase() {
            "black" => Ok(Color::Black),
            "blue" => Ok(Color::Blue),
            "green" => Ok(Color::Green),
            "red" => Ok(Color::Red),
            "cyan" => Ok(Color::Cyan),
            "magenta" => Ok(Color::Magenta),
            "yellow" => Ok(Color::Yellow),
            "white" => Ok(Color::White),
            _ => Err(ParseColorError(s.to_string())),
        }
    }
}

struct LossyStandardStream<W> {
    wtr: W,
    #[cfg(windows)]
    is_console: bool,
}

impl<W: io::Write> LossyStandardStream<W> {
    #[cfg(not(windows))]
    fn new(wtr: W) -> LossyStandardStream<W> {
        LossyStandardStream { wtr: wtr }
    }

    #[cfg(windows)]
    fn new(wtr: W) -> LossyStandardStream<W> {
        LossyStandardStream { wtr: wtr, is_console: false }
    }

    #[cfg(not(windows))]
    fn wrap<Q: io::Write>(&self, wtr: Q) -> LossyStandardStream<Q> {
        LossyStandardStream::new(wtr)
    }

    #[cfg(windows)]
    fn wrap<Q: io::Write>(&self, wtr: Q) -> LossyStandardStream<Q> {
        LossyStandardStream::new(wtr).is_console(self.is_console)
    }

    #[cfg(windows)]
    fn is_console(mut self, yes: bool) -> LossyStandardStream<W> {
        self.is_console = yes;
        self
    }

    fn get_ref(&self) -> &W {
        &self.wtr
    }
}

impl<W: WriteColor> WriteColor for LossyStandardStream<W> {
    fn supports_color(&self) -> bool { self.wtr.supports_color() }
    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.wtr.set_color(spec)
    }
    fn reset(&mut self) -> io::Result<()> { self.wtr.reset() }
}

impl<W: io::Write> io::Write for LossyStandardStream<W> {
    #[cfg(not(windows))]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.wtr.write(buf)
    }

    #[cfg(windows)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.is_console {
            write_lossy_utf8(&mut self.wtr, buf)
        } else {
            self.wtr.write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.wtr.flush()
    }
}

#[cfg(windows)]
fn write_lossy_utf8<W: io::Write>(mut w: W, buf: &[u8]) -> io::Result<usize> {
    match ::std::str::from_utf8(buf) {
        Ok(s) => w.write(s.as_bytes()),
        Err(ref e) if e.valid_up_to() == 0 => {
            try!(w.write(b"\xEF\xBF\xBD"));
            Ok(1)
        }
        Err(e) => w.write(&buf[..e.valid_up_to()]),
    }
}

#[cfg(test)]
mod tests {
    use super::StandardStream;

    fn assert_is_send<T: Send>() {}

    #[test]
    fn standard_stream_is_send() {
        assert_is_send::<StandardStream>();
    }
}
