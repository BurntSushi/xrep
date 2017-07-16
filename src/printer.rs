use std::error;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use regex::bytes::{Captures, Regex, Replacer};
use termcolor::{Color, ColorSpec, ParseColorError, WriteColor};

use pathutil::strip_prefix;
use ignore::types::FileTypeDef;

/// CountingReplacer implements the Replacer interface for Regex,
/// and counts how often replacement is being performed.
struct CountingReplacer<'r> {
    replace: &'r [u8],
    count: &'r mut usize,
}

impl<'r> CountingReplacer<'r> {
    fn new(replace: &'r [u8], count: &'r mut usize) -> CountingReplacer<'r> {
        CountingReplacer { replace: replace, count: count }
    }
}

impl<'r> Replacer for CountingReplacer<'r> {
    fn replace_append(&mut self, caps: &Captures, dst: &mut Vec<u8>) {
        *self.count += 1;
        caps.expand(self.replace, dst);
    }
}

/// Printer encapsulates all output logic for searching.
///
/// Note that we currently ignore all write errors. It's probably worthwhile
/// to fix this, but printers are only ever used for writes to stdout or
/// writes to memory, neither of which commonly fail.
pub struct Printer<W> {
    /// The underlying writer.
    wtr: W,
    /// Whether anything has been printed to wtr yet.
    has_printed: bool,
    /// Whether to show column numbers for the first match or not.
    column: bool,
    /// The string to use to separate non-contiguous runs of context lines.
    context_separator: Vec<u8>,
    /// The end-of-line terminator used by the printer. In general, eols are
    /// printed via the match directly, but occasionally we need to insert them
    /// ourselves (for example, to print a context separator).
    eol: u8,
    /// A file separator to show before any matches are printed.
    file_separator: Option<Vec<u8>>,
    /// Whether to show file name as a heading or not.
    ///
    /// N.B. If with_filename is false, then this setting has no effect.
    heading: bool,
    /// Whether to show every match on its own line.
    line_per_match: bool,
    /// Whether to print NUL bytes after a file path instead of new lines
    /// or `:`.
    null: bool,
    /// Print only the matched (non-empty) parts of a matching line
    only_matching: bool,
    /// A string to use as a replacement of each match in a matching line.
    replace: Option<Vec<u8>>,
    /// Whether to prefix each match with the corresponding file name.
    with_filename: bool,
    /// The color specifications.
    colors: ColorSpecs,
    /// The separator to use for file paths. If empty, this is ignored.
    path_separator: Option<u8>,
    /// Restrict lines to this many columns.
    max_columns: Option<usize>
}

impl<W: WriteColor> Printer<W> {
    /// Create a new printer that writes to wtr with the given color settings.
    pub fn new(wtr: W) -> Printer<W> {
        Printer {
            wtr: wtr,
            has_printed: false,
            column: false,
            context_separator: "--".to_string().into_bytes(),
            eol: b'\n',
            file_separator: None,
            heading: false,
            line_per_match: false,
            null: false,
            only_matching: false,
            replace: None,
            with_filename: false,
            colors: ColorSpecs::default(),
            path_separator: None,
            max_columns: None,
        }
    }

    /// Set the color specifications.
    pub fn colors(mut self, colors: ColorSpecs) -> Printer<W> {
        self.colors = colors;
        self
    }

    /// When set, column numbers will be printed for the first match on each
    /// line.
    pub fn column(mut self, yes: bool) -> Printer<W> {
        self.column = yes;
        self
    }

    /// Set the context separator. The default is `--`.
    pub fn context_separator(mut self, sep: Vec<u8>) -> Printer<W> {
        self.context_separator = sep;
        self
    }

    /// Set the end-of-line terminator. The default is `\n`.
    pub fn eol(mut self, eol: u8) -> Printer<W> {
        self.eol = eol;
        self
    }

    /// If set, the separator is printed before any matches. By default, no
    /// separator is printed.
    pub fn file_separator(mut self, sep: Vec<u8>) -> Printer<W> {
        self.file_separator = Some(sep);
        self
    }

    /// Whether to show file name as a heading or not.
    ///
    /// N.B. If with_filename is false, then this setting has no effect.
    pub fn heading(mut self, yes: bool) -> Printer<W> {
        self.heading = yes;
        self
    }

