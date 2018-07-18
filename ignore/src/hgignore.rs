/*!
doc
*/
use std::cell::RefCell;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, Read};
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::Arc;

use globset::{Candidate, GlobBuilder, GlobSet, GlobSetBuilder};
use regex::bytes::{Regex, RegexSet};
use thread_local::ThreadLocal;

use pathutil::{is_file_name, strip_prefix};
use {Error, Match, PartialErrorBuilder};

/// Pattern represents a rule from hgignore file (regex or glob).
#[derive(Clone, Debug)]
pub enum Pattern {
    /// Glob represents a single glob in a hgignore file.
    Glob {
        /// The file path that this glob was extracted from.
        from: Option<PathBuf>,
        /// The original glob string.
        original: String,
        /// The actual glob string used to convert to a regex.
        actual: String,
        /// Whether this glob should only match directories or not.
        is_only_dir: bool,
    },
    /// Regex represents a single regex in a hgignore file.
    Regex {
        /// The file path that this glob was extracted from.
        from: Option<PathBuf>,
        /// The regex string from hginore file.
        regex: String,
    },
}

impl Pattern {
    /// Returns the file path that defined this glob.
    pub fn from(&self) -> Option<&Path> {
        match self {
            &Pattern::Glob { ref from, .. } => from.as_ref().map(|p| &**p),
            &Pattern::Regex { ref from, .. } => from.as_ref().map(|p| &**p),
        }
    }

    /// The original pattern as it was defined in a hgignore file.
    pub fn original(&self) -> &str {
        match self {
            &Pattern::Glob { ref original, .. } => &original,
            &Pattern::Regex { ref regex, .. } => &regex,
        }
    }

    /// The actual pattern that was compiled to respect hgignore
    /// semantics.
    pub fn actual(&self) -> &str {
        match self {
            &Pattern::Glob { ref actual, .. } => &actual,
            &Pattern::Regex { ref regex, .. } => &regex,
        }
    }

    /// Whether this pattern must match a directory or not.
    pub fn is_only_dir(&self) -> bool {
        match self {
            &Pattern::Glob { is_only_dir, .. } => is_only_dir,
            _ => false,
        }
    }
}

/// doc
#[derive(Clone, Debug)]
pub struct Hgignore {
    set: GlobSet,
    regex_set: RegexSet,
    root: PathBuf,
    globs: Vec<Pattern>,
    regexes: Vec<Pattern>,
    num_ignores: u64,
    matches: Arc<ThreadLocal<RefCell<Vec<usize>>>>,
}

impl Hgignore {
    /// doc
    pub fn new<P: AsRef<Path>>(hgignore_path: P) -> (Hgignore, Option<Error>) {
        let path = hgignore_path.as_ref();
        let parent = path.parent().unwrap_or(Path::new("/"));
        let mut builder = HgignoreBuilder::new(parent);
        let mut errs = PartialErrorBuilder::default();
        errs.maybe_push_ignore_io(builder.add(path));
        match builder.build() {
            Ok(hgi) => (hgi, errs.into_error_option()),
            Err(err) => {
                errs.push(err);
                (Hgignore::empty(), errs.into_error_option())
            }
        }
    }

    /// doc
    pub fn global() -> (Hgignore, Option<Error>) {
        match hgrc_ignore_path() {
            None => (Hgignore::empty(), None),
            Some(path) => {
                if !path.is_file() {
                    (Hgignore::empty(), None)
                } else {
                    Hgignore::new(path)
                }
            }
        }
    }

    /// doc
    pub fn empty() -> Hgignore {
        HgignoreBuilder::new("").build().unwrap()
    }

