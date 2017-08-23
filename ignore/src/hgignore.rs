/*!
doc
*/
use std::cell::RefCell;
use std::env;
use std::fs::File;
use std::io::{self, Read, BufRead};
use std::path::{PathBuf};
use std::str;
use std::sync::Arc;
use std::path::Path;

use globset::{Candidate, GlobSetBuilder, GlobSet, GlobBuilder};
use regex::bytes::Regex;
use thread_local::ThreadLocal;

use {Error, Match, PartialErrorBuilder};
use pathutil::{is_file_name, strip_prefix};


/// doc
#[derive(Clone, Debug)]
pub struct Glob {
    from: Option<PathBuf>,
    original: String,
    actual: String,
    is_only_dir: bool,
}

impl Glob {
    /// Returns the file path that defined this glob.
    pub fn from(&self) -> Option<&Path> {
        self.from.as_ref().map(|p| &**p)
    }

    /// The original glob as it was defined in a gitignore file.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// The actual glob that was compiled to respect gitignore
    /// semantics.
    pub fn actual(&self) -> &str {
        &self.actual
    }

    /// Whether this glob must match a directory or not.
    pub fn is_only_dir(&self) -> bool {
        self.is_only_dir
    }
}


/// doc
#[derive(Clone, Debug)]
pub struct Hgignore {
    set: GlobSet,
    root: PathBuf,
    globs: Vec<Glob>,
    num_ignores: u64,
    matches: Arc<ThreadLocal<RefCell<Vec<usize>>>>,
}

