use std::cmp;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, FileType, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use std::vec;

use channel;
use same_file::Handle;
use walkdir::{self, WalkDir};

use dir::{Ignore, IgnoreBuilder};
use gitignore::GitignoreBuilder;
use overrides::Override;
use types::Types;
use {Error, PartialErrorBuilder};

/// A directory entry with a possible error attached.
///
/// The error typically refers to a problem parsing ignore files in a
/// particular directory.
#[derive(Clone, Debug)]
pub struct DirEntry {
    dent: DirEntryInner,
    err: Option<Error>,
}

impl DirEntry {
    /// The full path that this entry represents.
    pub fn path(&self) -> &Path {
        self.dent.path()
    }

    /// The full path that this entry represents.
    /// Analogous to [`path`], but moves ownership of the path.
    ///
    /// [`path`]: struct.DirEntry.html#method.path
    pub fn into_path(self) -> PathBuf {
        self.dent.into_path()
    }

    /// Whether this entry corresponds to a symbolic link or not.
    pub fn path_is_symlink(&self) -> bool {
        self.dent.path_is_symlink()
    }

    /// Returns true if and only if this entry corresponds to stdin.
    ///
    /// i.e., The entry has depth 0 and its file name is `-`.
    pub fn is_stdin(&self) -> bool {
        self.dent.is_stdin()
    }

    /// Return the metadata for the file that this entry points to.
    pub fn metadata(&self) -> Result<Metadata, Error> {
        self.dent.metadata()
    }

    /// Return the file type for the file that this entry points to.
    ///
    /// This entry doesn't have a file type if it corresponds to stdin.
    pub fn file_type(&self) -> Option<FileType> {
        self.dent.file_type()
    }

    /// Return the file name of this entry.
    ///
    /// If this entry has no file name (e.g., `/`), then the full path is
    /// returned.
    pub fn file_name(&self) -> &OsStr {
        self.dent.file_name()
    }

    /// Returns the depth at which this entry was created relative to the root.
    pub fn depth(&self) -> usize {
        self.dent.depth()
    }

    /// Returns the underlying inode number if one exists.
    ///
    /// If this entry doesn't have an inode number, then `None` is returned.
    #[cfg(unix)]
    pub fn ino(&self) -> Option<u64> {
        self.dent.ino()
    }

    /// Returns an error, if one exists, associated with processing this entry.
    ///
    /// An example of an error is one that occurred while parsing an ignore
    /// file. Errors related to traversing a directory tree itself are reported
    /// as part of yielding the directory entry, and not with this method.
    pub fn error(&self) -> Option<&Error> {
        self.err.as_ref()
    }

    /// Returns true if and only if this entry points to a directory.
    fn is_dir(&self) -> bool {
        self.dent.is_dir()
    }

    fn new_stdin() -> DirEntry {
        DirEntry {
            dent: DirEntryInner::Stdin,
            err: None,
        }
    }

    fn new_walkdir(dent: walkdir::DirEntry, err: Option<Error>) -> DirEntry {
        DirEntry {
            dent: DirEntryInner::Walkdir(dent),
            err: err,
        }
    }

    fn new_raw(dent: DirEntryRaw, err: Option<Error>) -> DirEntry {
        DirEntry {
            dent: DirEntryInner::Raw(dent),
            err: err,
        }
    }
}

/// DirEntryInner is the implementation of DirEntry.
///
/// It specifically represents three distinct sources of directory entries:
///
/// 1. From the walkdir crate.
/// 2. Special entries that represent things like stdin.
/// 3. From a path.
///
/// Specifically, (3) has to essentially re-create the DirEntry implementation
/// from WalkDir.
#[derive(Clone, Debug)]
enum DirEntryInner {
    Stdin,
    Walkdir(walkdir::DirEntry),
    Raw(DirEntryRaw),
}

impl DirEntryInner {
    fn path(&self) -> &Path {
        use self::DirEntryInner::*;
        match *self {
            Stdin => Path::new("<stdin>"),
            Walkdir(ref x) => x.path(),
            Raw(ref x) => x.path(),
        }
    }

    fn into_path(self) -> PathBuf {
        use self::DirEntryInner::*;
        match self {
            Stdin => PathBuf::from("<stdin>"),
            Walkdir(x) => x.into_path(),
            Raw(x) => x.into_path(),
        }
    }

    fn path_is_symlink(&self) -> bool {
        use self::DirEntryInner::*;
        match *self {
            Stdin => false,
            Walkdir(ref x) => x.path_is_symlink(),
            Raw(ref x) => x.path_is_symlink(),
        }
    }

    fn is_stdin(&self) -> bool {
        match *self {
            DirEntryInner::Stdin => true,
            _ => false,
        }
    }

    fn metadata(&self) -> Result<Metadata, Error> {
        use self::DirEntryInner::*;
        match *self {
            Stdin => {
                let err = Error::Io(io::Error::new(
                    io::ErrorKind::Other, "<stdin> has no metadata"));
                Err(err.with_path("<stdin>"))
            }
            Walkdir(ref x) => {
                x.metadata().map_err(|err| {
                    Error::Io(io::Error::from(err)).with_path(x.path())
                })
            }
            Raw(ref x) => x.metadata(),
        }
    }

    fn file_type(&self) -> Option<FileType> {
        use self::DirEntryInner::*;
        match *self {
            Stdin => None,
            Walkdir(ref x) => Some(x.file_type()),
            Raw(ref x) => Some(x.file_type()),
        }
    }

    fn file_name(&self) -> &OsStr {
        use self::DirEntryInner::*;
        match *self {
            Stdin => OsStr::new("<stdin>"),
            Walkdir(ref x) => x.file_name(),
            Raw(ref x) => x.file_name(),
        }
    }

    fn depth(&self) -> usize {
        use self::DirEntryInner::*;
        match *self {
            Stdin => 0,
            Walkdir(ref x) => x.depth(),
            Raw(ref x) => x.depth(),
        }
    }

    #[cfg(unix)]
    fn ino(&self) -> Option<u64> {
        use walkdir::DirEntryExt;
        use self::DirEntryInner::*;
        match *self {
            Stdin => None,
            Walkdir(ref x) => Some(x.ino()),
            Raw(ref x) => Some(x.ino()),
        }
    }

    /// Returns true if and only if this entry points to a directory.
    fn is_dir(&self) -> bool {
        self.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
    }
}

/// DirEntryRaw is essentially copied from the walkdir crate so that we can
/// build `DirEntry`s from whole cloth in the parallel iterator.
#[derive(Clone)]
struct DirEntryRaw {
    /// The path as reported by the `fs::ReadDir` iterator (even if it's a
    /// symbolic link).
    path: PathBuf,
    /// The file type. Necessary for recursive iteration, so store it.
    ty: FileType,
    /// Is set when this entry was created from a symbolic link and the user
    /// expects the iterator to follow symbolic links.
    follow_link: bool,
    /// The depth at which this entry was generated relative to the root.
    depth: usize,
    /// The underlying inode number (Unix only).
    #[cfg(unix)]
    ino: u64,
    /// The underlying metadata (Windows only). We store this on Windows
    /// because this comes for free while reading a directory.
    #[cfg(windows)]
    metadata: fs::Metadata,
}

impl fmt::Debug for DirEntryRaw {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Leaving out FileType because it doesn't have a debug impl
        // in Rust 1.9. We could add it if we really wanted to by manually
        // querying each possibly file type. Meh. ---AG
        f.debug_struct("DirEntryRaw")
            .field("path", &self.path)
            .field("follow_link", &self.follow_link)
            .field("depth", &self.depth)
            .finish()
    }
}

impl DirEntryRaw {
    fn path(&self) -> &Path {
        &self.path
    }