    /// Returns true if and only if this hgignore has zero patterns, and
    /// therefore never matches any file path.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.regex_set.len() == 0
    }

    /// Returns whether the given file path matched a pattern in this hgignore
    /// matcher.
    ///
    /// `is_dir` should be true if the path refers to a directory and false
    /// otherwise.
    ///
    /// The given path is matched relative to the path given when building
    /// the matcher. Specifically, before matching `path`, its prefix (as
    /// determined by a common suffix of the directory containing this
    /// hgignore) is stripped. If there is no common suffix/prefix overlap,
    /// then `path` is assumed to be relative to this matcher.
    pub fn matched<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> Match<&Pattern> {
        if self.is_empty() {
            return Match::None;
        }
        self.matched_stripped(self.strip(path.as_ref()), is_dir)
    }

    /// Like matched, but takes a path that has already been stripped.
    fn matched_stripped<P: AsRef<Path>>(&self, path: P, is_dir: bool) -> Match<&Pattern> {
        if !self.set.is_empty() {
            let path = path.as_ref();
            let candidate = Candidate::new(path);
            let _matches = self.matches.get_default();
            let mut matches = _matches.borrow_mut();
            self.set.matches_candidate_into(&candidate, &mut *matches);
            for &i in matches.iter().rev() {
                let glob = &self.globs[i];
                if !glob.is_only_dir() || is_dir {
                    return Match::Ignore(glob);
                }
            }
        }
        if self.regex_set.len() != 0 {
            let path = path.as_ref();
            let candidate = Candidate::new(path);
            let re_matches = self.regex_set.matches(&*candidate.path);
            match re_matches.iter().next() {
                Some(i) => return Match::Ignore(&self.regexes[i]),
                None => return Match::None,
            };
        }
        Match::None
    }

    /// Strips the given path such that it's suitable for matching with this
    /// hgignore matcher.
    fn strip<'a, P: 'a + AsRef<Path> + ?Sized>(&'a self, path: &'a P) -> &'a Path {
        let mut path = path.as_ref();
        // A leading ./ is completely superfluous. We also strip it from
        // our hgignore root path, so we need to strip it from our candidate
        // path too.
        if let Some(p) = strip_prefix("./", path) {
            path = p;
        }
        // Strip any common prefix between the candidate path and the root
        // of the hgignore, to make sure we get relative matching right.
        // BUT, a file name might not have any directory components to it,
        // in which case, we don't want to accidentally strip any part of the
        // file name.
        if !is_file_name(path) {
            if let Some(p) = strip_prefix(&self.root, path) {
                path = p;
                // If we're left with a leading slash, get rid of it.
                if let Some(p) = strip_prefix("/", path) {
                    path = p;
                }
            }
        }
        path
    }
}

/// doc
#[derive(Copy, Clone, Debug)]
pub enum HgignoreSyntax {
    /// doc
    Glob,
    /// doc
    Regexp,
}

/// doc
pub struct HgignoreBuilder {
    builder: GlobSetBuilder,
    root: PathBuf,
    regex_lines: Vec<Pattern>,
    globs: Vec<Pattern>,
}

impl HgignoreBuilder {
    /// doc
    pub fn new<P: AsRef<Path>>(root: P) -> HgignoreBuilder {
        let root = root.as_ref();
        HgignoreBuilder {
            builder: GlobSetBuilder::new(),
            root: strip_prefix("./", root).unwrap_or(root).to_path_buf(),
            regex_lines: vec![],
            globs: vec![],
        }
    }

    /// doc
    pub fn build(&self) -> Result<Hgignore, Error> {
        let set = try!(self.builder.build().map_err(|err| Error::Glob {
            glob: None,
            err: err.to_string(),
        }));
        let rls: Vec<&str> = self.regex_lines.iter().map(|ref x| x.original()).collect();
        let nignore = self.globs.iter().count() + rls.iter().count();
        return Ok(Hgignore {
            set: set,
            root: self.root.clone(),
            globs: self.globs.clone(),
            regex_set: RegexSet::new(rls).unwrap(),
            regexes: self.regex_lines.clone(),
            num_ignores: nignore as u64,
            matches: Arc::new(ThreadLocal::default()),
        });
    }

