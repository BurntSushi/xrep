extern crate atty;
#[macro_use]
extern crate clap;
extern crate globset;
extern crate grep;
extern crate ignore;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate num_cpus;
extern crate regex;
extern crate same_file;
#[macro_use]
extern crate serde_json;
extern crate termcolor;
#[cfg(windows)]
extern crate winapi;

use std::io;
use std::process;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ignore::WalkState;

use args::Args;
use subject::Subject;

#[macro_use]
mod messages;

mod app;
mod args;
mod config;
mod decompressor;
mod preprocessor;
mod logger;
mod path_printer;
mod search;
mod subject;
mod unescape;

pub type Result<T> = ::std::result::Result<T, Box<::std::error::Error>>;

pub fn main() {
    match Args::parse().and_then(run) {
        Ok(true) => process::exit(0),
        Ok(false) => process::exit(1),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(2);
        }
    }
}

fn run(args: Args) -> Result<bool> {
    use args::Command::*;

    match args.command()? {
        Search => search(args),
        SearchParallel => search_parallel(args),
        SearchNever => Ok(false),
        Files => files(args),
        FilesParallel => files_parallel(args),
        Types => types(args),
    }
}

/// The top-level entry point for single-threaded search. This recursively
/// steps through the file list (current directory by default) and searches
/// each file sequentially.
fn search(args: Args) -> Result<bool> {
    let started_at = Instant::now();
    let quit_after_match = args.quit_after_match()?;
    let subject_builder = args.subject_builder();
    let mut stats = args.stats()?;
    let mut searcher = args.search_worker(args.stdout())?;
    let mut matched = false;

    for result in args.walker()? {
        let subject = match subject_builder.build_from_result(result) {
            Some(subject) => subject,
            None => continue,
        };
        let search_result = match searcher.search(&subject) {
            Ok(search_result) => search_result,
            Err(err) => {
                // A broken pipe means graceful termination.
                if err.kind() == io::ErrorKind::BrokenPipe {
                    break;
                }
                message!("{}: {}", subject.path().display(), err);
                continue;
            }
        };
        matched = matched || search_result.has_match();
        if let Some(ref mut stats) = stats {
            *stats += search_result.stats().unwrap();
        }
        if matched && quit_after_match {
            break;
        }
    }
    if let Some(ref stats) = stats {
        let elapsed = Instant::now().duration_since(started_at);
        // We don't care if we couldn't print this successfully.
        let _ = searcher.print_stats(elapsed, stats);
    }
    Ok(matched)
}

/// The top-level entry point for multi-threaded search. The parallelism is
/// itself achieved by the recursive directory traversal. All we need to do is
/// feed it a worker for performing a search on each file.
fn search_parallel(args: Args) -> Result<bool> {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering::SeqCst;

    let quit_after_match = args.quit_after_match()?;
    let started_at = Instant::now();
    let subject_builder = Arc::new(args.subject_builder());
    let bufwtr = Arc::new(args.buffer_writer()?);
    let stats = Arc::new(args.stats()?.map(Mutex::new));
    let matched = Arc::new(AtomicBool::new(false));
    let mut searcher_err = None;
    args.walker_parallel()?.run(|| {
        let args = args.clone();
        let bufwtr = Arc::clone(&bufwtr);
        let stats = Arc::clone(&stats);
        let matched = Arc::clone(&matched);
        let subject_builder = Arc::clone(&subject_builder);
        let mut searcher = match args.search_worker(bufwtr.buffer()) {
            Ok(searcher) => searcher,
            Err(err) => {
                searcher_err = Some(err);
                return Box::new(move |_| {
                    WalkState::Quit
                });
            }
        };

        Box::new(move |result| {
            let subject = match subject_builder.build_from_result(result) {
                Some(subject) => subject,
                None => return WalkState::Continue,
            };
            searcher.printer().get_mut().clear();
            let search_result = match searcher.search(&subject) {
                Ok(search_result) => search_result,
                Err(err) => {
                    message!("{}: {}", subject.path().display(), err);
                    return WalkState::Continue;
                }
            };
            if search_result.has_match() {
                matched.store(true, SeqCst);
            }
            if let Some(ref locked_stats) = *stats {
                let mut stats = locked_stats.lock().unwrap();
                *stats += search_result.stats().unwrap();
            }
            if let Err(err) = bufwtr.print(searcher.printer().get_mut()) {
                // A broken pipe means graceful termination.
                if err.kind() == io::ErrorKind::BrokenPipe {
                    return WalkState::Quit;
                }
                // Otherwise, we continue on our merry way.
                message!("{}: {}", subject.path().display(), err);
            }
            if matched.load(SeqCst) && quit_after_match {
                WalkState::Quit
            } else {
                WalkState::Continue
            }
        })
    });
    if let Some(err) = searcher_err.take() {
        return Err(err);
    }
    if let Some(ref locked_stats) = *stats {
        let elapsed = Instant::now().duration_since(started_at);
        let stats = locked_stats.lock().unwrap();
        let mut searcher = args.search_worker(args.stdout())?;
        // We don't care if we couldn't print this successfully.
        let _ = searcher.print_stats(elapsed, &stats);
    }
    Ok(matched.load(SeqCst))
}