    fn into_path(self) -> PathBuf {
        self.path
    }

    fn path_is_symlink(&self) -> bool {
        self.ty.is_symlink() || self.follow_link
    }

    fn metadata(&self) -> Result<Metadata, Error> {
        self.metadata_internal()
    }

    #[cfg(windows)]
    fn metadata_internal(&self) -> Result<fs::Metadata, Error> {
        if self.follow_link {
            fs::metadata(&self.path)
        } else {
            Ok(self.metadata.clone())
        }.map_err(|err| Error::Io(io::Error::from(err)).with_path(&self.path))
    }

    #[cfg(not(windows))]
    fn metadata_internal(&self) -> Result<fs::Metadata, Error> {
        if self.follow_link {
            fs::metadata(&self.path)
        } else {
            fs::symlink_metadata(&self.path)
        }.map_err(|err| Error::Io(io::Error::from(err)).with_path(&self.path))
    }

    fn file_type(&self) -> FileType {
        self.ty
    }

    fn file_name(&self) -> &OsStr {
        self.path.file_name().unwrap_or_else(|| self.path.as_os_str())
    }

    fn depth(&self) -> usize {
        self.depth
    }

    #[cfg(unix)]
    fn ino(&self) -> u64 {
        self.ino
    }

    fn from_entry(
        depth: usize,
        ent: &fs::DirEntry,
    ) -> Result<DirEntryRaw, Error> {
        let ty = ent.file_type().map_err(|err| {
            let err = Error::Io(io::Error::from(err)).with_path(ent.path());
            Error::WithDepth {
                depth: depth,
                err: Box::new(err),
            }
        })?;
        DirEntryRaw::from_entry_os(depth, ent, ty)
    }

    #[cfg(windows)]
    fn from_entry_os(
        depth: usize,
        ent: &fs::DirEntry,
        ty: fs::FileType,
    ) -> Result<DirEntryRaw, Error> {
        let md = ent.metadata().map_err(|err| {
            let err = Error::Io(io::Error::from(err)).with_path(ent.path());
            Error::WithDepth {
                depth: depth,
                err: Box::new(err),
            }
        })?;
        Ok(DirEntryRaw {
            path: ent.path(),
            ty: ty,
            follow_link: false,
            depth: depth,
            metadata: md,
        })
    }

    #[cfg(unix)]
    fn from_entry_os(
        depth: usize,
        ent: &fs::DirEntry,
        ty: fs::FileType,
    ) -> Result<DirEntryRaw, Error> {
        use std::os::unix::fs::DirEntryExt;

        Ok(DirEntryRaw {
            path: ent.path(),
            ty: ty,
            follow_link: false,
            depth: depth,
            ino: ent.ino(),
        })
    }

    #[cfg(not(unix))]
    fn from_path(
        depth: usize,
        pb: PathBuf,
        link: bool,
    ) -> Result<DirEntryRaw, Error> {
        let md = fs::metadata(&pb).map_err(|err| {
            Error::Io(err).with_path(&pb)
        })?;
        Ok(DirEntryRaw {
            path: pb,
            ty: md.file_type(),
            follow_link: link,
            depth: depth,
            metadata: md,
        })
    }

    #[cfg(unix)]
    fn from_path(
        depth: usize,
        pb: PathBuf,
        link: bool,
    ) -> Result<DirEntryRaw, Error> {
        use std::os::unix::fs::MetadataExt;

        let md = fs::metadata(&pb).map_err(|err| {
            Error::Io(err).with_path(&pb)
        })?;
        Ok(DirEntryRaw {
            path: pb,
            ty: md.file_type(),
            follow_link: link,
            depth: depth,
            ino: md.ino(),
        })
    }
}

/// WalkBuilder builds a recursive directory iterator.
///
/// The builder supports a large number of configurable options. This includes
/// specific glob overrides, file type matching, toggling whether hidden
/// files are ignored or not, and of course, support for respecting gitignore
/// files.
///
/// By default, all ignore files found are respected. This includes `.ignore`,
/// `.gitignore`, `.git/info/exclude` and even your global gitignore
/// globs, usually found in `$XDG_CONFIG_HOME/git/ignore`.
///
/// Some standard recursive directory options are also supported, such as
/// limiting the recursive depth or whether to follow symbolic links (disabled
/// by default).
///
/// # Ignore rules
///
/// There are many rules that influence whether a particular file or directory
/// is skipped by this iterator. Those rules are documented here. Note that
/// the rules assume a default configuration.
///
/// * First, glob overrides are checked. If a path matches a glob override,
/// then matching stops. The path is then only skipped if the glob that matched
/// the path is an ignore glob. (An override glob is a whitelist glob unless it
/// starts with a `!`, in which case it is an ignore glob.)
/// * Second, ignore files are checked. Ignore files currently only come from
/// git ignore files (`.gitignore`, `.git/info/exclude` and the configured
/// global gitignore file), plain `.ignore` files, which have the same format
/// as gitignore files, or explicitly added ignore files. The precedence order
/// is: `.ignore`, `.gitignore`, `.git/info/exclude`, global gitignore and
/// finally explicitly added ignore files. Note that precedence between
/// different types of ignore files is not impacted by the directory hierarchy;
/// any `.ignore` file overrides all `.gitignore` files. Within each precedence
/// level, more nested ignore files have a higher precedence than less nested
/// ignore files.
/// * Third, if the previous step yields an ignore match, then all matching
/// is stopped and the path is skipped. If it yields a whitelist match, then
/// matching continues. A whitelist match can be overridden by a later matcher.
/// * Fourth, unless the path is a directory, the file type matcher is run on
/// the path. As above, if it yields an ignore match, then all matching is
/// stopped and the path is skipped. If it yields a whitelist match, then
/// matching continues.
/// * Fifth, if the path hasn't been whitelisted and it is hidden, then the
/// path is skipped.
/// * Sixth, unless the path is a directory, the size of the file is compared
/// against the max filesize limit. If it exceeds the limit, it is skipped.
/// * Seventh, if the path has made it this far then it is yielded in the
/// iterator.
#[derive(Clone)]
pub struct WalkBuilder {
    paths: Vec<PathBuf>,
    ig_builder: IgnoreBuilder,
    max_depth: Option<usize>,
    max_filesize: Option<u64>,
    follow_links: bool,
    same_file_system: bool,
    sorter: Option<Sorter>,
    threads: usize,
    skip: Option<Arc<Handle>>,
}

#[derive(Clone)]
enum Sorter {
    ByName(Arc<Fn(&OsStr, &OsStr) -> cmp::Ordering + Send + Sync + 'static>),
    ByPath(Arc<Fn(&Path, &Path) -> cmp::Ordering + Send + Sync + 'static>),
}

impl fmt::Debug for WalkBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WalkBuilder")
            .field("paths", &self.paths)
            .field("ig_builder", &self.ig_builder)
            .field("max_depth", &self.max_depth)
            .field("max_filesize", &self.max_filesize)
            .field("follow_links", &self.follow_links)
            .field("threads", &self.threads)
            .field("skip", &self.skip)
            .finish()
    }
}

impl WalkBuilder {
    /// Create a new builder for a recursive directory iterator for the
    /// directory given.
    ///
    /// Note that if you want to traverse multiple different directories, it
    /// is better to call `add` on this builder than to create multiple
    /// `Walk` values.
    pub fn new<P: AsRef<Path>>(path: P) -> WalkBuilder {
        WalkBuilder {
            paths: vec![path.as_ref().to_path_buf()],
            ig_builder: IgnoreBuilder::new(),
            max_depth: None,
            max_filesize: None,
            follow_links: false,
            same_file_system: false,
            sorter: None,
            threads: 0,
            skip: None,
        }
    }