    /// Whether to show every match on its own line.
    pub fn line_per_match(mut self, yes: bool) -> Printer<W> {
        self.line_per_match = yes;
        self
    }

    /// Whether to cause NUL bytes to follow file paths instead of other
    /// visual separators (like `:`, `-` and `\n`).
    pub fn null(mut self, yes: bool) -> Printer<W> {
        self.null = yes;
        self
    }

    /// Print only the matched (non-empty) parts of a matching line
    pub fn only_matching(mut self, yes: bool) -> Printer<W> {
        self.only_matching = yes;
        self
    }

    /// A separator to use when printing file paths. When empty, use the
    /// default separator for the current platform. (/ on Unix, \ on Windows.)
    pub fn path_separator(mut self, sep: Option<u8>) -> Printer<W> {
        self.path_separator = sep;
        self
    }

    /// Replace every match in each matching line with the replacement string
    /// given.
    pub fn replace(mut self, replacement: Vec<u8>) -> Printer<W> {
        self.replace = Some(replacement);
        self
    }

    /// When set, each match is prefixed with the file name that it came from.
    pub fn with_filename(mut self, yes: bool) -> Printer<W> {
        self.with_filename = yes;
        self
    }

    /// Configure the max. number of columns used for printing matching lines.
    pub fn max_columns(mut self, max_columns: Option<usize>) -> Printer<W> {
        self.max_columns = max_columns;
        self
    }

    /// Returns true if and only if something has been printed.
    pub fn has_printed(&self) -> bool {
        self.has_printed
    }

    /// Flushes the underlying writer and returns it.
    #[allow(dead_code)]
    pub fn into_inner(mut self) -> W {
        let _ = self.wtr.flush();
        self.wtr
    }

    /// Prints a type definition.
    pub fn type_def(&mut self, def: &FileTypeDef) {
        self.write(def.name().as_bytes());
        self.write(b": ");
        let mut first = true;
        for glob in def.globs() {
            if !first {
                self.write(b", ");
            }
            self.write(glob.as_bytes());
            first = false;
        }
        self.write_eol();
    }

    /// Prints the given path.
    pub fn path<P: AsRef<Path>>(&mut self, path: P) {
        let path = strip_prefix("./", path.as_ref()).unwrap_or(path.as_ref());
        self.write_path(path);
        self.write_path_eol();
    }

    /// Prints the given path and a count of the number of matches found.
    pub fn path_count<P: AsRef<Path>>(&mut self, path: P, count: u64) {
        if self.with_filename {
            self.write_path(path);
            self.write_path_sep(b':');
        }
        self.write(count.to_string().as_bytes());
        self.write_eol();
    }

    /// Prints the context separator.
    pub fn context_separate(&mut self) {
        if self.context_separator.is_empty() {
            return;
        }
        let _ = self.wtr.write_all(&self.context_separator);
        self.write_eol();
    }

    pub fn matched<P: AsRef<Path>>(
        &mut self,
        re: &Regex,
        path: P,
        buf: &[u8],
        start: usize,
        end: usize,
        line_number: Option<u64>,
    ) {
        if !self.line_per_match && !self.only_matching {
            let mat = re
                .find(&buf[start..end])
                .map(|m| (m.start(), m.end()))
                .unwrap_or((0, 0));
            return self.write_match(
                re, path, buf, start, end, line_number, mat.0, mat.1);
        }
        for m in re.find_iter(&buf[start..end]) {
            self.write_match(
                re, path.as_ref(), buf, start, end,
                line_number, m.start(), m.end());
        }
    }

