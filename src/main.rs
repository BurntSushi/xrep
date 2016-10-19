extern crate ctrlc;
extern crate deque;
extern crate docopt;
extern crate env_logger;
extern crate globset;
extern crate grep;
#[cfg(windows)]
extern crate kernel32;
#[macro_use]
extern crate lazy_static;
extern crate libc;
#[macro_use]
extern crate log;
extern crate memchr;
extern crate memmap;
extern crate num_cpus;
extern crate regex;
extern crate rustc_serialize;
extern crate term;
extern crate thread_local;
extern crate walkdir;
#[cfg(windows)]
extern crate winapi;

use std::error::Error;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process;
use std::result;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::cmp;

use deque::{Stealer, Stolen};
use grep::Grep;
use memmap::{Mmap, Protection};
use term::Terminal;
use walkdir::DirEntry;

use args::Args;
use out::{ColoredTerminal, Out};
use pathutil::strip_prefix;
use printer::Printer;
use search_stream::InputBuffer;
#[cfg(windows)]
use terminal_win::WindowsBuffer;

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

mod args;
mod atty;
mod gitignore;
mod ignore;
mod out;
mod pathutil;
mod printer;
mod search_buffer;
mod search_stream;
#[cfg(windows)]
mod terminal_win;
mod types;
mod walk;

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