    /// Build a new `Walk` iterator.
    pub fn build(&self) -> Walk {
        let follow_links = self.follow_links;
        let max_depth = self.max_depth;
        let sorter = self.sorter.clone();
        let its = self.paths.iter().map(move |p| {
            if p == Path::new("-") {
                (p.to_path_buf(), None)
            } else {
                let mut wd = WalkDir::new(p);
                wd = wd.follow_links(follow_links || p.is_file());
                wd = wd.same_file_system(self.same_file_system);
                if let Some(max_depth) = max_depth {
                    wd = wd.max_depth(max_depth);
                }
                if let Some(ref sorter) = sorter {
                    match sorter.clone() {
                        Sorter::ByName(cmp) => {
                            wd = wd.sort_by(move |a, b| {
                                cmp(a.file_name(), b.file_name())
                            });
                        }
                        Sorter::ByPath(cmp) => {
                            wd = wd.sort_by(move |a, b| {
                                cmp(a.path(), b.path())
                            });
                        }
                    }
                }
                (p.to_path_buf(), Some(WalkEventIter::from(wd)))
            }
        }).collect::<Vec<_>>().into_iter();
        let ig_root = self.ig_builder.build();
        Walk {
            its: its,
            it: None,
            ig_root: ig_root.clone(),
            ig: ig_root.clone(),
            max_filesize: self.max_filesize,
            skip: self.skip.clone(),
        }
    }

    /// Build a new `WalkParallel` iterator.
    ///
    /// Note that this *doesn't* return something that implements `Iterator`.
    /// Instead, the returned value must be run with a closure. e.g.,
    /// `builder.build_parallel().run(|| |path| println!("{:?}", path))`.
    pub fn build_parallel(&self) -> WalkParallel {
        WalkParallel {
            paths: self.paths.clone().into_iter(),
            ig_root: self.ig_builder.build(),
            max_depth: self.max_depth,
            max_filesize: self.max_filesize,
            follow_links: self.follow_links,
            same_file_system: self.same_file_system,
            threads: self.threads,
            skip: self.skip.clone(),
        }
    }

    /// Add a file path to the iterator.
    ///
    /// Each additional file path added is traversed recursively. This should
    /// be preferred over building multiple `Walk` iterators since this
    /// enables reusing resources across iteration.
    pub fn add<P: AsRef<Path>>(&mut self, path: P) -> &mut WalkBuilder {
        self.paths.push(path.as_ref().to_path_buf());
        self
    }

    /// The maximum depth to recurse.
    ///
    /// The default, `None`, imposes no depth restriction.
    pub fn max_depth(&mut self, depth: Option<usize>) -> &mut WalkBuilder {
        self.max_depth = depth;
        self
    }

    /// Whether to follow symbolic links or not.
    pub fn follow_links(&mut self, yes: bool) -> &mut WalkBuilder {
        self.follow_links = yes;
        self
    }

    /// Whether to ignore files above the specified limit.
    pub fn max_filesize(&mut self, filesize: Option<u64>) -> &mut WalkBuilder {
        self.max_filesize = filesize;
        self
    }

    /// The number of threads to use for traversal.
    ///
    /// Note that this only has an effect when using `build_parallel`.
    ///
    /// The default setting is `0`, which chooses the number of threads
    /// automatically using heuristics.
    pub fn threads(&mut self, n: usize) -> &mut WalkBuilder {
        self.threads = n;
        self
    }

    /// Add a global ignore file to the matcher.
    ///
    /// This has lower precedence than all other sources of ignore rules.
    ///
    /// If there was a problem adding the ignore file, then an error is
    /// returned. Note that the error may indicate *partial* failure. For
    /// example, if an ignore file contains an invalid glob, all other globs
    /// are still applied.
    pub fn add_ignore<P: AsRef<Path>>(&mut self, path: P) -> Option<Error> {
        let mut builder = GitignoreBuilder::new("");
        let mut errs = PartialErrorBuilder::default();
        errs.maybe_push(builder.add(path));
        match builder.build() {
            Ok(gi) => { self.ig_builder.add_ignore(gi); }
            Err(err) => { errs.push(err); }
        }
        errs.into_error_option()
    }

    /// Add a custom ignore file name
    ///
    /// These ignore files have higher precedence than all other ignore files.
    ///
    /// When specifying multiple names, earlier names have lower precedence than
    /// later names.
    pub fn add_custom_ignore_filename<S: AsRef<OsStr>>(
        &mut self,
        file_name: S
    ) -> &mut WalkBuilder {
        self.ig_builder.add_custom_ignore_filename(file_name);
        self
    }

    /// Add an override matcher.
    ///
    /// By default, no override matcher is used.
    ///
    /// This overrides any previous setting.
    pub fn overrides(&mut self, overrides: Override) -> &mut WalkBuilder {
        self.ig_builder.overrides(overrides);
        self
    }

    /// Add a file type matcher.
    ///
    /// By default, no file type matcher is used.
    ///
    /// This overrides any previous setting.
    pub fn types(&mut self, types: Types) -> &mut WalkBuilder {
        self.ig_builder.types(types);
        self
    }

    /// Enables all the standard ignore filters.
    ///
    /// This toggles, as a group, all the filters that are enabled by default:
    ///
    /// - [hidden()](#method.hidden)
    /// - [parents()](#method.parents)
    /// - [ignore()](#method.ignore)
    /// - [git_ignore()](#method.git_ignore)
    /// - [git_global()](#method.git_global)
    /// - [git_exclude()](#method.git_exclude)
    ///
    /// They may still be toggled individually after calling this function.
    ///
    /// This is (by definition) enabled by default.
    pub fn standard_filters(&mut self, yes: bool) -> &mut WalkBuilder {
        self.hidden(yes)
            .parents(yes)
            .ignore(yes)
            .git_ignore(yes)
            .git_global(yes)
            .git_exclude(yes)
    }

    /// Enables ignoring hidden files.
    ///
    /// This is enabled by default.
    pub fn hidden(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.hidden(yes);
        self
    }

    /// Enables reading ignore files from parent directories.
    ///
    /// If this is enabled, then .gitignore files in parent directories of each
    /// file path given are respected. Otherwise, they are ignored.
    ///
    /// This is enabled by default.
    pub fn parents(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.parents(yes);
        self
    }

    /// Enables reading `.ignore` files.
    ///
    /// `.ignore` files have the same semantics as `gitignore` files and are
    /// supported by search tools such as ripgrep and The Silver Searcher.
    ///
    /// This is enabled by default.
    pub fn ignore(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.ignore(yes);
        self
    }

    /// Enables reading a global gitignore file, whose path is specified in
    /// git's `core.excludesFile` config option.
    ///
    /// Git's config file location is `$HOME/.gitconfig`. If `$HOME/.gitconfig`
    /// does not exist or does not specify `core.excludesFile`, then
    /// `$XDG_CONFIG_HOME/git/ignore` is read. If `$XDG_CONFIG_HOME` is not
    /// set or is empty, then `$HOME/.config/git/ignore` is used instead.
    ///
    /// This is enabled by default.
    pub fn git_global(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.git_global(yes);
        self
    }

    /// Enables reading `.gitignore` files.
    ///
    /// `.gitignore` files have match semantics as described in the `gitignore`
    /// man page.
    ///
    /// This is enabled by default.
    pub fn git_ignore(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.git_ignore(yes);
        self
    }

    /// Enables reading `.git/info/exclude` files.
    ///
    /// `.git/info/exclude` files have match semantics as described in the
    /// `gitignore` man page.
    ///
    /// This is enabled by default.
    pub fn git_exclude(&mut self, yes: bool) -> &mut WalkBuilder {
        self.ig_builder.git_exclude(yes);
        self
    }