impl Hgignore {
    /// doc
    pub fn new<P: AsRef<Path>>(
        hgignore_path: P,
    ) -> (Hgignore, Option<Error>) {
        let path = hgignore_path.as_ref();
        let parent = path.parent().unwrap_or(Path::new("/"));
        let mut builder = HgignoreBuilder::new(parent);
        let mut errs = PartialErrorBuilder::default();
        errs.maybe_push_ignore_io(builder.add(path));
        match builder.build() {
            Ok(gi) => (gi, errs.into_error_option()),
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

    /// Returns true if and only if this gitignore has zero globs, and
    /// therefore never matches any file path.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Returns whether the given file path matched a pattern in this gitignore
    /// matcher.
    ///
    /// `is_dir` should be true if the path refers to a directory and false
    /// otherwise.
    ///
    /// The given path is matched relative to the path given when building
    /// the matcher. Specifically, before matching `path`, its prefix (as
    /// determined by a common suffix of the directory containing this
    /// gitignore) is stripped. If there is no common suffix/prefix overlap,
    /// then `path` is assumed to be relative to this matcher.
    pub fn matched<P: AsRef<Path>>(
        &self,
        path: P,
        is_dir: bool,
    ) -> Match<&Glob> {
        if self.is_empty() {
            return Match::None;
        }
        self.matched_stripped(self.strip(path.as_ref()), is_dir)
    }

    /// Like matched, but takes a path that has already been stripped.
    fn matched_stripped<P: AsRef<Path>>(
        &self,
        path: P,
        is_dir: bool,
    ) -> Match<&Glob> {
        if self.is_empty() {
            return Match::None;
        }
        let path = path.as_ref();
        let _matches = self.matches.get_default();
        let mut matches = _matches.borrow_mut();
        let candidate = Candidate::new(path);
        self.set.matches_candidate_into(&candidate, &mut *matches);
        for &i in matches.iter().rev() {
            let glob = &self.globs[i];
            if !glob.is_only_dir() || is_dir {
                return Match::Ignore(glob)
            };
        }
        Match::None
    }

    /// Strips the given path such that it's suitable for matching with this
    /// hgignore matcher.
    fn strip<'a, P: 'a + AsRef<Path> + ?Sized>(
        &'a self,
        path: &'a P,
    ) -> &'a Path {
        let mut path = path.as_ref();
        // A leading ./ is completely superfluous. We also strip it from
        // our gitignore root path, so we need to strip it from our candidate
        // path too.
        if let Some(p) = strip_prefix("./", path) {
            path = p;
        }
        // Strip any common prefix between the candidate path and the root
        // of the gitignore, to make sure we get relative matching right.
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
    globs: Vec<Glob>,
}

impl HgignoreBuilder {
    /// doc
    pub fn new<P: AsRef<Path>>(root: P) -> HgignoreBuilder {
        let root = root.as_ref();
        HgignoreBuilder {
            builder: GlobSetBuilder::new(),
            root: strip_prefix("./", root).unwrap_or(root).to_path_buf(),
            globs: vec![],
        }
    }

    /// doc
    pub fn build(&self) -> Result<Hgignore, Error> {
        let nignore = self.globs.iter().count();
        let set = try!(
            self.builder.build().map_err(|err| {
                Error::Glob {
                    glob: None,
                    err: err.to_string(),
                }
            }));
        Ok(Hgignore {
            set: set,
            root: self.root.clone(),
            globs: self.globs.clone(),
            num_ignores: nignore as u64,
            matches: Arc::new(ThreadLocal::default()),
        })
    }

    /// doc
    pub fn add<P: AsRef<Path>>(&mut self, path: P) -> Option<Error> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                r"^syntax:\s*(regexp|glob)\s*$").unwrap();
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
            match RE.captures(line.as_bytes()) {
                None => {
                    if let Err(err) = self.add_line(
                        Some(path.to_path_buf()), &line, current_syntax
                    ) {
                        errs.push(err.tagged(path, lineno));
                    };
                },
                Some(caps) => {
                    current_syntax = match &caps[1] {
                        b"glob" => HgignoreSyntax::Glob,
                        _ => HgignoreSyntax::Regexp,
                    }
                },
            };
        }
        errs.into_error_option()
    }

    /// doc
    pub fn add_line(
        &mut self,
        from: Option<PathBuf>,
        mut line: &str,
        current_syntax: HgignoreSyntax
    ) -> Result<&mut HgignoreBuilder, Error> {
        if line.starts_with("#") {
            return Ok(self);
        }
        if !line.ends_with("\\ ") {
            line = line.trim_right();
        }
        if line.is_empty() {
            return Ok(self);
        }
        match current_syntax {
            HgignoreSyntax::Glob => self.add_glob_line(from, line),
            HgignoreSyntax::Regexp => self.add_regex_line(from, line),
        }
    }

    /// doc
    pub fn add_regex_line(
        &mut self,
        from: Option<PathBuf>,
        line: &str,
    ) -> Result<&mut HgignoreBuilder, Error> {
        let glob = Glob {
            from: from,
            original: line.to_string(),
            actual: String::new(),
            is_only_dir: false,
        };
        let parsed = try!(
            GlobBuilder::new(&line.to_string())
                // .literal_separator(literal_separator)
                .from_regex(true)
                .build()
                .map_err(|err| {
                    Error::Glob {
                        glob: Some(glob.original.clone()),
                        err: err.kind().to_string(),
                    }
                }));
        self.builder.add(parsed);
        self.globs.push(glob);
        Ok(self)
    }

    /// doc
    pub fn add_glob_line(
        &mut self,
        from: Option<PathBuf>,
        mut line: &str,
    ) -> Result<&mut HgignoreBuilder, Error> {
        let mut glob = Glob {
            from: from,
            original: line.to_string(),
            actual: String::new(),
            is_only_dir: false,
        };
        let mut literal_separator = false;
        let has_slash = line.chars().any(|c| c == '/');
        let mut is_absolute = false;
        if line.starts_with("\\#") {
            line = &line[1..];
            is_absolute = line.chars().nth(0) == Some('/');
        } else {
            if line.starts_with("/") {
                // `man gitignore` says that if a glob starts with a slash,
                // then the glob can only match the beginning of a path
                // (relative to the location of gitignore). We achieve this by
                // simply banning wildcards from matching /.
                literal_separator = true;
                line = &line[1..];
                is_absolute = true;
            }
        }
        // If it ends with a slash, then this should only match directories,
        // but the slash should otherwise not be used while globbing.
        if let Some((i, c)) = line.char_indices().rev().nth(0) {
            if c == '/' {
                glob.is_only_dir = true;
                line = &line[..i];
            }
        }
        // If there is a literal slash, then we note that so that globbing
        // doesn't let wildcards match slashes.
        glob.actual = line.to_string();
        if has_slash {
            literal_separator = true;
        }
        // If there was a leading slash, then this is a glob that must
        // match the entire path name. Otherwise, we should let it match
        // anywhere, so use a **/ prefix.
        if !is_absolute {
            // ... but only if we don't already have a **/ prefix.
            if !glob.actual.starts_with("**/") {
                glob.actual = format!("**/{}", glob.actual);
            }
        }
        // If the glob ends with `/**`, then we should only match everything
        // inside a directory, but not the directory itself. Standard globs
        // will match the directory. So we add `/*` to force the issue.
        if glob.actual.ends_with("/**") {
            glob.actual = format!("{}/*", glob.actual);
        }
        let parsed = try!(
            GlobBuilder::new(&glob.actual)
                .literal_separator(literal_separator)
                .build()
                .map_err(|err| {
                    Error::Glob {
                        glob: Some(glob.original.clone()),
                        err: err.kind().to_string(),
                    }
                }));
        self.builder.add(parsed);
        self.globs.push(glob);
        Ok(self)

    }
}