fn main() {
    match Args::parse().and_then(run) {
        Ok(count) if count == 0 => process::exit(1),
        Ok(_) => process::exit(0),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}

fn run(args: Args) -> Result<u64> {
    let args = Arc::new(args);

    let handler_args = args.clone();
    ctrlc::set_handler(move || {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        let _ = handler_args.stdout().reset();
        let _ = stdout.flush();

        process::exit(1);
    });

    let paths = args.paths();
    let threads = cmp::max(1, args.threads() - 1);
    let isone =
        paths.len() == 1 && (paths[0] == Path::new("-") || paths[0].is_file());
    if args.files() {
        return run_files(args.clone());
    }
    if args.type_list() {
        return run_types(args.clone());
    }
    if threads == 1 || isone {
        return run_one_thread(args.clone());
    }

    let out = Arc::new(Mutex::new(args.out()));
    let quiet_matched = QuietMatched::new(args.quiet());
    let mut workers = vec![];

    let workq = {
        let (workq, stealer) = deque::new();
        for _ in 0..threads {
            let worker = MultiWorker {
                chan_work: stealer.clone(),
                quiet_matched: quiet_matched.clone(),
                out: out.clone(),
                outbuf: Some(args.outbuf()),
                worker: Worker {
                    args: args.clone(),
                    inpbuf: args.input_buffer(),
                    grep: args.grep(),
                    match_count: 0,
                },
            };
            workers.push(thread::spawn(move || worker.run()));
        }
        workq
    };
    let mut paths_searched: u64 = 0;
    for p in paths {
        if quiet_matched.has_match() {
            break;
        }
        if p == Path::new("-") {
            paths_searched += 1;
            workq.push(Work::Stdin);
        } else {
            for ent in try!(args.walker(p)) {
                if quiet_matched.has_match() {
                    break;
                }
                paths_searched += 1;
                workq.push(Work::File(ent));
            }
        }
    }
    if !paths.is_empty() && paths_searched == 0 {
        eprintln!("No files were searched, which means ripgrep probably \
                   applied a filter you didn't expect. \
                   Try running again with --debug.");
    }
    for _ in 0..workers.len() {
        workq.push(Work::Quit);
    }
    let mut match_count = 0;
    for worker in workers {
        match_count += worker.join().unwrap();
    }
    Ok(match_count)
}

fn run_one_thread(args: Arc<Args>) -> Result<u64> {
    let mut worker = Worker {
        args: args.clone(),
        inpbuf: args.input_buffer(),
        grep: args.grep(),
        match_count: 0,
    };
    let paths = args.paths();
    let mut term = args.stdout();

    let mut paths_searched: u64 = 0;
    for p in paths {
        if args.quiet() && worker.match_count > 0 {
            break;
        }
        if p == Path::new("-") {
            paths_searched += 1;
            let mut printer = args.printer(&mut term);
            if worker.match_count > 0 {
                if let Some(sep) = args.file_separator() {
                    printer = printer.file_separator(sep);
                }
            }
            worker.do_work(&mut printer, WorkReady::Stdin);
        } else {
            for ent in try!(args.walker(p)) {
                paths_searched += 1;
                let mut printer = args.printer(&mut term);
                if worker.match_count > 0 {
                    if args.quiet() {
                        break;
                    }
                    if let Some(sep) = args.file_separator() {
                        printer = printer.file_separator(sep);
                    }
                }
                let file = match File::open(ent.path()) {
                    Ok(file) => file,
                    Err(err) => {
                        eprintln!("{}: {}", ent.path().display(), err);
                        continue;
                    }
                };
                worker.do_work(&mut printer, WorkReady::DirFile(ent, file));
            }
        }
    }
    if !paths.is_empty() && paths_searched == 0 {
        eprintln!("No files were searched, which means ripgrep probably \
                   applied a filter you didn't expect. \
                   Try running again with --debug.");
    }
    Ok(worker.match_count)
}

fn run_files(args: Arc<Args>) -> Result<u64> {
    let term = args.stdout();
    let mut printer = args.printer(term);
    let mut file_count = 0;
    for p in args.paths() {
        if p == Path::new("-") {
            printer.path(&Path::new("<stdin>"));
            file_count += 1;
        } else {
            for ent in try!(args.walker(p)) {
                printer.path(ent.path());
                file_count += 1;
            }
        }
    }
    Ok(file_count)
}

fn run_types(args: Arc<Args>) -> Result<u64> {
    let term = args.stdout();
    let mut printer = args.printer(term);
    let mut ty_count = 0;
    for def in args.type_defs() {
        printer.type_def(def);
        ty_count += 1;
    }
    Ok(ty_count)
}

enum Work {
    Stdin,
    File(DirEntry),
    Quit,
}

enum WorkReady {
    Stdin,
    DirFile(DirEntry, File),
}

struct MultiWorker {
    chan_work: Stealer<Work>,
    quiet_matched: QuietMatched,
    out: Arc<Mutex<Out>>,
    #[cfg(not(windows))]
    outbuf: Option<ColoredTerminal<term::TerminfoTerminal<Vec<u8>>>>,
    #[cfg(windows)]
    outbuf: Option<ColoredTerminal<WindowsBuffer>>,
    worker: Worker,
}

struct Worker {
    args: Arc<Args>,
    inpbuf: InputBuffer,
    grep: Grep,
    match_count: u64,
}

impl MultiWorker {
    fn run(mut self) -> u64 {
        loop {
            if self.quiet_matched.has_match() {
                break;
            }
            let work = match self.chan_work.steal() {
                Stolen::Empty | Stolen::Abort => continue,
                Stolen::Data(Work::Quit) => break,
                Stolen::Data(Work::Stdin) => WorkReady::Stdin,
                Stolen::Data(Work::File(ent)) => {
                    match File::open(ent.path()) {
                        Ok(file) => WorkReady::DirFile(ent, file),
                        Err(err) => {
                            eprintln!("{}: {}", ent.path().display(), err);
                            continue;
                        }
                    }
                }
            };
            let mut outbuf = self.outbuf.take().unwrap();
            outbuf.clear();
            let mut printer = self.worker.args.printer(outbuf);
            self.worker.do_work(&mut printer, work);
            if self.quiet_matched.set_match(self.worker.match_count > 0) {
                break;
            }
            let outbuf = printer.into_inner();
            if !outbuf.get_ref().is_empty() {
                let mut out = self.out.lock().unwrap();
                out.write(&outbuf);
            }
            self.outbuf = Some(outbuf);
        }
        self.worker.match_count
    }
}

impl Worker {
    fn do_work<W: Terminal + Send>(
        &mut self,
        printer: &mut Printer<W>,
        work: WorkReady,
    ) {
        let result = match work {
            WorkReady::Stdin => {
                let stdin = io::stdin();
                let stdin = stdin.lock();
                self.search(printer, &Path::new("<stdin>"), stdin)
            }
            WorkReady::DirFile(ent, file) => {
                let mut path = ent.path();
                if let Some(p) = strip_prefix("./", path) {
                    path = p;
                }
                if self.args.mmap() {
                    self.search_mmap(printer, path, &file)
                } else {
                    self.search(printer, path, file)
                }
            }
        };
        match result {
            Ok(count) => {
                self.match_count += count;
            }
            Err(err) => {
                eprintln!("{}", err);
            }
        }
    }

    fn search<R: io::Read, W: Terminal + Send>(
        &mut self,
        printer: &mut Printer<W>,
        path: &Path,
        rdr: R,
    ) -> Result<u64> {
        self.args.searcher(
            &mut self.inpbuf,
            printer,
            &self.grep,
            path,
            rdr,
        ).run().map_err(From::from)
    }

    fn search_mmap<W: Terminal + Send>(
        &mut self,
        printer: &mut Printer<W>,
        path: &Path,
        file: &File,
    ) -> Result<u64> {
        if try!(file.metadata()).len() == 0 {
            // Opening a memory map with an empty file results in an error.
            // However, this may not actually be an empty file! For example,
            // /proc/cpuinfo reports itself as an empty file, but it can
            // produce data when it's read from. Therefore, we fall back to
            // regular read calls.
            return self.search(printer, path, file);
        }
        let mmap = try!(Mmap::open(file, Protection::Read));
        Ok(self.args.searcher_buffer(
            printer,
            &self.grep,
            path,
            unsafe { mmap.as_slice() },
        ).run())
    }
}

#[derive(Clone, Debug)]
struct QuietMatched(Arc<Option<AtomicBool>>);

impl QuietMatched {
    fn new(quiet: bool) -> QuietMatched {
        let atomic = if quiet { Some(AtomicBool::new(false)) } else { None };
        QuietMatched(Arc::new(atomic))
    }

    fn has_match(&self) -> bool {
        match *self.0 {
            None => false,
            Some(ref matched) => matched.load(Ordering::SeqCst),
        }
    }

    fn set_match(&self, yes: bool) -> bool {
        match *self.0 {
            None => false,
            Some(_) if !yes => false,
            Some(ref m) => { m.store(true, Ordering::SeqCst); true }
        }
    }
}
