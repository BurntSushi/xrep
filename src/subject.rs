use std::path::Path;

use ignore::{self, DirEntry};
use log;

/// A configuration for describing how subjects should be built.
#[derive(Clone, Debug)]
struct Config {
    strip_dot_prefix: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            strip_dot_prefix: false,
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
            log::debug!(
                "ignoring {}: failed to pass subject filter: \
                 file type: {:?}, metadata: {:?}",
                 subj.dent.path().display(),
                 subj.dent.file_type(),
                 subj.dent.metadata()
            );
        }
        None
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
    fn is_dir(&self) -> bool {
        self.dent.file_type().map_or(false, |ft| ft.is_dir())
    }

    /// Returns true if and only if this subject points to a file.
    fn is_file(&self) -> bool {
        self.dent.file_type().map_or(false, |ft| ft.is_file())
    }
}