/*
 * TODO:
 *   Global configuration like the username setting is typically put into:
 *
 *       o %USERPROFILE%\mercurial.ini (on Windows)
 *
 *       o $HOME/.hgrc (on Unix, Plan9)
 *
 *       The names of these files depend on the system on which Mercurial is installed. \*.rc files from a
 *       single  directory are read in alphabetical order, later ones overriding earlier ones. Where mul-
 *       tiple paths are given below, settings from earlier paths override later ones.
 *
 *       On Unix, the following files are consulted:
 *
 *       o <repo>/.hg/hgrc (per-repository)
 *
 *       o $HOME/.hgrc (per-user)
 *
 *       o <install-root>/etc/mercurial/hgrc (per-installation)
 *
 *       o <install-root>/etc/mercurial/hgrc.d/\*.rc (per-installation)
 *
 *       o /etc/mercurial/hgrc (per-system)
 *
 *       o /etc/mercurial/hgrc.d/\*.rc (per-system)
 *
 *       o <internal>/default.d/\*.rc (defaults)
 *
 *       On Windows, the following files are consulted:
 *
 *       o <repo>/.hg/hgrc (per-repository)
 *
 *       o %USERPROFILE%\.hgrc (per-user)
 *
 *       o %USERPROFILE%\Mercurial.ini (per-user)
 *
 *       o %HOME%\.hgrc (per-user)
 *
 *       o %HOME%\Mercurial.ini (per-user)
 *
 *       o HKEY_LOCAL_MACHINE\SOFTWARE\Mercurial (per-installation)
 *
 *       o <install-dir>\hgrc.d\*.rc (per-installation)
 *
 *       o <install-dir>\Mercurial.ini (per-installation)
 *
 *       o <internal>/default.d/\*.rc (defaults)
 *      Note   The registry key HKEY_LOCAL_MACHINE\SOFTWARE\Wow6432Node\Mercurial is used  when  running
 *              32-bit Python on 64-bit Windows.
 *
 *       On Windows 9x, %HOME% is replaced by %APPDATA%.
 *
 *       On Plan9, the following files are consulted:
 *
 *       o <repo>/.hg/hgrc (per-repository)
 *
 *       o $home/lib/hgrc (per-user)
 *
 *       o <install-root>/lib/mercurial/hgrc (per-installation)
 *
 *       o <install-root>/lib/mercurial/hgrc.d/\*.rc (per-installation)
 *
 *       o /lib/mercurial/hgrc (per-system)
 *
 *       o /lib/mercurial/hgrc.d/\*.rc (per-system)
 *
 *       o <internal>/default.d/\*.rc (defaults)
 */
/// FIXME: parses only ignore setting from $HOME/.hgrc
fn hgrc_ignore_path() -> Option<PathBuf> {
    hgrc_contents()
        .and_then(|data| parse_ignore_file(&data))
}

fn hgrc_contents() -> Option<Vec<u8>> {
    let home = match env::var_os("HOME") {
        None => return None,
        Some(home) => PathBuf::from(home),
    };
    let mut file = match File::open(home.join(".hgrc")) {
        Err(_) => return None,
        Ok(file) => io::BufReader::new(file),
    };
    let mut contents = vec![];
    file.read_to_end(&mut contents).ok().map(|_| contents)
}


fn parse_ignore_file(data: &[u8]) -> Option<PathBuf> {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"(?ium)^\s*ignore\s*=\s*(.+)\s*$").unwrap();
    };
    let caps = match RE.captures(data) {
        None => return None,
        Some(caps) => caps,
    };
    str::from_utf8(&caps[1]).ok().map(|s| PathBuf::from(expand_tilde(s)))
}

/// Expands ~ in file paths to the value of $HOME.
fn expand_tilde(path: &str) -> String {
    let home = match env::var("HOME") {
        Err(_) => return path.to_string(),
        Ok(home) => home,
    };
    path.replace("~", &home)
}