/// The top-level entry point for listing files without searching them. This
/// recursively steps through the file list (current directory by default) and
/// prints each path sequentially using a single thread.
fn files(args: Args) -> Result<bool> {
    let quit_after_match = args.quit_after_match()?;
    let subject_builder = args.subject_builder();
    let mut matched = false;
    let mut path_printer = args.path_printer(args.stdout())?;
    for result in args.walker()? {
        let subject = match subject_builder.build_from_result(result) {
            Some(subject) => subject,
            None => continue,
        };
        matched = true;
        if quit_after_match {
            break;
        }
        if let Err(err) = path_printer.write_path(subject.path()) {
            // A broken pipe means graceful termination.
            if err.kind() == io::ErrorKind::BrokenPipe {
                break;
            }
            // Otherwise, we have some other error that's preventing us from
            // writing to stdout, so we should bubble it up.
            return Err(err.into());
        }
    }
    Ok(matched)
}

/// The top-level entry point for listing files without searching them. This
/// recursively steps through the file list (current directory by default) and
/// prints each path sequentially using multiple threads.
fn files_parallel(args: Args) -> Result<bool> {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering::SeqCst;
    use std::sync::mpsc;
    use std::thread;

    let quit_after_match = args.quit_after_match()?;
    let subject_builder = Arc::new(args.subject_builder());
    let mut path_printer = args.path_printer(args.stdout())?;
    let matched = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<Subject>();

    let print_thread = thread::spawn(move || -> io::Result<()> {
        for subject in rx.iter() {
            path_printer.write_path(subject.path())?;
        }
        Ok(())
    });
    args.walker_parallel()?.run(|| {
        let subject_builder = Arc::clone(&subject_builder);
        let matched = Arc::clone(&matched);
        let tx = tx.clone();

        Box::new(move |result| {
            let subject = match subject_builder.build_from_result(result) {
                Some(subject) => subject,
                None => return WalkState::Continue,
            };
            matched.store(true, SeqCst);
            if quit_after_match {
                WalkState::Quit
            } else {
                match tx.send(subject) {
                    Ok(_) => WalkState::Continue,
                    Err(_) => WalkState::Quit,
                }
            }
        })
    });
    drop(tx);
    if let Err(err) = print_thread.join().unwrap() {
        // A broken pipe means graceful termination, so fall through.
        // Otherwise, something bad happened while writing to stdout, so bubble
        // it up.
        if err.kind() != io::ErrorKind::BrokenPipe {
            return Err(err.into());
        }
    }
    Ok(matched.load(SeqCst))
}

/// The top-level entry point for --type-list.
fn types(args: Args) -> Result<bool> {
    let mut count = 0;
    let mut stdout = args.stdout();
    for def in args.type_defs()? {
        count += 1;
        stdout.write_all(def.name().as_bytes())?;
        stdout.write_all(b": ")?;

        let mut first = true;
        for glob in def.globs() {
            if !first {
                stdout.write_all(b", ")?;
            }
            stdout.write_all(glob.as_bytes())?;
            first = false;
        }
        stdout.write_all(b"\n")?;
    }
    Ok(count > 0)
}