    fn write_match<P: AsRef<Path>>(
        &mut self,
        re: &Regex,
        path: P,
        buf: &[u8],
        start: usize,
        end: usize,
        line_number: Option<u64>,
        match_start: usize,
        match_end: usize,
    ) {
        if self.heading && self.with_filename && !self.has_printed {
            self.write_file_sep();
            self.write_path(path);
            self.write_path_eol();
        } else if !self.heading && self.with_filename {
            self.write_path(path);
            self.write_path_sep(b':');
        }
        if let Some(line_number) = line_number {
            self.line_number(line_number, b':');
        }
        if self.column {
            self.column_number(match_start as u64 + 1, b':');
        }
        if self.replace.is_some() {
            let mut count = 0;
            let line = {
                let replacer = CountingReplacer::new(
                    self.replace.as_ref().unwrap(), &mut count);
                re.replace_all(&buf[start..end], replacer)
            };
            if self.max_columns.map_or(false, |m| line.len() > m) {
                let msg = format!(
                    "[Omitted long line with {} replacements]", count);
                self.write_colored(msg.as_bytes(), |colors| colors.matched());
                self.write_eol();
                return;
            }
            self.write(&line);
            if line.last() != Some(&self.eol) {
                self.write_eol();
            }
        } else {
            if self.only_matching {
                let buf = &buf[start + match_start..start + match_end];
                self.write_matched_line(re, buf, true);
            } else {
                self.write_matched_line(re, &buf[start..end], false);
            }
        }
    }

    fn write_matched_line(
        &mut self,
        re: &Regex,
        buf: &[u8],
        only_match: bool,
    ) {
        if self.max_columns.map_or(false, |m| buf.len() > m) {
            let count = re.find_iter(buf).count();
            let msg = format!("[Omitted long line with {} matches]", count);
            self.write_colored(msg.as_bytes(), |colors| colors.matched());
            self.write_eol();
            return;
        }
        if !self.wtr.supports_color() || self.colors.matched().is_none() {
            self.write(buf);
        } else if only_match {
            self.write_colored(buf, |colors| colors.matched());
        } else {
            let mut last_written = 0;
            for m in re.find_iter(buf) {
                self.write(&buf[last_written..m.start()]);
                self.write_colored(
                    &buf[m.start()..m.end()], |colors| colors.matched());
                last_written = m.end();
            }
            self.write(&buf[last_written..]);
        }
        if buf.last() != Some(&self.eol) {
            self.write_eol();
        }
    }

    pub fn context<P: AsRef<Path>>(
        &mut self,
        path: P,
        buf: &[u8],
        start: usize,
        end: usize,
        line_number: Option<u64>,
    ) {
        if self.heading && self.with_filename && !self.has_printed {
            self.write_file_sep();
            self.write_path(path);
            self.write_path_eol();
        } else if !self.heading && self.with_filename {
            self.write_path(path);
            self.write_path_sep(b'-');
        }
        if let Some(line_number) = line_number {
            self.line_number(line_number, b'-');
        }
        if self.max_columns.map_or(false, |m| end - start > m) {
            self.write(format!("[Omitted long context line]").as_bytes());
            self.write_eol();
            return;
        }
        self.write(&buf[start..end]);
        if buf[start..end].last() != Some(&self.eol) {
            self.write_eol();
        }
    }

    fn separator(&mut self, sep: &[u8]) {
        self.write(&sep);
    }

    fn write_path_sep(&mut self, sep: u8) {
        if self.null {
            self.write(b"\x00");
        } else {
            self.separator(&[sep]);
        }
    }

    fn write_path_eol(&mut self) {
        if self.null {
            self.write(b"\x00");
        } else {
            self.write_eol();
        }
    }

    #[cfg(unix)]
    fn write_path<P: AsRef<Path>>(&mut self, path: P) {
        use std::os::unix::ffi::OsStrExt;
        let path = path.as_ref().as_os_str().as_bytes();
        self.write_path_replace_separator(path);
    }

    #[cfg(not(unix))]
    fn write_path<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref().to_string_lossy();
        self.write_path_replace_separator(path.as_bytes());
    }

    fn write_path_replace_separator(&mut self, path: &[u8]) {
        match self.path_separator {
            None => self.write_colored(path, |colors| colors.path()),
            Some(sep) => {
                let transformed_path: Vec<_> = path.iter().map(|&b| {
                    if b == b'/' || (cfg!(windows) && b == b'\\') {
                        sep
                    } else {
                        b
                    }
                }).collect();
                self.write_colored(&transformed_path, |colors| colors.path());
            }
        }
    }

    fn line_number(&mut self, n: u64, sep: u8) {
        self.write_colored(n.to_string().as_bytes(), |colors| colors.line());
        self.separator(&[sep]);
    }

    fn column_number(&mut self, n: u64, sep: u8) {
        self.write_colored(n.to_string().as_bytes(), |colors| colors.column());
        self.separator(&[sep]);
    }

    fn write(&mut self, buf: &[u8]) {
        self.has_printed = true;
        let _ = self.wtr.write_all(buf);
    }

    fn write_eol(&mut self) {
        let eol = self.eol;
        self.write(&[eol]);
    }

    fn write_colored<F>(&mut self, buf: &[u8], get_color: F)
        where F: Fn(&ColorSpecs) -> &ColorSpec
    {
        let _ = self.wtr.set_color( get_color(&self.colors) );
        self.write(buf);
        let _ = self.wtr.reset();
    }

    fn write_file_sep(&mut self) {
        if let Some(ref sep) = self.file_separator {
            self.has_printed = true;
            let _ = self.wtr.write_all(sep);
            let _ = self.wtr.write_all(b"\n");
        }
    }
}