    /// Set a function for sorting directory entries by their path.
    ///
    /// If a compare function is set, the resulting iterator will return all
    /// paths in sorted order. The compare function will be called to compare
    /// entries from the same directory.
    ///
    /// This is like `sort_by_file_name`, except the comparator accepts
    /// a `&Path` instead of the base file name, which permits it to sort by
    /// more criteria.
    ///
    /// This method will override any previous sorter set by this method or
    /// by `sort_by_file_name`.
    ///
    /// Note that this is not used in the parallel iterator.
    pub fn sort_by_file_path<F>(
        &mut self,
        cmp: F,
    ) -> &mut WalkBuilder
    where F: Fn(&Path, &Path) -> cmp::Ordering + Send + Sync + 'static
    {
        self.sorter = Some(Sorter::ByPath(Arc::new(cmp)));
        self
    }

    /// Set a function for sorting directory entries by file name.
    ///
    /// If a compare function is set, the resulting iterator will return all
    /// paths in sorted order. The compare function will be called to compare
    /// names from entries from the same directory using only the name of the
    /// entry.
    ///
    /// This method will override any previous sorter set by this method or
    /// by `sort_by_file_path`.
    ///
    /// Note that this is not used in the parallel iterator.
    pub fn sort_by_file_name<F>(&mut self, cmp: F) -> &mut WalkBuilder
    where F: Fn(&OsStr, &OsStr) -> cmp::Ordering + Send + Sync + 'static
    {
        self.sorter = Some(Sorter::ByName(Arc::new(cmp)));
        self
    }

    /// Do not cross file system boundaries.
    ///
    /// When this option is enabled, directory traversal will not descend into
    /// directories that are on a different file system from the root path.
    ///
    /// Currently, this option is only supported on Unix and Windows. If this
    /// option is used on an unsupported platform, then directory traversal
    /// will immediately return an error and will not yield any entries.
    pub fn same_file_system(&mut self, yes: bool) -> &mut WalkBuilder {
        self.same_file_system = yes;
        self
    }

    /// Do not yield directory entries that are believed to correspond to
    /// stdout.
    ///
    /// This is useful when a command is invoked via shell redirection to a
    /// file that is also being read. For example, `grep -r foo ./ > results`
    /// might end up trying to search `results` even though it is also writing
    /// to it, which could cause an unbounded feedback loop. Setting this
    /// option prevents this from happening by skipping over the `results`
    /// file.
    ///
    /// This is disabled by default.
    pub fn skip_stdout(&mut self, yes: bool) -> &mut WalkBuilder {
        if yes {
            self.skip = stdout_handle().map(Arc::new);
        } else {
            self.skip = None;
        }
        self
    }
}

/// Walk is a recursive directory iterator over file paths in one or more
/// directories.
///
/// Only file and directory paths matching the rules are returned. By default,
/// ignore files like `.gitignore` are respected. The precise matching rules
/// and precedence is explained in the documentation for `WalkBuilder`.
pub struct Walk {
    its: vec::IntoIter<(PathBuf, Option<WalkEventIter>)>,
    it: Option<WalkEventIter>,
    ig_root: Ignore,
    ig: Ignore,
    max_filesize: Option<u64>,
    skip: Option<Arc<Handle>>,
}

impl Walk {
    /// Creates a new recursive directory iterator for the file path given.
    ///
    /// Note that this uses default settings, which include respecting
    /// `.gitignore` files. To configure the iterator, use `WalkBuilder`
    /// instead.
    pub fn new<P: AsRef<Path>>(path: P) -> Walk {
        WalkBuilder::new(path).build()
    }

    fn skip_entry(&self, ent: &DirEntry) -> Result<bool, Error> {
        if ent.depth() == 0 {
            return Ok(false);
        }

        if let Some(ref stdout) = self.skip {
            if path_equals(ent, stdout)? {
                return Ok(true);
            }
        }
        let is_dir = ent.file_type().map_or(false, |ft| ft.is_dir());
        let max_size = self.max_filesize;
        let should_skip_path = skip_path(&self.ig, ent.path(), is_dir);
        let should_skip_filesize = if !is_dir && max_size.is_some() {
            skip_filesize(max_size.unwrap(), ent.path(), &ent.metadata().ok())
        } else {
            false
        };

        Ok(should_skip_path || should_skip_filesize)
    }
}

impl Iterator for Walk {
    type Item = Result<DirEntry, Error>;

    #[inline(always)]
    fn next(&mut self) -> Option<Result<DirEntry, Error>> {
        loop {
            let ev = match self.it.as_mut().and_then(|it| it.next()) {
                Some(ev) => ev,
                None => {
                    match self.its.next() {
                        None => return None,
                        Some((_, None)) => {
                            return Some(Ok(DirEntry::new_stdin()));
                        }
                        Some((path, Some(it))) => {
                            self.it = Some(it);
                            if path.is_dir() {
                                let (ig, err) = self.ig_root.add_parents(path);
                                self.ig = ig;
                                if let Some(err) = err {
                                    return Some(Err(err));
                                }
                            } else {
                                self.ig = self.ig_root.clone();
                            }
                        }
                    }
                    continue;
                }
            };
            match ev {
                Err(err) => {
                    return Some(Err(Error::from_walkdir(err)));
                }
                Ok(WalkEvent::Exit) => {
                    self.ig = self.ig.parent().unwrap();
                }
                Ok(WalkEvent::Dir(ent)) => {
                    let mut ent = DirEntry::new_walkdir(ent, None);
                    let should_skip = match self.skip_entry(&ent) {
                        Err(err) => return Some(Err(err)),
                        Ok(should_skip) => should_skip,
                    };
                    if should_skip {
                        self.it.as_mut().unwrap().it.skip_current_dir();
                        // Still need to push this on the stack because
                        // we'll get a WalkEvent::Exit event for this dir.
                        // We don't care if it errors though.
                        let (igtmp, _) = self.ig.add_child(ent.path());
                        self.ig = igtmp;
                        continue;
                    }
                    let (igtmp, err) = self.ig.add_child(ent.path());
                    self.ig = igtmp;
                    ent.err = err;
                    return Some(Ok(ent));
                }
                Ok(WalkEvent::File(ent)) => {
                    let ent = DirEntry::new_walkdir(ent, None);
                    let should_skip = match self.skip_entry(&ent) {
                        Err(err) => return Some(Err(err)),
                        Ok(should_skip) => should_skip,
                    };
                    if should_skip {
                        continue;
                    }
                    return Some(Ok(ent));
                }
            }
        }
    }
}

/// WalkEventIter transforms a WalkDir iterator into an iterator that more
/// accurately describes the directory tree. Namely, it emits events that are
/// one of three types: directory, file or "exit." An "exit" event means that
/// the entire contents of a directory have been enumerated.
struct WalkEventIter {
    depth: usize,
    it: walkdir::IntoIter,
    next: Option<Result<walkdir::DirEntry, walkdir::Error>>,
}

#[derive(Debug)]
enum WalkEvent {
    Dir(walkdir::DirEntry),
    File(walkdir::DirEntry),
    Exit,
}

impl From<WalkDir> for WalkEventIter {
    fn from(it: WalkDir) -> WalkEventIter {
        WalkEventIter { depth: 0, it: it.into_iter(), next: None }
    }
}

impl Iterator for WalkEventIter {
    type Item = walkdir::Result<WalkEvent>;