    /// doc
    pub fn add<P: AsRef<Path>>(&mut self, path: P) -> Option<Error> {
        lazy_static! {
            static ref SYNTAX_SWITCH: Regex = Regex::new(r"^syntax:\s*(regexp|glob)\s*$").unwrap();
        };
        let mut current_syntax = HgignoreSyntax::Regexp;
        let path = path.as_ref();
        let file = match File::open(path) {
            Err(err) => return Some(Error::Io(err).with_path(path)),
            Ok(file) => file,
        };
        let rdr = io::BufReader::new(file);
        let mut errs = PartialErrorBuilder::default();
        for (i, line) in rdr.lines().enumerate() {
            let lineno = (i + 1) as u64;
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    errs.push(Error::Io(err).tagged(path, lineno));
                    break;
                }
            };
            match SYNTAX_SWITCH.captures(line.as_bytes()) {
                None => {
                    if let Err(err) = self.add_line(Some(path.to_path_buf()), &line, current_syntax)
                    {
                        errs.push(err.tagged(path, lineno));
                    };
                }
                Some(caps) => {
                    current_syntax = match &caps[1] {
                        b"glob" => HgignoreSyntax::Glob,
                        _ => HgignoreSyntax::Regexp,
                    }
                }
            };
        }
        errs.into_error_option()
    }

    /// Add each glob line from the string given.
    ///
    /// If this string came from a particular `hgignore` file, then its path
    /// should be provided here.
    ///
    /// The string given should be formatted as a `hgignore` file
    /// without syntax switch in it.
    #[cfg(test)]
    fn add_str(
        &mut self,
        from: Option<PathBuf>,
        hgignore: &str,
        current_syntax: HgignoreSyntax,
    ) -> Result<&mut HgignoreBuilder, Error> {
        for line in hgignore.lines() {
            self.add_line(from.clone(), line, current_syntax)?;
        }
        Ok(self)
    }

    /// doc
    pub fn add_line(
        &mut self,
        from: Option<PathBuf>,
        mut line: &str,
        current_syntax: HgignoreSyntax,
    ) -> Result<&mut HgignoreBuilder, Error> {
        if line.starts_with("#") {
            return Ok(self);
        }
        line = trim_comment(line).trim_right();
        if line.is_empty() {
            return Ok(self);
        }
        // TODO: extended patterns support, see hg help patterns
        lazy_static! {
            static ref EXT_PATTERN: Regex = Regex::new(r"^(\w+):(.*)$").unwrap();
        };
        match EXT_PATTERN.captures(line.as_bytes()) {
            Some(caps) => {
                match &caps[1] {
                    b"path" => debug!("HG pattern \"path:\" is not supported."),
                    b"rootfilesin" => debug!("HG pattern \"rootfilesin:\" is not supported."),
                    b"glob" => return self.add_glob_line(from, str::from_utf8(&caps[2]).unwrap()),
                    b"re" => return self.add_regex_line(from, str::from_utf8(&caps[2]).unwrap()),
                    b"listfile" => debug!("HG pattern \"listfile:\" is not supported."),
                    b"listfile0" => debug!("HG pattern \"listfile0:\" is not supported."),
                    b"include" => debug!("HG pattern \"include:\" is not supported."),
                    b"subinclude" => debug!("HG pattern \"subinclude:\" is not supported."),
                    _ => {
                        return match current_syntax {
                            HgignoreSyntax::Glob => self.add_glob_line(from, line),
                            HgignoreSyntax::Regexp => self.add_regex_line(from, line),
                        }
                    }
                }
                return Ok(self);
            }
            None => match current_syntax {
                HgignoreSyntax::Glob => self.add_glob_line(from, line),
                HgignoreSyntax::Regexp => self.add_regex_line(from, line),
            },
        }
    }

    /// doc
    pub fn add_regex_line(
        &mut self,
        from: Option<PathBuf>,
        line: &str,
    ) -> Result<&mut HgignoreBuilder, Error> {
        // FIXME: validate regex otherwise RegexSet build will
        // fail. So we have to create regexes twice:
        // during validation and during RegexSet build.
        // Another option is to use list of regexes instead of RegexSet
        match Regex::new(line) {
            Ok(_) => {
                self.regex_lines.push(Pattern::Regex {
                    from: from,
                    regex: line.to_string(),
                });
            }
            Err(_) => {
                debug!("{} is not valid rust regex.", line);
            }
        };
        Ok(self)
    }

    /// doc
    pub fn add_glob_line(
        &mut self,
        from: Option<PathBuf>,
        mut line: &str,
    ) -> Result<&mut HgignoreBuilder, Error> {
        let mut actual;
        let mut is_only_dir = false;
        let original = line.to_string();

        let mut literal_separator = false;
        let has_slash = line.chars().any(|c| c == '/');
        if line.starts_with("\\#") {
            line = &line[1..];
        }
        // If it ends with a slash, then this should only match directories,
        // but the slash should otherwise not be used while globbing.
        if let Some((i, c)) = line.char_indices().rev().nth(0) {
            if c == '/' {
                is_only_dir = true;
                line = &line[..i];
            }
        }
        // If there is a literal slash, then we note that so that globbing
        // doesn't let wildcards match slashes.
        actual = line.to_string();
        if has_slash {
            literal_separator = true;
        }
        // If there was a leading slash, then this is a glob that must
        // match the entire path name. Otherwise, we should let it match
        // anywhere, so use a **/ prefix.
        if !(actual.starts_with("**/") || (actual == "**" && is_only_dir)) {
            actual = format!("**/{}", actual);
        }
        // If the glob ends with `/**`, then we should only match everything
        // inside a directory, but not the directory itself. Standard globs
        // will match the directory. So we add `/*` to force the issue.
        if actual.ends_with("/**") {
            actual = format!("{}/*", actual);
        }
        let parsed = try!(
            GlobBuilder::new(&actual)
                .literal_separator(literal_separator)
                .build()
                .map_err(|err| Error::Glob {
                    glob: Some(original.clone()),
                    err: err.kind().to_string(),
                })
        );
        self.builder.add(parsed);
        self.globs.push(Pattern::Glob {
            from: from,
            original: original,
            actual: actual,
            is_only_dir: is_only_dir,
        });
        Ok(self)
    }
}

