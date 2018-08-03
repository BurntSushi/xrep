extern crate crossbeam;
extern crate ignore;
extern crate walkdir;

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use crossbeam::sync::MsQueue;
use ignore::WalkBuilder;
use walkdir::WalkDir;

fn main() {
    let mut path = env::args().nth(1).unwrap();
    let mut parallel = false;
    let mut simple = false;
    let queue: Arc<MsQueue<Option<DirEntry>>> = Arc::new(MsQueue::new());
    if path == "parallel" {
        path = env::args().nth(2).unwrap();
        parallel = true;
    } else if path == "walkdir" {
        path = env::args().nth(2).unwrap();
        simple = true;
    }

    let stdout_queue = queue.clone();
    let stdout_thread = thread::spawn(move || {
        let mut stdout = io::BufWriter::new(io::stdout());
        while let Some(dent) = stdout_queue.pop() {
            write_path(&mut stdout, dent.path());
        }
    });

    if parallel {
        let walker = WalkBuilder::new(path).threads(6).build_parallel();
        walker.run(|| {
            let queue = queue.clone();
            Box::new(move |result| {
                use ignore::WalkState::*;

                queue.push(Some(DirEntry::Y(result.unwrap())));
                Continue
            })
        });
    } else if simple {
        let walker = WalkDir::new(path);
        for result in walker {
            queue.push(Some(DirEntry::X(result.unwrap())));
        }
    } else {
        let walker = WalkBuilder::new(path).build();
        for result in walker {
            queue.push(Some(DirEntry::Y(result.unwrap())));
        }
    }
    queue.push(None);
    stdout_thread.join().unwrap();
}

enum DirEntry {
    X(walkdir::DirEntry),
    Y(ignore::DirEntry),
}

impl DirEntry {
    fn path(&self) -> &Path {
        match *self {
            DirEntry::X(ref x) => x.path(),
            DirEntry::Y(ref y) => y.path(),
        }
    }
}

#[cfg(unix)]
fn write_path<W: Write>(mut wtr: W, path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    wtr.write(path.as_os_str().as_bytes()).unwrap();
    wtr.write(b"\n").unwrap();
}

#[cfg(not(unix))]
fn write_path<W: Write>(mut wtr: W, path: &Path) {
    wtr.write(path.to_string_lossy().as_bytes()).unwrap();
    wtr.write(b"\n").unwrap();
}