/// An error that can occur when parsing color specifications.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// This occurs when an unrecognized output type is used.
    UnrecognizedOutType(String),
    /// This occurs when an unrecognized spec type is used.
    UnrecognizedSpecType(String),
    /// This occurs when an unrecognized color name is used.
    UnrecognizedColor(String, String),
    /// This occurs when an unrecognized style attribute is used.
    UnrecognizedStyle(String),
    /// This occurs when the format of a color specification is invalid.
    InvalidFormat(String),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnrecognizedOutType(_) => "unrecognized output type",
            Error::UnrecognizedSpecType(_) => "unrecognized spec type",
            Error::UnrecognizedColor(_, _) => "unrecognized color name",
            Error::UnrecognizedStyle(_) => "unrecognized style attribute",
            Error::InvalidFormat(_) => "invalid color spec",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnrecognizedOutType(ref name) => {
                write!(f, "Unrecognized output type '{}'. Choose from: \
                           path, line, column, match.", name)
            }
            Error::UnrecognizedSpecType(ref name) => {
                write!(f, "Unrecognized spec type '{}'. Choose from: \
                           fg, bg, style, none.", name)
            }
            Error::UnrecognizedColor(_, ref msg) => {
                write!(f, "{}", msg)
            }
            Error::UnrecognizedStyle(ref name) => {
                write!(f, "Unrecognized style attribute '{}'. Choose from: \
                           nobold, bold, nointense, intense.", name)
            }
            Error::InvalidFormat(ref original) => {
                write!(
                    f,
                    "Invalid color speci format: '{}'. Valid format \
                     is '(path|line|column|match):(fg|bg|style):(value)'.",
                    original)
            }
        }
    }
}

impl From<ParseColorError> for Error {
    fn from(err: ParseColorError) -> Error {
        Error::UnrecognizedColor(err.invalid().to_string(), err.to_string())
    }
}

/// A merged set of color specifications.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ColorSpecs {
    path: ColorSpec,
    line: ColorSpec,
    column: ColorSpec,
    matched: ColorSpec,
}

/// A single color specification provided by the user.
///
/// A `ColorSpecs` can be built by merging a sequence of `Spec`s.
///
/// ## Example
///
/// The only way to build a `Spec` is to parse it from a string. Once multiple
/// `Spec`s have been constructed, then can be merged into a single
/// `ColorSpecs` value.
///
/// ```rust
/// use termcolor::{Color, ColorSpecs, Spec};
///
/// let spec1: Spec = "path:fg:blue".parse().unwrap();
/// let spec2: Spec = "match:bg:green".parse().unwrap();
/// let specs = ColorSpecs::new(&[spec1, spec2]);
///
/// assert_eq!(specs.path().fg(), Some(Color::Blue));
/// assert_eq!(specs.matched().bg(), Some(Color::Green));
/// ```
///
/// ## Format
///
/// The format of a `Spec` is a triple: `{type}:{attribute}:{value}`. Each
/// component is defined as follows:
///
/// * `{type}` can be one of `path`, `line`, `column` or `match`.
/// * `{attribute}` can be one of `fg`, `bg` or `style`. `{attribute}` may also
///   be the special value `none`, in which case, `{value}` can be omitted.
/// * `{value}` is either a color name (for `fg`/`bg`) or a style instruction.
///
/// `{type}` controls which part of the output should be styled and is
/// application dependent.
///
/// When `{attribute}` is `none`, then this should cause any existing color
/// settings to be cleared.
///
/// `{value}` should be a color when `{attribute}` is `fg` or `bg`, or it
/// should be a style instruction when `{attribute}` is `style`. When
/// `{attribute}` is `none`, `{value}` must be omitted.
///
/// Valid colors are `black`, `blue`, `green`, `red`, `cyan`, `magenta`,
/// `yellow`, `white`.
///
/// Valid style instructions are `nobold`, `bold`, `intense`, `nointense`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spec {
    ty: OutType,
    value: SpecValue,
}