    #[inline(always)]
    fn next(&mut self) -> Option<walkdir::Result<WalkEvent>> {
        let dent = self.next.take().or_else(|| self.it.next());
        let depth = match dent {
            None => 0,
            Some(Ok(ref dent)) => dent.depth(),
            Some(Err(ref err)) => err.depth(),
        };
        if depth < self.depth {
            self.depth -= 1;
            self.next = dent;
            return Some(Ok(WalkEvent::Exit));
        }
        self.depth = depth;
        match dent {
            None => None,
            Some(Err(err)) => Some(Err(err)),
            Some(Ok(dent)) => {
                if dent.file_type().is_dir() {
                    self.depth += 1;
                    Some(Ok(WalkEvent::Dir(dent)))
                } else {
                    Some(Ok(WalkEvent::File(dent)))
                }
            }
        }
    }
}

/// WalkState is used in the parallel recursive directory iterator to indicate
/// whether walking should continue as normal, skip descending into a
/// particular directory or quit the walk entirely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WalkState {
    /// Continue walking as normal.
    Continue,
    /// If the directory entry given is a directory, don't descend into it.
    /// In all other cases, this has no effect.
    Skip,
    /// Quit the entire iterator as soon as possible.
    ///
    /// Note that this is an inherently asynchronous action. It is possible
    /// for more entries to be yielded even after instructing the iterator
    /// to quit.
    Quit,
}

impl WalkState {
    fn is_quit(&self) -> bool {
        *self == WalkState::Quit
    }
}

/// WalkParallel is a parallel recursive directory iterator over files paths
/// in one or more directories.
///
/// Only file and directory paths matching the rules are returned. By default,
/// ignore files like `.gitignore` are respected. The precise matching rules
/// and precedence is explained in the documentation for `WalkBuilder`.
///
/// Unlike `Walk`, this uses multiple threads for traversing a directory.
pub struct WalkParallel {
    paths: vec::IntoIter<PathBuf>,
    ig_root: Ignore,
    max_filesize: Option<u64>,
    max_depth: Option<usize>,
    follow_links: bool,
    same_file_system: bool,
    threads: usize,
    skip: Option<Arc<Handle>>,
}

impl WalkParallel {
    /// Execute the parallel recursive directory iterator. `mkf` is called
    /// for each thread used for iteration. The function produced by `mkf`
    /// is then in turn called for each visited file path.
    pub fn run<F>(
        self,
        mut mkf: F,
    ) where F: FnMut() -> Box<FnMut(Result<DirEntry, Error>) -> WalkState + Send + 'static> {
        let mut f = mkf();
        let threads = self.threads();
        // TODO: Figure out how to use a bounded channel here. With an
        // unbounded channel, the workers can run away and fill up memory
        // with all of the file paths. But a bounded channel doesn't work since
        // our producers are also are consumers, so they end up getting stuck.
        //
        // We probably need to rethink parallel traversal completely to fix
        // this. The best case scenario would be finding a way to use rayon
        // to do this.
        let (tx, rx) = channel::unbounded();
        let mut any_work = false;
        // Send the initial set of root paths to the pool of workers.
        // Note that we only send directories. For files, we send to them the
        // callback directly.
        for path in self.paths {
            let (dent, root_device) =
                if path == Path::new("-") {
                    (DirEntry::new_stdin(), None)
                } else {
                    let root_device =
                        if !self.same_file_system {
                            None
                        } else {
                            match device_num(&path) {
                                Ok(root_device) => Some(root_device),
                                Err(err) => {
                                    let err = Error::Io(err).with_path(path);
                                    if f(Err(err)).is_quit() {
                                        return;
                                    }
                                    continue;
                                }
                            }
                        };
                    match DirEntryRaw::from_path(0, path, false) {
                        Ok(dent) => {
                            (DirEntry::new_raw(dent, None), root_device)
                        }
                        Err(err) => {
                            if f(Err(err)).is_quit() {
                                return;
                            }
                            continue;
                        }
                    }
                };
            tx.send(Message::Work(Work {
                dent: dent,
                ignore: self.ig_root.clone(),
                root_device: root_device,
            }));
            any_work = true;
        }
        // ... but there's no need to start workers if we don't need them.
        if !any_work {
            return;
        }
        // Create the workers and then wait for them to finish.
        let num_waiting = Arc::new(AtomicUsize::new(0));
        let num_quitting = Arc::new(AtomicUsize::new(0));
        let quit_now = Arc::new(AtomicBool::new(false));
        let mut handles = vec![];
        for _ in 0..threads {
            let worker = Worker {
                f: mkf(),
                tx: tx.clone(),
                rx: rx.clone(),
                quit_now: quit_now.clone(),
                is_waiting: false,
                is_quitting: false,
                num_waiting: num_waiting.clone(),
                num_quitting: num_quitting.clone(),
                threads: threads,
                max_depth: self.max_depth,
                max_filesize: self.max_filesize,
                follow_links: self.follow_links,
                skip: self.skip.clone(),
            };
            handles.push(thread::spawn(|| worker.run()));
        }
        drop(tx);
        drop(rx);
        for handle in handles {
            handle.join().unwrap();
        }
    }

    fn threads(&self) -> usize {
        if self.threads == 0 {
            2
        } else {
            self.threads
        }
    }
}

/// Message is the set of instructions that a worker knows how to process.
enum Message {
    /// A work item corresponds to a directory that should be descended into.
    /// Work items for entries that should be skipped or ignored should not
    /// be produced.
    Work(Work),
    /// This instruction indicates that the worker should start quitting.
    Quit,
}

/// A unit of work for each worker to process.
///
/// Each unit of work corresponds to a directory that should be descended
/// into.
struct Work {
    /// The directory entry.
    dent: DirEntry,
    /// Any ignore matchers that have been built for this directory's parents.
    ignore: Ignore,
    /// The root device number. When present, only files with the same device
    /// number should be considered.
    root_device: Option<u64>,
}

impl Work {
    /// Returns true if and only if this work item is a directory.
    fn is_dir(&self) -> bool {
        self.dent.is_dir()
    }

    /// Returns true if and only if this work item is a symlink.
    fn is_symlink(&self) -> bool {
        self.dent.file_type().map_or(false, |ft| ft.is_symlink())
    }

    /// Adds ignore rules for parent directories.
    ///
    /// Note that this only applies to entries at depth 0. On all other
    /// entries, this is a no-op.
    fn add_parents(&mut self) -> Option<Error> {
        if self.dent.depth() > 0 {
            return None;
        }
        // At depth 0, the path of this entry is a root path, so we can
        // use it directly to add parent ignore rules.
        let (ig, err) = self.ignore.add_parents(self.dent.path());
        self.ignore = ig;
        err
    }

    /// Reads the directory contents of this work item and adds ignore
    /// rules for this directory.
    ///
    /// If there was a problem with reading the directory contents, then
    /// an error is returned. If there was a problem reading the ignore
    /// rules for this directory, then the error is attached to this
    /// work item's directory entry.
    fn read_dir(&mut self) -> Result<fs::ReadDir, Error> {
        let readdir = match fs::read_dir(self.dent.path()) {
            Ok(readdir) => readdir,
            Err(err) => {
                let err = Error::from(err)
                    .with_path(self.dent.path())
                    .with_depth(self.dent.depth());
                return Err(err);
            }
        };
        let (ig, err) = self.ignore.add_child(self.dent.path());
        self.ignore = ig;
        self.dent.err = err;
        Ok(readdir)
    }
}

