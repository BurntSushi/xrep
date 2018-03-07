extern crate atty;
extern crate bytecount;
#[macro_use]
extern crate clap;
extern crate encoding_rs;
extern crate globset;
extern crate grep;
extern crate ignore;
#[macro_use]
extern crate lazy_static;
extern crate libc;
#[macro_use]
extern crate log;
extern crate memchr;
extern crate memmap;
extern crate num_cpus;
extern crate regex;
extern crate same_file;
extern crate termcolor;
#[cfg(windows)]
extern crate winapi;

use std::error::Error;
use std::process;
use std::result;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

use args::Args;
use worker::Work;

macro_rules! errored {
    ($($tt:tt)*) => {
        return Err(From::from(format!($($tt)*)));
    }
}

mod app;
mod args;
mod config;
mod decoder;
mod decompressor;
mod logger;
mod pathutil;
mod printer;
mod search_buffer;
mod search_stream;
mod unescape;
mod worker;

pub type Result<T> = result::Result<T, Box<Error>>;

fn main() {
    reset_sigpipe();
    match Args::parse().map(Arc::new).and_then(run) {
        Ok(0) => process::exit(1),
        Ok(_) => process::exit(0),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}

fn run(args: Arc<Args>) -> Result<u64> {
    if args.never_match() {
        return Ok(0);
    }
    let threads = args.threads();
    if args.files() {
        if threads == 1 || args.is_one_path() {
            run_files_one_thread(&args)
        } else {
            run_files_parallel(args)
        }
    } else if args.type_list() {
        run_types(&args)
    } else if threads == 1 || args.is_one_path() {
        run_one_thread(&args)
    } else {
        run_parallel(&args)
    }
}

fn run_parallel(args: &Arc<Args>) -> Result<u64> {
    let bufwtr = Arc::new(args.buffer_writer());
    let quiet_matched = args.quiet_matched();
    let paths_searched = Arc::new(AtomicUsize::new(0));
    let match_line_count = Arc::new(AtomicUsize::new(0));

    args.walker_parallel().run(|| {
        let args = Arc::clone(args);
        let quiet_matched = quiet_matched.clone();
        let paths_searched = paths_searched.clone();
        let match_line_count = match_line_count.clone();
        let bufwtr = Arc::clone(&bufwtr);
        let mut buf = bufwtr.buffer();
        let mut worker = args.worker();
        Box::new(move |result| {
            use ignore::WalkState::*;

            if quiet_matched.has_match() {
                return Quit;
            }
            let dent = match get_or_log_dir_entry(
                result,
                args.stdout_handle(),
                args.files(),
                args.no_messages(),
            ) {
                None => return Continue,
                Some(dent) => dent,
            };
            paths_searched.fetch_add(1, Ordering::SeqCst);
            buf.clear();
            {
                // This block actually executes the search and prints the
                // results into outbuf.
                let mut printer = args.printer(&mut buf);
                let count =
                    if dent.is_stdin() {
                        worker.run(&mut printer, Work::Stdin)
                    } else {
                        worker.run(&mut printer, Work::DirEntry(dent))
                    };
                match_line_count.fetch_add(count as usize, Ordering::SeqCst);
                if quiet_matched.set_match(count > 0) {
                    return Quit;
                }
            }
            // BUG(burntsushi): We should handle this error instead of ignoring
            // it. See: https://github.com/BurntSushi/ripgrep/issues/200
            let _ = bufwtr.print(&buf);
            Continue
        })
    });
    if !args.paths().is_empty() && paths_searched.load(Ordering::SeqCst) == 0 {
        if !args.no_messages() {
            eprint_nothing_searched();
        }
    }
    Ok(match_line_count.load(Ordering::SeqCst) as u64)
}

fn run_one_thread(args: &Arc<Args>) -> Result<u64> {
    let stdout = args.stdout();
    let mut stdout = stdout.lock();
    let mut worker = args.worker();
    let mut paths_searched: u64 = 0;
    let mut match_line_count = 0;
    for result in args.walker() {
        let dent = match get_or_log_dir_entry(
            result,
            args.stdout_handle(),
            args.files(),
            args.no_messages(),
        ) {
            None => continue,
            Some(dent) => dent,
        };
        let mut printer = args.printer(&mut stdout);
        if match_line_count > 0 {
            if args.quiet() {
                break;
            }
            if let Some(sep) = args.file_separator() {
                printer = printer.file_separator(sep);
            }
        }
        paths_searched += 1;
        match_line_count +=
            if dent.is_stdin() {
                worker.run(&mut printer, Work::Stdin)
            } else {
                worker.run(&mut printer, Work::DirEntry(dent))
            };
    }
    if !args.paths().is_empty() && paths_searched == 0 {
        if !args.no_messages() {
            eprint_nothing_searched();
        }
    }
    Ok(match_line_count)
}

fn run_files_parallel(args: Arc<Args>) -> Result<u64> {
    let print_args = Arc::clone(&args);
    let (tx, rx) = mpsc::channel::<ignore::DirEntry>();
    let print_thread = thread::spawn(move || {
        let stdout = print_args.stdout();
        let mut printer = print_args.printer(stdout.lock());
        let mut file_count = 0;
        for dent in rx.iter() {
            if !print_args.quiet() {
                printer.path(dent.path());
            }
            file_count += 1;
        }
        file_count
    });
    args.walker_parallel().run(move || {
        let args = Arc::clone(&args);
        let tx = tx.clone();
        Box::new(move |result| {
            if let Some(dent) = get_or_log_dir_entry(
                result,
                args.stdout_handle(),
                args.files(),
                args.no_messages(),
            ) {
                tx.send(dent).unwrap();
            }
            ignore::WalkState::Continue
        })
    });
    Ok(print_thread.join().unwrap())
}

fn run_files_one_thread(args: &Arc<Args>) -> Result<u64> {
    let stdout = args.stdout();
    let mut printer = args.printer(stdout.lock());
    let mut file_count = 0;
    for result in args.walker() {
        let dent = match get_or_log_dir_entry(
            result,
            args.stdout_handle(),
            args.files(),
            args.no_messages(),
        ) {
            None => continue,
            Some(dent) => dent,
        };
        if !args.quiet() {
            printer.path(dent.path());
        }
        file_count += 1;
    }
    Ok(file_count)
}

fn run_types(args: &Arc<Args>) -> Result<u64> {
    let stdout = args.stdout();
    let mut printer = args.printer(stdout.lock());
    let mut ty_count = 0;
    for def in args.type_defs() {
        printer.type_def(def);
        ty_count += 1;
    }
    Ok(ty_count)
}

fn get_or_log_dir_entry(
    result: result::Result<ignore::DirEntry, ignore::Error>,
    stdout_handle: Option<&same_file::Handle>,
    files_only: bool,
    no_messages: bool,
) -> Option<ignore::DirEntry> {
    match result {
        Err(err) => {
            if !no_messages {
                eprintln!("{}", err);
            }
            None
        }
        Ok(dent) => {
            if let Some(err) = dent.error() {
                if !no_messages {
                    eprintln!("{}", err);
                }
            }
            if dent.file_type().is_none() {
                return Some(dent); // entry is stdin
            }
            // A depth of 0 means the user gave the path explicitly, so we
            // should always try to search it.
            if dent.depth() == 0 && !ignore_entry_is_dir(&dent) {
                return Some(dent);
            } else if !ignore_entry_is_file(&dent) {
                return None;
            }
            // If we are redirecting stdout to a file, then don't search that
            // file.
            if !files_only && is_stdout_file(&dent, stdout_handle, no_messages) {
                return None;
            }
            Some(dent)
        }
    }
}

/// Returns true if and only if the given `ignore::DirEntry` points to a
/// directory.
///
/// This works around a bug in Rust's standard library:
/// https://github.com/rust-lang/rust/issues/46484
#[cfg(windows)]
fn ignore_entry_is_dir(dent: &ignore::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    use winapi::um::winnt::FILE_ATTRIBUTE_DIRECTORY;

    dent.metadata().map(|md| {
        md.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0
    }).unwrap_or(false)
}

/// Returns true if and only if the given `ignore::DirEntry` points to a
/// directory.
#[cfg(not(windows))]
fn ignore_entry_is_dir(dent: &ignore::DirEntry) -> bool {
    dent.file_type().map_or(false, |ft| ft.is_dir())
}

/// Returns true if and only if the given `ignore::DirEntry` points to a
/// file.
///
/// This works around a bug in Rust's standard library:
/// https://github.com/rust-lang/rust/issues/46484
#[cfg(windows)]
fn ignore_entry_is_file(dent: &ignore::DirEntry) -> bool {
    !ignore_entry_is_dir(dent)
}

/// Returns true if and only if the given `ignore::DirEntry` points to a
/// file.
#[cfg(not(windows))]
fn ignore_entry_is_file(dent: &ignore::DirEntry) -> bool {
    dent.file_type().map_or(false, |ft| ft.is_file())
}

fn is_stdout_file(
    dent: &ignore::DirEntry,
    stdout_handle: Option<&same_file::Handle>,
    no_messages: bool,
) -> bool {
    let stdout_handle = match stdout_handle {
        None => return false,
        Some(stdout_handle) => stdout_handle,
    };
    // If we know for sure that these two things aren't equal, then avoid
    // the costly extra stat call to determine equality.
    if !maybe_dent_eq_handle(dent, stdout_handle) {
        return false;
    }
    match same_file::Handle::from_path(dent.path()) {
        Ok(h) => stdout_handle == &h,
        Err(err) => {
            if !no_messages {
                eprintln!("{}: {}", dent.path().display(), err);
            }
            false
        }
    }
}

#[cfg(unix)]
fn maybe_dent_eq_handle(
    dent: &ignore::DirEntry,
    handle: &same_file::Handle,
) -> bool {
    dent.ino() == Some(handle.ino())
}

#[cfg(not(unix))]
fn maybe_dent_eq_handle(_: &ignore::DirEntry, _: &same_file::Handle) -> bool {
    true
}

fn eprint_nothing_searched() {
    eprintln!("No files were searched, which means ripgrep probably \
               applied a filter you didn't expect. \
               Try running again with --debug.");
}

// The Rust standard library suppresses the default SIGPIPE behavior, so that
// writing to a closed pipe doesn't kill the process. The goal is to instead
// handle errors through the normal result mechanism. Ripgrep needs some
// refactoring before it will be able to do that, however, so we re-enable the
// standard SIGPIPE behavior as a workaround. See
// https://github.com/BurntSushi/ripgrep/issues/200.
#[cfg(unix)]
fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {
    // no-op
}
