extern crate atty;
extern crate bytecount;
#[macro_use]
extern crate clap;
extern crate encoding_rs;
extern crate env_logger;
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

macro_rules! eprintln {
    ($($tt:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(&mut ::std::io::stderr(), $($tt)*);
    }}
}

mod app;
mod args;
mod decoder;
mod pathutil;
mod printer;
mod search_buffer;
mod search_stream;
mod unescape;
mod worker;

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

fn main() {
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
            run_files_one_thread(args)
        } else {
            run_files_parallel(args)
        }
    } else if args.type_list() {
        run_types(args)
    } else if threads == 1 || args.is_one_path() {
        run_one_thread(args)
    } else {
        run_parallel(args)
    }
}

fn run_parallel(args: Arc<Args>) -> Result<u64> {
    let bufwtr = Arc::new(args.buffer_writer());
    let quiet_matched = args.quiet_matched();
    let paths_searched = Arc::new(AtomicUsize::new(0));
    let match_count = Arc::new(AtomicUsize::new(0));

    args.walker_parallel().run(|| {
        let args = args.clone();
        let quiet_matched = quiet_matched.clone();
        let paths_searched = paths_searched.clone();
        let match_count = match_count.clone();
        let bufwtr = bufwtr.clone();
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
                match_count.fetch_add(count as usize, Ordering::SeqCst);
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
    Ok(match_count.load(Ordering::SeqCst) as u64)
}

fn run_one_thread(args: Arc<Args>) -> Result<u64> {
    let stdout = args.stdout();
    let mut stdout = stdout.lock();
    let mut worker = args.worker();
    let mut paths_searched: u64 = 0;
    let mut match_count = 0;
    for result in args.walker() {
        let dent = match get_or_log_dir_entry(
            result,
            args.stdout_handle(),
            args.no_messages(),
        ) {
            None => continue,
            Some(dent) => dent,
        };
        let mut printer = args.printer(&mut stdout);
        if match_count > 0 {
            if args.quiet() {
                break;
            }
            if let Some(sep) = args.file_separator() {
                printer = printer.file_separator(sep);
            }
        }
        paths_searched += 1;
        match_count +=
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
    Ok(match_count)
}

fn run_files_parallel(args: Arc<Args>) -> Result<u64> {
    let print_args = args.clone();
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
        let args = args.clone();
        let tx = tx.clone();
        Box::new(move |result| {
            if let Some(dent) = get_or_log_dir_entry(
                result,
                args.stdout_handle(),
                args.no_messages(),
            ) {
                tx.send(dent).unwrap();
            }
            ignore::WalkState::Continue
        })
    });
    Ok(print_thread.join().unwrap())
}

fn run_files_one_thread(args: Arc<Args>) -> Result<u64> {
    let stdout = args.stdout();
    let mut printer = args.printer(stdout.lock());
    let mut file_count = 0;
    for result in args.walker() {
        let dent = match get_or_log_dir_entry(
            result,
            args.stdout_handle(),
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

fn run_types(args: Arc<Args>) -> Result<u64> {
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
            let ft = match dent.file_type() {
                None => return Some(dent), // entry is stdin
                Some(ft) => ft,
            };
            // A depth of 0 means the user gave the path explicitly, so we
            // should always try to search it.
            if dent.depth() == 0 && !ft.is_dir() {
                return Some(dent);
            } else if !ft.is_file() {
                return None;
            }
            // If we are redirecting stdout to a file, then don't search that
            // file.
            if is_stdout_file(&dent, stdout_handle, no_messages) {
                return None;
            }
            Some(dent)
        }
    }
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