/// A worker is responsible for descending into directories, updating the
/// ignore matchers, producing new work and invoking the caller's callback.
///
/// Note that a worker is *both* a producer and a consumer.
struct Worker {
    /// The caller's callback.
    f: Box<FnMut(Result<DirEntry, Error>) -> WalkState + Send + 'static>,
    /// The push side of our mpmc queue.
    tx: channel::Sender<Message>,
    /// The receive side of our mpmc queue.
    rx: channel::Receiver<Message>,
    /// Whether all workers should quit at the next opportunity. Note that
    /// this is distinct from quitting because of exhausting the contents of
    /// a directory. Instead, this is used when the caller's callback indicates
    /// that the iterator should quit immediately.
    quit_now: Arc<AtomicBool>,
    /// Whether this worker is waiting for more work.
    is_waiting: bool,
    /// Whether this worker has started to quit.
    is_quitting: bool,
    /// The number of workers waiting for more work.
    num_waiting: Arc<AtomicUsize>,
    /// The number of workers waiting to quit.
    num_quitting: Arc<AtomicUsize>,
    /// The total number of workers.
    threads: usize,
    /// The maximum depth of directories to descend. A value of `0` means no
    /// descension at all.
    max_depth: Option<usize>,
    /// The maximum size a searched file can be (in bytes). If a file exceeds
    /// this size it will be skipped.
    max_filesize: Option<u64>,
    /// Whether to follow symbolic links or not. When this is enabled, loop
    /// detection is performed.
    follow_links: bool,
    /// A file handle to skip, currently is either `None` or stdout, if it's
    /// a file and it has been requested to skip files identical to stdout.
    skip: Option<Arc<Handle>>,
}

impl Worker {
    /// Runs this worker until there is no more work left to do.
    ///
    /// The worker will call the caller's callback for all entries that aren't
    /// skipped by the ignore matcher.
    fn run(mut self) {
        while let Some(mut work) = self.get_work() {
            // If the work is not a directory, then we can just execute the
            // caller's callback immediately and move on.
            if work.is_symlink() || !work.is_dir() {
                if (self.f)(Ok(work.dent)).is_quit() {
                    self.quit_now();
                    return;
                }
                continue;
            }
            if let Some(err) = work.add_parents() {
                if (self.f)(Err(err)).is_quit() {
                    self.quit_now();
                    return;
                }
            }
            let readdir = match work.read_dir() {
                Ok(readdir) => readdir,
                Err(err) => {
                    if (self.f)(Err(err)).is_quit() {
                        self.quit_now();
                        return;
                    }
                    continue;
                }
            };
            let descend =
                if let Some(root_device) = work.root_device {
                    match is_same_file_system(root_device, work.dent.path()) {
                        Ok(true) => true,
                        Ok(false) => false,
                        Err(err) => {
                            if (self.f)(Err(err)).is_quit() {
                                self.quit_now();
                                return;
                            }
                            false
                        }
                    }
                } else {
                    true
                };

            let depth = work.dent.depth();
            match (self.f)(Ok(work.dent)) {
                WalkState::Continue => {}
                WalkState::Skip => continue,
                WalkState::Quit => {
                    self.quit_now();
                    return;
                }
            }
            if !descend {
                continue;
            }
            if self.max_depth.map_or(false, |max| depth >= max) {
                continue;
            }
            for result in readdir {
                let state = self.run_one(
                    &work.ignore,
                    depth + 1,
                    work.root_device,
                    result,
                );
                if state.is_quit() {
                    self.quit_now();
                    return;
                }
            }
        }
    }

    /// Runs the worker on a single entry from a directory iterator.
    ///
    /// If the entry is a path that should be ignored, then this is a no-op.
    /// Otherwise, the entry is pushed on to the queue. (The actual execution
    /// of the callback happens in `run`.)
    ///
    /// If an error occurs while reading the entry, then it is sent to the
    /// caller's callback.
    ///
    /// `ig` is the `Ignore` matcher for the parent directory. `depth` should
    /// be the depth of this entry. `result` should be the item yielded by
    /// a directory iterator.
    fn run_one(
        &mut self,
        ig: &Ignore,
        depth: usize,
        root_device: Option<u64>,
        result: Result<fs::DirEntry, io::Error>,
    ) -> WalkState {
        let fs_dent = match result {
            Ok(fs_dent) => fs_dent,
            Err(err) => {
                return (self.f)(Err(Error::from(err).with_depth(depth)));
            }
        };
        let mut dent = match DirEntryRaw::from_entry(depth, &fs_dent) {
            Ok(dent) => DirEntry::new_raw(dent, None),
            Err(err) => {
                return (self.f)(Err(err));
            }
        };
        let is_symlink = dent.file_type().map_or(false, |ft| ft.is_symlink());
        if self.follow_links && is_symlink {
            let path = dent.path().to_path_buf();
            dent = match DirEntryRaw::from_path(depth, path, true) {
                Ok(dent) => DirEntry::new_raw(dent, None),
                Err(err) => {
                    return (self.f)(Err(err));
                }
            };
            if dent.is_dir() {
                if let Err(err) = check_symlink_loop(ig, dent.path(), depth) {
                    return (self.f)(Err(err));
                }
            }
        }
        if let Some(ref stdout) = self.skip {
            let is_stdout = match path_equals(&dent, stdout) {
                Ok(is_stdout) => is_stdout,
                Err(err) => return (self.f)(Err(err)),
            };
            if is_stdout {
                return WalkState::Continue;
            }
        }
        let is_dir = dent.is_dir();
        let max_size = self.max_filesize;
        let should_skip_path = skip_path(ig, dent.path(), is_dir);
        let should_skip_filesize =
            if !is_dir && max_size.is_some() {
                skip_filesize(
                    max_size.unwrap(),
                    dent.path(),
                    &dent.metadata().ok(),
                )
            } else {
                false
            };

        if !should_skip_path && !should_skip_filesize {
            self.tx.send(Message::Work(Work {
                dent: dent,
                ignore: ig.clone(),
                root_device: root_device,
            }));
        }
        WalkState::Continue
    }

