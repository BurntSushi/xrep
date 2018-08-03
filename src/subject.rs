use std::io;
use std::path::Path;
use std::sync::Arc;

use ignore::{self, DirEntry};
use same_file::Handle;

/// A configuration for describing how subjects should be built.
#[derive(Clone, Debug)]
struct Config {
    skip: Option<Arc<Handle>>,
    strip_dot_prefix: bool,
    separator: Option<u8>,
    terminator: Option<u8>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            skip: None,
            strip_dot_prefix: false,
            separator: None,
            terminator: None,
        }
    }
}

/// A builder for constructing things to search over.
#[derive(Clone, Debug)]
pub struct SubjectBuilder {
    config: Config,
}

impl SubjectBuilder {
    /// Return a new subject builder with a default configuration.
    pub fn new() -> SubjectBuilder {
        SubjectBuilder { config: Config::default() }
    }

    /// Create a new subject from a possibly missing directory entry.
    ///
    /// If the directory entry isn't present, then the corresponding error is
    /// logged if messages have been configured. Otherwise, if the subject is
    /// deemed searchable, then it is returned.
    pub fn build_from_result(
        &self,
        result: Result<DirEntry, ignore::Error>,
    ) -> Option<Subject> {
        match result {
            Ok(dent) => self.build(dent),
            Err(err) => {
                message!("{}", err);
                None
            }
        }
    }

    /// Create a new subject using this builder's configuration.
    ///
    /// If a subject could not be created or should otherwise not be searched,
    /// then this returns `None` after emitting any relevant log messages.
    pub fn build(&self, dent: DirEntry) -> Option<Subject> {
        let subj = Subject {
            dent: dent,
            strip_dot_prefix: self.config.strip_dot_prefix,
        };
        if let Some(ignore_err) = subj.dent.error() {
            ignore_message!("{}", ignore_err);
        }
        // If this entry represents stdin, then we always search it.
        if subj.dent.is_stdin() {
            return Some(subj);
        }
        // If we're supposed to skip a particular file, then skip it.
        if let Some(ref handle) = self.config.skip {
            match subj.equals(handle) {
                Ok(false) => {} // fallthrough
                Ok(true) => {
                    debug!(
                        "ignoring {}: (probably same file as stdout)",
                        subj.dent.path().display()
                    );
                    return None;
                }
                Err(err) => {
                    debug!(
                        "ignoring {}: got error: {}",
                        subj.dent.path().display(), err
                    );
                    return None;
                }
            }
        }
        // If this subject has a depth of 0, then it was provided explicitly
        // by an end user (or via a shell glob). In this case, we always want
        // to search it if it even smells like a file (e.g., a symlink).
        if subj.dent.depth() == 0 && !subj.is_dir() {
            return Some(subj);
        }
        // At this point, we only want to search something it's explicitly a
        // file. This omits symlinks. (If ripgrep was configured to follow
        // symlinks, then they have already been followed by the directory
        // traversal.)
        if subj.is_file() {
            return Some(subj);
        }
        // We got nothin. Emit a debug message, but only if this isn't a
        // directory. Otherwise, emitting messages for directories is just
        // noisy.
        if !subj.is_dir() {
            debug!(
                "ignoring {}: failed to pass subject filter: \
                 file type: {:?}, metadata: {:?}",
                 subj.dent.path().display(),
                 subj.dent.file_type(),
                 subj.dent.metadata()
            );
        }
        None
    }

    /// When provided, subjects that represent the same file as the handle
    /// given will be skipped.
    ///
    /// Typically, it is useful to pass a handle referring to stdout, such
    /// that the file being written to isn't searched, which can lead to
    /// an unbounded feedback mechanism.
    ///
    /// Only one handle to skip can be provided.
    pub fn skip(
        &mut self,
        handle: Option<Handle>,
    ) -> &mut SubjectBuilder {
        self.config.skip = handle.map(Arc::new);
        self
    }

    /// When enabled, if the subject's file path starts with `./` then it is
    /// stripped.
    ///
    /// This is useful when implicitly searching the current working directory.
    pub fn strip_dot_prefix(&mut self, yes: bool) -> &mut SubjectBuilder {
        self.config.strip_dot_prefix = yes;
        self
    }
}

/// A subject is a thing we want to search. Generally, a subject is either a
/// file or stdin.
#[derive(Clone, Debug)]
pub struct Subject {
    dent: DirEntry,
    strip_dot_prefix: bool,
}

impl Subject {
    /// Return the file path corresponding to this subject.
    ///
    /// If this subject corresponds to stdin, then a special `<stdin>` path
    /// is returned instead.
    pub fn path(&self) -> &Path {
        if self.strip_dot_prefix && self.dent.path().starts_with("./") {
            self.dent.path().strip_prefix("./").unwrap()
        } else {
            self.dent.path()
        }
    }

    /// Returns true if and only if this entry corresponds to stdin.
    pub fn is_stdin(&self) -> bool {
        self.dent.is_stdin()
    }

    /// Returns true if and only if this subject points to a directory.
    ///
    /// This works around a bug in Rust's standard library:
    /// https://github.com/rust-lang/rust/issues/46484
    #[cfg(windows)]
    fn is_dir(&self) -> bool {
        use std::os::windows::fs::MetadataExt;
        use winapi::um::winnt::FILE_ATTRIBUTE_DIRECTORY;

        self.dent.metadata().map(|md| {
            md.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0
        }).unwrap_or(false)
    }

    /// Returns true if and only if this subject points to a directory.
    #[cfg(not(windows))]
    fn is_dir(&self) -> bool {
        self.dent.file_type().map_or(false, |ft| ft.is_dir())
    }

    /// Returns true if and only if this subject points to a file.
    ///
    /// This works around a bug in Rust's standard library:
    /// https://github.com/rust-lang/rust/issues/46484
    #[cfg(windows)]
    fn is_file(&self) -> bool {
        !self.is_dir()
    }

    /// Returns true if and only if this subject points to a file.
    #[cfg(not(windows))]
    fn is_file(&self) -> bool {
        self.dent.file_type().map_or(false, |ft| ft.is_file())
    }

    /// Returns true if and only if this subject is believed to be equivalent
    /// to the given handle. If there was a problem querying this subject for
    /// information to determine equality, then that error is returned.
    fn equals(&self, handle: &Handle) -> io::Result<bool> {
        #[cfg(unix)]
        fn never_equal(dent: &DirEntry, handle: &Handle) -> bool {
            dent.ino() != Some(handle.ino())
        }

        #[cfg(not(unix))]
        fn never_equal(_: &DirEntry, _: &Handle) -> bool {
            false
        }

        // If we know for sure that these two things aren't equal, then avoid
        // the costly extra stat call to determine equality.
        if self.dent.is_stdin() || never_equal(&self.dent, handle) {
            return Ok(false);
        }
        Handle::from_path(self.path()).map(|h| &h == handle)
    }
}
