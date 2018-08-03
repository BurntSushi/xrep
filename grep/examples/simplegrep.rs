extern crate atty;
extern crate grep;
extern crate termcolor;
extern crate walkdir;

use std::env;
use std::error;
use std::ffi::OsString;
use std::path::Path;
use std::process;
use std::result;

use grep::printer::{ColorSpecs, StandardBuilder};
use grep::regex::RegexMatcher;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use termcolor::{ColorChoice, StandardStream};
use walkdir::WalkDir;

macro_rules! fail {
    ($($tt:tt)*) => {
        return Err(From::from(format!($($tt)*)));
    }
}

type Result<T> = result::Result<T, Box<error::Error>>;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{}", err);
        process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let mut args: Vec<OsString> = env::args_os().collect();
    if args.len() < 2 {
        fail!("Usage: simplegrep <pattern> [<path> ...]");
    }
    if args.len() == 2 {
        args.push(OsString::from("./"));
    }
    let pattern = match args[1].clone().into_string() {
        Ok(pattern) => pattern,
        Err(_) => {
            fail!(
                "pattern is not valid UTF-8: '{:?}'",
                args[1].to_string_lossy()
            );
        }
    };
    search(&pattern, &args[2..])
}

fn search(pattern: &str, paths: &[OsString]) -> Result<()> {
    let matcher = RegexMatcher::new_line_matcher(&pattern)?;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();
    let mut printer = StandardBuilder::new()
        .color_specs(colors())
        .build(StandardStream::stdout(color_choice()));

    for path in paths {
        for result in WalkDir::new(path) {
            let dent = match result {
                Ok(dent) => dent,
                Err(err) => {
                    eprintln!(
                        "{}: {}",
                        err.path().unwrap_or(Path::new("error")).display(),
                        err,
                    );
                    continue;
                }
            };
            if !dent.file_type().is_file() {
                continue;
            }
            let result = searcher.search_path(
                &matcher,
                dent.path(),
                printer.sink_with_path(&matcher, dent.path()),
            );
            if let Err(err) = result {
                eprintln!("{}: {}", dent.path().display(), err);
            }
        }
    }
    Ok(())
}

fn color_choice() -> ColorChoice {
    if atty::is(atty::Stream::Stdout) {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    }
}

fn colors() -> ColorSpecs {
    ColorSpecs::new(&[
        "path:fg:magenta".parse().unwrap(),
        "line:fg:green".parse().unwrap(),
        "match:fg:red".parse().unwrap(),
        "match:style:bold".parse().unwrap(),
    ])
}