    /// Returns the next directory to descend into.
    ///
    /// If all work has been exhausted, then this returns None. The worker
    /// should then subsequently quit.
    fn get_work(&mut self) -> Option<Work> {
        loop {
            if self.is_quit_now() {
                return None;
            }
            match self.rx.try_recv() {
                Some(Message::Work(work)) => {
                    self.waiting(false);
                    self.quitting(false);
                    return Some(work);
                }
                Some(Message::Quit) => {
                    // We can't just quit because a Message::Quit could be
                    // spurious. For example, it's possible to observe that
                    // all workers are waiting even if there's more work to
                    // be done.
                    //
                    // Therefore, we do a bit of a dance to wait until all
                    // workers have signaled that they're ready to quit before
                    // actually quitting.
                    //
                    // If the Quit message turns out to be spurious, then the
                    // loop below will break and we'll go back to looking for
                    // more work.
                    self.waiting(true);
                    self.quitting(true);
                    while !self.is_quit_now() {
                        let nwait = self.num_waiting();
                        let nquit = self.num_quitting();
                        // If the number of waiting workers dropped, then
                        // abort our attempt to quit.
                        if nwait < self.threads {
                            break;
                        }
                        // If all workers are in this quit loop, then we
                        // can stop.
                        if nquit == self.threads {
                            return None;
                        }
                        // Otherwise, spin.
                    }
                }
                None => {
                    self.waiting(true);
                    self.quitting(false);
                    if self.num_waiting() == self.threads {
                        for _ in 0..self.threads {
                            self.tx.send(Message::Quit);
                        }
                    } else {
                        // You're right to consider this suspicious, but it's
                        // a useful heuristic to permit producers to catch up
                        // to consumers without burning the CPU. It is also
                        // useful as a means to prevent burning the CPU if only
                        // one worker is left doing actual work. It's not
                        // perfect and it doesn't leave the CPU completely
                        // idle, but it's not clear what else we can do. :-/
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            }
        }
    }

    /// Indicates that all workers should quit immediately.
    fn quit_now(&self) {
        self.quit_now.store(true, Ordering::SeqCst);
    }

    /// Returns true if this worker should quit immediately.
    fn is_quit_now(&self) -> bool {
        self.quit_now.load(Ordering::SeqCst)
    }

    /// Returns the total number of workers waiting for work.
    fn num_waiting(&self) -> usize {
        self.num_waiting.load(Ordering::SeqCst)
    }

    /// Returns the total number of workers ready to quit.
    fn num_quitting(&self) -> usize {
        self.num_quitting.load(Ordering::SeqCst)
    }

    /// Sets this worker's "quitting" state to the value of `yes`.
    fn quitting(&mut self, yes: bool) {
        if yes {
            if !self.is_quitting {
                self.is_quitting = true;
                self.num_quitting.fetch_add(1, Ordering::SeqCst);
            }
        } else {
            if self.is_quitting {
                self.is_quitting = false;
                self.num_quitting.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    /// Sets this worker's "waiting" state to the value of `yes`.
    fn waiting(&mut self, yes: bool) {
        if yes {
            if !self.is_waiting {
                self.is_waiting = true;
                self.num_waiting.fetch_add(1, Ordering::SeqCst);
            }
        } else {
            if self.is_waiting {
                self.is_waiting = false;
                self.num_waiting.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }
}

fn check_symlink_loop(
    ig_parent: &Ignore,
    child_path: &Path,
    child_depth: usize,
) -> Result<(), Error> {
    let hchild = Handle::from_path(child_path).map_err(|err| {
        Error::from(err).with_path(child_path).with_depth(child_depth)
    })?;
    for ig in ig_parent.parents().take_while(|ig| !ig.is_absolute_parent()) {
        let h = Handle::from_path(ig.path()).map_err(|err| {
            Error::from(err).with_path(child_path).with_depth(child_depth)
        })?;
        if hchild == h {
            return Err(Error::Loop {
                ancestor: ig.path().to_path_buf(),
                child: child_path.to_path_buf(),
            }.with_depth(child_depth));
        }
    }
    Ok(())
}

// Before calling this function, make sure that you ensure that is really
// necessary as the arguments imply a file stat.
fn skip_filesize(
    max_filesize: u64,
    path: &Path,
    ent: &Option<Metadata>
) -> bool {
    let filesize = match *ent {
        Some(ref md) => Some(md.len()),
        None => None
    };

    if let Some(fs) = filesize {
        if fs > max_filesize {
            debug!("ignoring {}: {} bytes", path.display(), fs);
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn skip_path(
    ig: &Ignore,
    path: &Path,
    is_dir: bool,
) -> bool {
    let m = ig.matched(path, is_dir);
    if m.is_ignore() {
        debug!("ignoring {}: {:?}", path.display(), m);
        true
    } else if m.is_whitelist() {
        debug!("whitelisting {}: {:?}", path.display(), m);
        false
    } else {
        false
    }
}

/// Returns a handle to stdout for filtering search.
///
/// A handle is returned if and only if stdout is being redirected to a file.
/// The handle returned corresponds to that file.
///
/// This can be used to ensure that we do not attempt to search a file that we
/// may also be writing to.
fn stdout_handle() -> Option<Handle> {
    let h = match Handle::stdout() {
        Err(_) => return None,
        Ok(h) => h,
    };
    let md = match h.as_file().metadata() {
        Err(_) => return None,
        Ok(md) => md,
    };
    if !md.is_file() {
        return None;
    }
    Some(h)
}

/// Returns true if and only if the given directory entry is believed to be
/// equivalent to the given handle. If there was a problem querying the path
/// for information to determine equality, then that error is returned.
fn path_equals(dent: &DirEntry, handle: &Handle) -> Result<bool, Error> {
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
    if dent.is_stdin() || never_equal(dent, handle) {
        return Ok(false);
    }
    Handle::from_path(dent.path())
        .map(|h| &h == handle)
        .map_err(|err| Error::Io(err).with_path(dent.path()))
}

/// Returns true if and only if the given path is on the same device as the
/// given root device.
fn is_same_file_system(root_device: u64, path: &Path) -> Result<bool, Error> {
    let dent_device = device_num(path)
        .map_err(|err| Error::Io(err).with_path(path))?;
    Ok(root_device == dent_device)
}

#[cfg(unix)]
fn device_num<P: AsRef<Path>>(path: P)-> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;

    path.as_ref().metadata().map(|md| md.dev())
}

 #[cfg(windows)]
fn device_num<P: AsRef<Path>>(path: P) -> io::Result<u64> {
    use winapi_util::{Handle, file};

    let h = Handle::from_path_any(path)?;
    file::information(h).map(|info| info.volume_serial_number())
}

#[cfg(not(any(unix, windows)))]
fn device_num<P: AsRef<Path>>(_: P)-> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        "walkdir: same_file_system option not supported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use tempdir::TempDir;

    use super::{DirEntry, WalkBuilder, WalkState, device_num};

    fn wfile<P: AsRef<Path>>(path: P, contents: &str) {
        let mut file = File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    fn wfile_size<P: AsRef<Path>>(path: P, size: u64) {
        let file = File::create(path).unwrap();
        file.set_len(size).unwrap();
    }

    #[cfg(unix)]
    fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) {
        use std::os::unix::fs::symlink;
        symlink(src, dst).unwrap();
    }

    fn mkdirp<P: AsRef<Path>>(path: P) {
        fs::create_dir_all(path).unwrap();
    }

    fn normal_path(unix: &str) -> String {
        if cfg!(windows) {
            unix.replace("\\", "/")
        } else {
            unix.to_string()
        }
    }

    fn walk_collect(prefix: &Path, builder: &WalkBuilder) -> Vec<String> {
        let mut paths = vec![];
        for result in builder.build() {
            let dent = match result {
                Err(_) => continue,
                Ok(dent) => dent,
            };
            let path = dent.path().strip_prefix(prefix).unwrap();
            if path.as_os_str().is_empty() {
                continue;
            }
            paths.push(normal_path(path.to_str().unwrap()));
        }
        paths.sort();
        paths
    }

    fn walk_collect_parallel(
        prefix: &Path,
        builder: &WalkBuilder,
    ) -> Vec<String> {
        let mut paths = vec![];
        for dent in walk_collect_entries_parallel(builder) {
            let path = dent.path().strip_prefix(prefix).unwrap();
            if path.as_os_str().is_empty() {
                continue;
            }
            paths.push(normal_path(path.to_str().unwrap()));
        }
        paths.sort();
        paths
    }

    fn walk_collect_entries_parallel(builder: &WalkBuilder) -> Vec<DirEntry> {
        let dents = Arc::new(Mutex::new(vec![]));
        builder.build_parallel().run(|| {
            let dents = dents.clone();
            Box::new(move |result| {
                if let Ok(dent) = result {
                    dents.lock().unwrap().push(dent);
                }
                WalkState::Continue
            })
        });

        let dents = dents.lock().unwrap();
        dents.to_vec()
    }

    fn mkpaths(paths: &[&str]) -> Vec<String> {
        let mut paths: Vec<_> = paths.iter().map(|s| s.to_string()).collect();
        paths.sort();
        paths
    }

    fn assert_paths(
        prefix: &Path,
        builder: &WalkBuilder,
        expected: &[&str],
    ) {
        let got = walk_collect(prefix, builder);
        assert_eq!(got, mkpaths(expected), "single threaded");
        let got = walk_collect_parallel(prefix, builder);
        assert_eq!(got, mkpaths(expected), "parallel");
    }

    #[test]
    fn no_ignores() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a/b/c"));
        mkdirp(td.path().join("x/y"));
        wfile(td.path().join("a/b/foo"), "");
        wfile(td.path().join("x/y/foo"), "");

        assert_paths(td.path(), &WalkBuilder::new(td.path()), &[
            "x", "x/y", "x/y/foo", "a", "a/b", "a/b/foo", "a/b/c",
        ]);
    }

    #[test]
    fn custom_ignore() {
        let td = TempDir::new("walk-test-").unwrap();
        let custom_ignore = ".customignore";
        mkdirp(td.path().join("a"));
        wfile(td.path().join(custom_ignore), "foo");
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("bar"), "");
        wfile(td.path().join("a/bar"), "");

        let mut builder = WalkBuilder::new(td.path());
        builder.add_custom_ignore_filename(&custom_ignore);
        assert_paths(td.path(), &builder, &["bar", "a", "a/bar"]);
    }

    #[test]
    fn custom_ignore_exclusive_use() {
        let td = TempDir::new("walk-test-").unwrap();
        let custom_ignore = ".customignore";
        mkdirp(td.path().join("a"));
        wfile(td.path().join(custom_ignore), "foo");
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("bar"), "");
        wfile(td.path().join("a/bar"), "");

        let mut builder = WalkBuilder::new(td.path());
        builder.ignore(false);
        builder.git_ignore(false);
        builder.git_global(false);
        builder.git_exclude(false);
        builder.add_custom_ignore_filename(&custom_ignore);
        assert_paths(td.path(), &builder, &["bar", "a", "a/bar"]);
    }

    #[test]
    fn gitignore() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join(".git"));
        mkdirp(td.path().join("a"));
        wfile(td.path().join(".gitignore"), "foo");
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("bar"), "");
        wfile(td.path().join("a/bar"), "");

        assert_paths(td.path(), &WalkBuilder::new(td.path()), &[
            "bar", "a", "a/bar",
        ]);
    }

    #[test]
    fn explicit_ignore() {
        let td = TempDir::new("walk-test-").unwrap();
        let igpath = td.path().join(".not-an-ignore");
        mkdirp(td.path().join("a"));
        wfile(&igpath, "foo");
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("bar"), "");
        wfile(td.path().join("a/bar"), "");

        let mut builder = WalkBuilder::new(td.path());
        assert!(builder.add_ignore(&igpath).is_none());
        assert_paths(td.path(), &builder, &["bar", "a", "a/bar"]);
    }

    #[test]
    fn explicit_ignore_exclusive_use() {
        let td = TempDir::new("walk-test-").unwrap();
        let igpath = td.path().join(".not-an-ignore");
        mkdirp(td.path().join("a"));
        wfile(&igpath, "foo");
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("bar"), "");
        wfile(td.path().join("a/bar"), "");

        let mut builder = WalkBuilder::new(td.path());
        builder.standard_filters(false);
        assert!(builder.add_ignore(&igpath).is_none());
        assert_paths(td.path(), &builder,
            &[".not-an-ignore", "bar", "a", "a/bar"]);
    }

