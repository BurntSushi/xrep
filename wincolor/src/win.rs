use std::io;
use std::mem;

use winapi::shared::minwindef::{DWORD, WORD};
use winapi::um::consoleapi;
use winapi::um::processenv;
use winapi::um::winbase::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
use winapi::um::wincon::{
    self,
    FOREGROUND_BLUE as FG_BLUE,
    FOREGROUND_GREEN as FG_GREEN,
    FOREGROUND_RED as FG_RED,
    FOREGROUND_INTENSITY as FG_INTENSITY,
};

const FG_CYAN: WORD = FG_BLUE | FG_GREEN;
const FG_MAGENTA: WORD = FG_BLUE | FG_RED;
const FG_YELLOW: WORD = FG_GREEN | FG_RED;
const FG_WHITE: WORD = FG_BLUE | FG_GREEN | FG_RED;

/// A Windows console.
///
/// This represents a very limited set of functionality available to a Windows
/// console. In particular, it can only change text attributes such as color
/// and intensity.
///
/// There is no way to "write" to this console. Simply write to
/// stdout or stderr instead, while interleaving instructions to the console
/// to change text attributes.
///
/// A common pitfall when using a console is to forget to flush writes to
/// stdout before setting new text attributes.
#[derive(Debug)]
pub struct Console {
    handle_id: DWORD,
    start_attr: TextAttributes,
    cur_attr: TextAttributes,
}

impl Console {
    /// Get a console for a standard I/O stream.
    fn create_for_stream(handle_id: DWORD) -> io::Result<Console> {
        let mut info = unsafe { mem::zeroed() };
        let res = unsafe {
            let handle = processenv::GetStdHandle(handle_id);
            wincon::GetConsoleScreenBufferInfo(handle, &mut info)
        };
        if res == 0 {
            return Err(io::Error::last_os_error());
        }
        let attr = TextAttributes::from_word(info.wAttributes);
        Ok(Console {
            handle_id: handle_id,
            start_attr: attr,
            cur_attr: attr,
        })
    }

    /// Create a new Console to stdout.
    ///
    /// If there was a problem creating the console, then an error is returned.
    pub fn stdout() -> io::Result<Console> {
        Self::create_for_stream(STD_OUTPUT_HANDLE)
    }

    /// Create a new Console to stderr.
    ///
    /// If there was a problem creating the console, then an error is returned.
    pub fn stderr() -> io::Result<Console> {
        Self::create_for_stream(STD_ERROR_HANDLE)
    }

    /// Applies the current text attributes.
    fn set(&mut self) -> io::Result<()> {
        let attr = self.cur_attr.to_word();
        let res = unsafe {
            let handle = processenv::GetStdHandle(self.handle_id);
            wincon::SetConsoleTextAttribute(handle, attr)
        };
        if res == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Apply the given intensity and color attributes to the console
    /// foreground.
    ///
    /// If there was a problem setting attributes on the console, then an error
    /// is returned.
    pub fn fg(&mut self, intense: Intense, color: Color) -> io::Result<()> {
        self.cur_attr.fg_color = color;
        self.cur_attr.fg_intense = intense;
        self.set()
    }

    /// Apply the given intensity and color attributes to the console
    /// background.
    ///
    /// If there was a problem setting attributes on the console, then an error
    /// is returned.
    pub fn bg(&mut self, intense: Intense, color: Color) -> io::Result<()> {
        self.cur_attr.bg_color = color;
        self.cur_attr.bg_intense = intense;
        self.set()
    }

    /// Reset the console text attributes to their original settings.
    ///
    /// The original settings correspond to the text attributes on the console
    /// when this `Console` value was created.
    ///
    /// If there was a problem setting attributes on the console, then an error
    /// is returned.
    pub fn reset(&mut self) -> io::Result<()> {
        self.cur_attr = self.start_attr;
        self.set()
    }

    /// Toggle virtual terminal processing.
    ///
    /// This method attempts to toggle virtual terminal processing for this
    /// console. If there was a problem toggling it, then an error returned.
    /// On success, the caller may assume that toggling it was successful.
    ///
    /// When virtual terminal processing is enabled, characters emitted to the
    /// console are parsed for VT100 and similar control character sequences
    /// that control color and other similar operations.
    pub fn set_virtual_terminal_processing(
        &mut self,
        yes: bool,
    ) -> io::Result<()> {
        let vt = wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;

        let mut old_mode = 0;
        let handle = unsafe { processenv::GetStdHandle(self.handle_id) };
        if unsafe { consoleapi::GetConsoleMode(handle, &mut old_mode) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let new_mode =
            if yes {
                old_mode | vt
            } else {
                old_mode & !vt
            };
        if old_mode == new_mode {
            return Ok(());
        }
        if unsafe { consoleapi::SetConsoleMode(handle, new_mode) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// A representation of text attributes for the Windows console.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct TextAttributes {
    fg_color: Color,
    fg_intense: Intense,
    bg_color: Color,
    bg_intense: Intense,
}

impl TextAttributes {
    fn to_word(&self) -> WORD {
        let mut w = 0;
        w |= self.fg_color.to_fg();
        w |= self.fg_intense.to_fg();
        w |= self.bg_color.to_bg();
        w |= self.bg_intense.to_bg();
        w
    }

    fn from_word(word: WORD) -> TextAttributes {
        TextAttributes {
            fg_color: Color::from_fg(word),
            fg_intense: Intense::from_fg(word),
            bg_color: Color::from_bg(word),
            bg_intense: Intense::from_bg(word),
        }
    }
}

/// Whether to use intense colors or not.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intense {
    Yes,
    No,
}

impl Intense {
    fn to_bg(&self) -> WORD {
        self.to_fg() << 4
    }

    fn from_bg(word: WORD) -> Intense {
        Intense::from_fg(word >> 4)
    }

    fn to_fg(&self) -> WORD {
        match *self {
            Intense::No => 0,
            Intense::Yes => FG_INTENSITY,
        }
    }

    fn from_fg(word: WORD) -> Intense {
        if word & FG_INTENSITY > 0 {
            Intense::Yes
        } else {
            Intense::No
        }
    }
}

/// The set of available colors for use with a Windows console.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Color {
    Black,
    Blue,
    Green,
    Red,
    Cyan,
    Magenta,
    Yellow,
    White,
}

impl Color {
    fn to_bg(&self) -> WORD {
        self.to_fg() << 4
    }

    fn from_bg(word: WORD) -> Color {
        Color::from_fg(word >> 4)
    }

    fn to_fg(&self) -> WORD {
        match *self {
            Color::Black => 0,
            Color::Blue => FG_BLUE,
            Color::Green => FG_GREEN,
            Color::Red => FG_RED,
            Color::Cyan => FG_CYAN,
            Color::Magenta => FG_MAGENTA,
            Color::Yellow => FG_YELLOW,
            Color::White => FG_WHITE,
        }
    }

    fn from_fg(word: WORD) -> Color {
        match word & 0b111 {
            FG_BLUE => Color::Blue,
            FG_GREEN => Color::Green,
            FG_RED => Color::Red,
            FG_CYAN => Color::Cyan,
            FG_MAGENTA => Color::Magenta,
            FG_YELLOW => Color::Yellow,
            FG_WHITE => Color::White,
            _ => Color::Black,
        }
    }
}