fn hgrc_ignore_path() -> Option<PathBuf> {
    hgrc_contents().and_then(|data| parse_ignore_file(&data))
}

/// Provides paths to per-user mercurial configs
/// TODO: return $HOME/Mercurial.ini only on windows
fn per_user_configs() -> Vec<PathBuf> {
    let configs = [".hgrc", "Mercurial.ini"];
    let mut result = vec![];
    if let Some(home) = env::home_dir() {
        for config in configs.iter() {
            let mut chome = home.clone();
            chome.push(config);
            result.push(chome);
        }
    }
    result
}

fn hgrc_contents() -> Option<Vec<u8>> {
    per_user_configs()
        .iter()
        .flat_map(|c| File::open(c).ok())
        .map(|f| io::BufReader::new(f))
        .flat_map(|mut br| {
            let mut contents = vec![];
            br.read_to_end(&mut contents).ok().map(|_| contents)
        })
        .next()
}

// TODO: parse only [ui] section
fn parse_ignore_file(data: &[u8]) -> Option<PathBuf> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(?ium)^\s*ignore\s*=\s*(.+)\s*$").unwrap();
    };
    let caps = match RE.captures(data) {
        None => return None,
        Some(caps) => caps,
    };
    str::from_utf8(&caps[1])
        .ok()
        .map(|s| PathBuf::from(expand_tilde(s)))
}

/// Expands `~/` in file paths to the value of $HOME.
fn expand_tilde(path: &str) -> String {
    // TODO: we don't handle `~username` shortcuts
    let expanded_path = if path.starts_with("~/") {
        env::home_dir().map(|home| path.replacen("~", home.to_str().unwrap(), 1))
    } else {
        None
    };
    expanded_path.unwrap_or(path.to_string())
}