/// The actual value given by the specification.
#[derive(Clone, Debug, Eq, PartialEq)]
enum SpecValue {
    None,
    Fg(Color),
    Bg(Color),
    Style(Style),
}

/// The set of configurable portions of ripgrep's output.
#[derive(Clone, Debug, Eq, PartialEq)]
enum OutType {
    Path,
    Line,
    Column,
    Match,
}

/// The specification type.
#[derive(Clone, Debug, Eq, PartialEq)]
enum SpecType {
    Fg,
    Bg,
    Style,
    None,
}

/// The set of available styles for use in the terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Style {
    Bold,
    NoBold,
    Intense,
    NoIntense,
}

impl ColorSpecs {
    /// Create color specifications from a list of user supplied
    /// specifications.
    pub fn new(user_specs: &[Spec]) -> ColorSpecs {
        let mut specs = ColorSpecs::default();
        for user_spec in user_specs {
            match user_spec.ty {
                OutType::Path => user_spec.merge_into(&mut specs.path),
                OutType::Line => user_spec.merge_into(&mut specs.line),
                OutType::Column => user_spec.merge_into(&mut specs.column),
                OutType::Match => user_spec.merge_into(&mut specs.matched),
            }
        }
        specs
    }

    /// Return the color specification for coloring file paths.
    fn path(&self) -> &ColorSpec {
        &self.path
    }

    /// Return the color specification for coloring line numbers.
    fn line(&self) -> &ColorSpec {
        &self.line
    }

    /// Return the color specification for coloring column numbers.
    fn column(&self) -> &ColorSpec {
        &self.column
    }

    /// Return the color specification for coloring matched text.
    fn matched(&self) -> &ColorSpec {
        &self.matched
    }
}

impl Spec {
    /// Merge this spec into the given color specification.
    fn merge_into(&self, cspec: &mut ColorSpec) {
        self.value.merge_into(cspec);
    }
}

impl SpecValue {
    /// Merge this spec value into the given color specification.
    fn merge_into(&self, cspec: &mut ColorSpec) {
        match *self {
            SpecValue::None => cspec.clear(),
            SpecValue::Fg(ref color) => { cspec.set_fg(Some(color.clone())); }
            SpecValue::Bg(ref color) => { cspec.set_bg(Some(color.clone())); }
            SpecValue::Style(ref style) => {
                match *style {
                    Style::Bold => { cspec.set_bold(true); }
                    Style::NoBold => { cspec.set_bold(false); }
                    Style::Intense => { cspec.set_intense(true); }
                    Style::NoIntense => { cspec.set_intense(false); }
                }
            }
        }
    }
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Spec, Error> {
        let pieces: Vec<&str> = s.split(':').collect();
        if pieces.len() <= 1 || pieces.len() > 3 {
            return Err(Error::InvalidFormat(s.to_string()));
        }
        let otype: OutType = try!(pieces[0].parse());
        match try!(pieces[1].parse()) {
            SpecType::None => Ok(Spec { ty: otype, value: SpecValue::None }),
            SpecType::Style => {
                if pieces.len() < 3 {
                    return Err(Error::InvalidFormat(s.to_string()));
                }
                let style: Style = try!(pieces[2].parse());
                Ok(Spec { ty: otype, value: SpecValue::Style(style) })
            }
            SpecType::Fg => {
                if pieces.len() < 3 {
                    return Err(Error::InvalidFormat(s.to_string()));
                }
                let color: Color = try!(pieces[2].parse());
                Ok(Spec { ty: otype, value: SpecValue::Fg(color) })
            }
            SpecType::Bg => {
                if pieces.len() < 3 {
                    return Err(Error::InvalidFormat(s.to_string()));
                }
                let color: Color = try!(pieces[2].parse());
                Ok(Spec { ty: otype, value: SpecValue::Bg(color) })
            }
        }
    }
}