    #[test]
    fn gitignore_parent() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join(".git"));
        mkdirp(td.path().join("a"));
        wfile(td.path().join(".gitignore"), "foo");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("a/bar"), "");

        let root = td.path().join("a");
        assert_paths(&root, &WalkBuilder::new(&root), &["bar"]);
    }

    #[test]
    fn max_depth() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a/b/c"));
        wfile(td.path().join("foo"), "");
        wfile(td.path().join("a/foo"), "");
        wfile(td.path().join("a/b/foo"), "");
        wfile(td.path().join("a/b/c/foo"), "");

        let mut builder = WalkBuilder::new(td.path());
        assert_paths(td.path(), &builder, &[
            "a", "a/b", "a/b/c", "foo", "a/foo", "a/b/foo", "a/b/c/foo",
        ]);
        assert_paths(td.path(), builder.max_depth(Some(0)), &[]);
        assert_paths(td.path(), builder.max_depth(Some(1)), &["a", "foo"]);
        assert_paths(td.path(), builder.max_depth(Some(2)), &[
            "a", "a/b", "foo", "a/foo",
        ]);
    }

    #[test]
    fn max_filesize() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a/b"));
        wfile_size(td.path().join("foo"), 0);
        wfile_size(td.path().join("bar"), 400);
        wfile_size(td.path().join("baz"), 600);
        wfile_size(td.path().join("a/foo"), 600);
        wfile_size(td.path().join("a/bar"), 500);
        wfile_size(td.path().join("a/baz"), 200);

        let mut builder = WalkBuilder::new(td.path());
        assert_paths(td.path(), &builder, &[
            "a", "a/b", "foo", "bar", "baz", "a/foo", "a/bar", "a/baz",
        ]);
        assert_paths(td.path(), builder.max_filesize(Some(0)), &[
            "a", "a/b", "foo"
        ]);
        assert_paths(td.path(), builder.max_filesize(Some(500)), &[
            "a", "a/b", "foo", "bar", "a/bar", "a/baz"
        ]);
        assert_paths(td.path(), builder.max_filesize(Some(50000)), &[
            "a", "a/b", "foo", "bar", "baz", "a/foo", "a/bar", "a/baz",
        ]);
    }

    #[cfg(unix)] // because symlinks on windows are weird
    #[test]
    fn symlinks() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a/b"));
        symlink(td.path().join("a/b"), td.path().join("z"));
        wfile(td.path().join("a/b/foo"), "");

        let mut builder = WalkBuilder::new(td.path());
        assert_paths(td.path(), &builder, &[
            "a", "a/b", "a/b/foo", "z",
        ]);
        assert_paths(td.path(), &builder.follow_links(true), &[
            "a", "a/b", "a/b/foo", "z", "z/foo",
        ]);
    }

    #[cfg(unix)] // because symlinks on windows are weird
    #[test]
    fn first_path_not_symlink() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("foo"));

        let dents = WalkBuilder::new(td.path().join("foo"))
            .build()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(1, dents.len());
        assert!(!dents[0].path_is_symlink());

        let dents = walk_collect_entries_parallel(
            &WalkBuilder::new(td.path().join("foo")),
        );
        assert_eq!(1, dents.len());
        assert!(!dents[0].path_is_symlink());
    }

    #[cfg(unix)] // because symlinks on windows are weird
    #[test]
    fn symlink_loop() {
        let td = TempDir::new("walk-test-").unwrap();
        mkdirp(td.path().join("a/b"));
        symlink(td.path().join("a"), td.path().join("a/b/c"));

        let mut builder = WalkBuilder::new(td.path());
        assert_paths(td.path(), &builder, &[
            "a", "a/b", "a/b/c",
        ]);
        assert_paths(td.path(), &builder.follow_links(true), &[
            "a", "a/b",
        ]);
    }

    // It's a little tricky to test the 'same_file_system' option since
    // we need an environment with more than one file system. We adopt a
    // heuristic where /sys is typically a distinct volume on Linux and roll
    // with that.
    #[test]
    #[cfg(target_os = "linux")]
    fn same_file_system() {
        // If for some reason /sys doesn't exist or isn't a directory, just
        // skip this test.
        if !Path::new("/sys").is_dir() {
            return;
        }

        // If our test directory actually isn't a different volume from /sys,
        // then this test is meaningless and we shouldn't run it.
        let td = TempDir::new("walk-test-").unwrap();
        if device_num(td.path()).unwrap() == device_num("/sys").unwrap() {
            return;
        }

        mkdirp(td.path().join("same_file"));
        symlink("/sys", td.path().join("same_file").join("alink"));

        // Create a symlink to sys and enable following symlinks. If the
        // same_file_system option doesn't work, then this probably will hit a
        // permission error. Otherwise, it should just skip over the symlink
        // completely.
        let mut builder = WalkBuilder::new(td.path());
        builder.follow_links(true).same_file_system(true);
        assert_paths(td.path(), &builder, &[
            "same_file", "same_file/alink",
        ]);
    }
}