/// Remove part of line without cars after leftmost #.
/// Mercurial consider sequence \# as escaped #.
fn trim_comment(line: &str) -> &str {
    let mut prev_char: char = ' ';
    return line.split(|c| {
        if c == '#' && prev_char != '\\' {
            return true;
        }
        prev_char = c;
        return false;
    }).nth(0).unwrap();
}

#[cfg(test)]
mod tests {
    use super::{Hgignore, HgignoreBuilder, HgignoreSyntax};
    use std::path::Path;

    fn hgi_from_str<P: AsRef<Path>>(root: P, s: &str, current_syntax: HgignoreSyntax) -> Hgignore {
        let mut builder = HgignoreBuilder::new(root);
        builder.add_str(None, s, current_syntax).unwrap();
        builder.build().unwrap()
    }

    macro_rules! ignored_glob {
        ($name:ident, $root:expr, $hgi:expr, $path:expr) => {
            ignored_glob!($name, $root, $hgi, $path, false);
        };
        ($name:ident, $root:expr, $hgi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let hgi = hgi_from_str($root, $hgi, HgignoreSyntax::Glob);
                assert!(hgi.matched($path, $is_dir).is_ignore());
            }
        };
    }

    macro_rules! ignored_re {
        ($name:ident, $root:expr, $hgi:expr, $path:expr) => {
            ignored_re!($name, $root, $hgi, $path, false);
        };
        ($name:ident, $root:expr, $hgi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let hgi = hgi_from_str($root, $hgi, HgignoreSyntax::Regexp);
                assert!(hgi.matched($path, $is_dir).is_ignore());
            }
        };
    }

    macro_rules! not_ignored_glob {
        ($name:ident, $root:expr, $hgi:expr, $path:expr) => {
            not_ignored_glob!($name, $root, $hgi, $path, false);
        };
        ($name:ident, $root:expr, $hgi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let hgi = hgi_from_str($root, $hgi, HgignoreSyntax::Glob);
                assert!(!hgi.matched($path, $is_dir).is_ignore());
            }
        };
    }

    macro_rules! not_ignored_re {
        ($name:ident, $root:expr, $hgi:expr, $path:expr) => {
            not_ignored_re!($name, $root, $hgi, $path, false);
        };
        ($name:ident, $root:expr, $hgi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let hgi = hgi_from_str($root, $hgi, HgignoreSyntax::Regexp);
                assert!(!hgi.matched($path, $is_dir).is_ignore());
            }
        };
    }

    const ROOT: &'static str = "/home/foobar/rust/rg";

    ignored_glob!(ig1, ROOT, "months", "months");
    ignored_glob!(ig2, ROOT, "*.lock", "Cargo.lock");
    ignored_glob!(ig3, ROOT, "*.rs", "src/main.rs");
    ignored_glob!(ig4, ROOT, "src/*.rs", "src/main.rs");
    ignored_glob!(ig5, ROOT, "foo/", "foo", true);
    ignored_glob!(ig6, ROOT, "**/foo", "foo");
    ignored_glob!(ig7, ROOT, "**/foo", "src/foo");
    ignored_glob!(ig8, ROOT, "**/foo/**", "src/foo/bar");
    ignored_glob!(ig9, ROOT, "**/foo/**", "wat/src/foo/bar/baz");
    ignored_glob!(ig10, ROOT, "**/foo/bar", "foo/bar");
    ignored_glob!(ig11, ROOT, "**/foo/bar", "src/foo/bar");
    ignored_glob!(ig12, ROOT, "abc/**", "abc/x");
    ignored_glob!(ig13, ROOT, "abc/**", "abc/x/y");
    ignored_glob!(ig14, ROOT, "abc/**", "abc/x/y/z");
    ignored_glob!(ig15, ROOT, "a/**/b", "a/b");
    ignored_glob!(ig16, ROOT, "a/**/b", "a/x/b");
    ignored_glob!(ig17, ROOT, "a/**/b", "a/x/y/b");
    ignored_glob!(ig18, ROOT, "!xy", "!xy");
    ignored_glob!(ig19, ROOT, r"\#foo", "#foo");
    ignored_glob!(ig20, ROOT, "foo", "./foo");
    ignored_glob!(ig21, ROOT, "target", "grep/target");
    ignored_glob!(ig22, ROOT, "Cargo.lock", "./tabwriter-bin/Cargo.lock");
    ignored_glob!(ig23, ROOT, "foo/", "xyz/foo", true);
    ignored_glob!(ig24, ROOT, "node_modules/ ", "node_modules", true);
    ignored_glob!(ig25, ROOT, "**/", "foo/bar", true);
    ignored_glob!(ig26, ROOT, "path1/*", "path1/foo");
    ignored_glob!(ig27, ROOT, "foo  # comment", "foo");
    ignored_glob!(ig28, ROOT, r"foo  \#", "foo  #");
    ignored_glob!(ig29, ROOT, r"foo  \#\##comment # and comment", "foo  ##");
    ignored_glob!(ig30, ROOT, "path1/*", "path2/path1/foo");
    ignored_glob!(ig31, ROOT, "src/*.rs", "src/grep/src/main.rs");

    not_ignored_glob!(ignot1, ROOT, "amonths", "months");
    not_ignored_glob!(ignot2, ROOT, "monthsa", "months");
    not_ignored_glob!(ignot7, ROOT, "foo/", "foo", false);
    not_ignored_glob!(ignot8, ROOT, "**/foo/**", "wat/src/afoo/bar/baz");
    not_ignored_glob!(ignot9, ROOT, "**/foo/**", "wat/src/fooa/bar/baz");
    not_ignored_glob!(ignot10, ROOT, "**/foo/bar", "foo/src/bar");
    not_ignored_glob!(ignot11, ROOT, "#foo", "#foo");
    not_ignored_glob!(ignot12, ROOT, "\n\n\n", "foo");
    not_ignored_glob!(ignot13, ROOT, "foo/**", "foo", true);
    not_ignored_glob!(
        ignot14,
        "./third_party/protobuf",
        "m4/ltoptions.m4",
        "./third_party/protobuf/csharp/src/packages/repositories.config"
    );

    ignored_re!(ir1, ROOT, r"^.*\.c", "cat-file.c");
    ignored_re!(ir2, ROOT, r"^src/.*\.rs", "src/main.rs");
    ignored_re!(ir3, ROOT, "^foo/bar/baz", "./foo/bar/baz");
    ignored_re!(ir4, "./src", "^llvm/", "./src/llvm/", true);
    ignored_re!(ir5, ROOT, "months", "months");
    ignored_re!(ir6, ROOT, r".*\.lock$", "Cargo.lock");
    ignored_re!(ir7, ROOT, r".*\.rs$", "src/main.rs");
    ignored_re!(ir8, ROOT, r"^src/.*\.rs$", "src/main.rs");
    ignored_re!(ir9, ROOT, ".a/b", ".a/b");
    ignored_re!(ir10, "./", ".a/b", ".a/b");
    // ignored_re!(ir11, ".", ".a/b", ".a/b");
    // ignored_re!(ir12, "./.", ".a/b", ".a/b");
    ignored_re!(ir13, "././", ".a/b", ".a/b");
    ignored_re!(ir14, "././.", ".a/b", ".a/b");
    ignored_re!(ir15, ROOT, r"\[", "[");
    ignored_re!(ir16, ROOT, r"\?", "?");
    ignored_re!(ir17, ROOT, r"\*", "*");
    ignored_re!(ir18, ROOT, "*.rs\n^a.o", "a.o"); // invalid regex skipped

    not_ignored_re!(irnot1, ROOT, r"^src/.*\.rs", "main.rs");
    not_ignored_re!(irnot2, ROOT, "*.rs\n^a.o", "main.rs"); // invalid regex
}