impl FromStr for OutType {
    type Err = Error;

    fn from_str(s: &str) -> Result<OutType, Error> {
        match &*s.to_lowercase() {
            "path" => Ok(OutType::Path),
            "line" => Ok(OutType::Line),
            "column" => Ok(OutType::Column),
            "match" => Ok(OutType::Match),
            _ => Err(Error::UnrecognizedOutType(s.to_string())),
        }
    }
}

impl FromStr for SpecType {
    type Err = Error;

    fn from_str(s: &str) -> Result<SpecType, Error> {
        match &*s.to_lowercase() {
            "fg" => Ok(SpecType::Fg),
            "bg" => Ok(SpecType::Bg),
            "style" => Ok(SpecType::Style),
            "none" => Ok(SpecType::None),
            _ => Err(Error::UnrecognizedSpecType(s.to_string())),
        }
    }
}

impl FromStr for Style {
    type Err = Error;

    fn from_str(s: &str) -> Result<Style, Error> {
        match &*s.to_lowercase() {
            "bold" => Ok(Style::Bold),
            "nobold" => Ok(Style::NoBold),
            "intense" => Ok(Style::Intense),
            "nointense" => Ok(Style::NoIntense),
            _ => Err(Error::UnrecognizedStyle(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use termcolor::{Color, ColorSpec};
    use super::{ColorSpecs, Error, OutType, Spec, SpecValue, Style};

    #[test]
    fn merge() {
        let user_specs: &[Spec] = &[
            "match:fg:blue".parse().unwrap(),
            "match:none".parse().unwrap(),
            "match:style:bold".parse().unwrap(),
        ];
        let mut expect_matched = ColorSpec::new();
        expect_matched.set_bold(true);
        assert_eq!(ColorSpecs::new(user_specs), ColorSpecs {
            path: ColorSpec::default(),
            line: ColorSpec::default(),
            column: ColorSpec::default(),
            matched: expect_matched,
        });
    }

    #[test]
    fn specs() {
        let spec: Spec = "path:fg:blue".parse().unwrap();
        assert_eq!(spec, Spec {
            ty: OutType::Path,
            value: SpecValue::Fg(Color::Blue),
        });

        let spec: Spec = "path:bg:red".parse().unwrap();
        assert_eq!(spec, Spec {
            ty: OutType::Path,
            value: SpecValue::Bg(Color::Red),
        });

        let spec: Spec = "match:style:bold".parse().unwrap();
        assert_eq!(spec, Spec {
            ty: OutType::Match,
            value: SpecValue::Style(Style::Bold),
        });

        let spec: Spec = "match:style:intense".parse().unwrap();
        assert_eq!(spec, Spec {
            ty: OutType::Match,
            value: SpecValue::Style(Style::Intense),
        });

        let spec: Spec = "line:none".parse().unwrap();
        assert_eq!(spec, Spec {
            ty: OutType::Line,
            value: SpecValue::None,
        });

        let spec: Spec = "column:bg:green".parse().unwrap();
        assert_eq!(spec, Spec {
            ty: OutType::Column,
            value: SpecValue::Bg(Color::Green),
        });
    }

    #[test]
    fn spec_errors() {
        let err = "line:nonee".parse::<Spec>().unwrap_err();
        assert_eq!(err, Error::UnrecognizedSpecType("nonee".to_string()));

        let err = "".parse::<Spec>().unwrap_err();
        assert_eq!(err, Error::InvalidFormat("".to_string()));

        let err = "foo".parse::<Spec>().unwrap_err();
        assert_eq!(err, Error::InvalidFormat("foo".to_string()));

        let err = "line:style:italic".parse::<Spec>().unwrap_err();
        assert_eq!(err, Error::UnrecognizedStyle("italic".to_string()));

        let err = "line:fg:brown".parse::<Spec>().unwrap_err();
        match err {
            Error::UnrecognizedColor(name, _) => assert_eq!(name, "brown"),
            err => assert!(false, "unexpected error: {:?}", err),
        }

        let err = "foo:fg:brown".parse::<Spec>().unwrap_err();
        assert_eq!(err, Error::UnrecognizedOutType("foo".to_string()));
    }
}
