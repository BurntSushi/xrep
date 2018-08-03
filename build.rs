#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

use clap::Shell;

use app::{RGArg, RGArgKind};

#[allow(dead_code)]
#[path = "src/app.rs"]
mod app;

fn main() {
    // If our version of Rust has runtime SIMD detection, then set a cfg so
    // we know we can test for it. We use this when generating ripgrep's
    // --version output.
    let version = rustc_version();
    let parsed = match Version::parse(&version) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("failed to parse `rustc --version`: {}", err);
            return;
        }
    };
    let minimum = Version { major: 1, minor: 27, patch: 0 };
    if version.contains("nightly") || parsed >= minimum {
        println!("cargo:rustc-cfg=ripgrep_runtime_cpu");
    }

    // OUT_DIR is set by Cargo and it's where any additional build artifacts
    // are written.
    let outdir = match env::var_os("OUT_DIR") {
        Some(outdir) => outdir,
        None => {
            eprintln!(
                "OUT_DIR environment variable not defined. \
                 Please file a bug: \
                 https://github.com/BurntSushi/ripgrep/issues/new");
            process::exit(1);
        }
    };
    fs::create_dir_all(&outdir).unwrap();

    let stamp_path = Path::new(&outdir).join("ripgrep-stamp");
    if let Err(err) = File::create(&stamp_path) {
        panic!("failed to write {}: {}", stamp_path.display(), err);
    }
    if let Err(err) = generate_man_page(&outdir) {
        eprintln!("failed to generate man page: {}", err);
    }

    // Use clap to build completion files.
    let mut app = app::app();
    app.gen_completions("rg", Shell::Bash, &outdir);
    app.gen_completions("rg", Shell::Fish, &outdir);
    app.gen_completions("rg", Shell::PowerShell, &outdir);
    // Note that we do not use clap's support for zsh. Instead, zsh completions
    // are manually maintained in `complete/_rg`.

    // Make the current git hash available to the build.
    if let Some(rev) = git_revision_hash() {
        println!("cargo:rustc-env=RIPGREP_BUILD_GIT_HASH={}", rev);
    }
}

fn git_revision_hash() -> Option<String> {
    let result = process::Command::new("git")
        .args(&["rev-parse", "--short=10", "HEAD"])
        .output();
    result.ok().and_then(|output| {
        let v = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    })
}

fn generate_man_page<P: AsRef<Path>>(outdir: P) -> io::Result<()> {
    // If asciidoc isn't installed, then don't do anything.
    if let Err(err) = process::Command::new("a2x").output() {
        eprintln!("Could not run 'a2x' binary, skipping man page generation.");
        eprintln!("Error from running 'a2x': {}", err);
        return Ok(());
    }
    // 1. Read asciidoc template.
    // 2. Interpolate template with auto-generated docs.
    // 3. Save interpolation to disk.
    // 4. Use a2x (part of asciidoc) to convert to man page.
    let outdir = outdir.as_ref();
    let cwd = env::current_dir()?;
    let tpl_path = cwd.join("doc").join("rg.1.txt.tpl");
    let txt_path = outdir.join("rg.1.txt");

    let mut tpl = String::new();
    File::open(&tpl_path)?.read_to_string(&mut tpl)?;
    tpl = tpl.replace("{OPTIONS}", &formatted_options()?);

    let githash = git_revision_hash();
    let githash = githash.as_ref().map(|x| &**x);
    tpl = tpl.replace("{VERSION}", &app::long_version(githash));

    File::create(&txt_path)?.write_all(tpl.as_bytes())?;
    let result = process::Command::new("a2x")
        .arg("--no-xmllint")
        .arg("--doctype").arg("manpage")
        .arg("--format").arg("manpage")
        .arg(&txt_path)
        .spawn()?
        .wait()?;
    if !result.success() {
        let msg = format!("'a2x' failed with exit code {:?}", result.code());
        return Err(ioerr(msg));
    }
    Ok(())
}

fn formatted_options() -> io::Result<String> {
    let mut args = app::all_args_and_flags();
    args.sort_by(|x1, x2| x1.name.cmp(&x2.name));

    let mut formatted = vec![];
    for arg in args {
        if arg.hidden {
            continue;
        }
        // ripgrep only has two positional arguments, and probably will only
        // ever have two positional arguments, so we just hardcode them into
        // the template.
        if let app::RGArgKind::Positional{..} = arg.kind {
            continue;
        }
        formatted.push(formatted_arg(&arg)?);
    }
    Ok(formatted.join("\n\n"))
}

fn formatted_arg(arg: &RGArg) -> io::Result<String> {
    match arg.kind {
        RGArgKind::Positional{..} => panic!("unexpected positional argument"),
        RGArgKind::Switch { long, short, multiple } => {
            let mut out = vec![];

            let mut header = format!("--{}", long);
            if let Some(short) = short {
                header = format!("-{}, {}", short, header);
            }
            if multiple {
                header = format!("*{}* ...::", header);
            } else {
                header = format!("*{}*::", header);
            }
            writeln!(out, "{}", header)?;
            writeln!(out, "{}", formatted_doc_txt(arg)?)?;

            Ok(String::from_utf8(out).unwrap())
        }
        RGArgKind::Flag { long, short, value_name, multiple, .. } => {
            let mut out = vec![];

            let mut header = format!("--{}", long);
            if let Some(short) = short {
                header = format!("-{}, {}", short, header);
            }
            if multiple {
                header = format!("*{}* _{}_ ...::", header, value_name);
            } else {
                header = format!("*{}* _{}_::", header, value_name);
            }
            writeln!(out, "{}", header)?;
            writeln!(out, "{}", formatted_doc_txt(arg)?)?;

            Ok(String::from_utf8(out).unwrap())
        }
    }
}

fn formatted_doc_txt(arg: &RGArg) -> io::Result<String> {
    let paragraphs: Vec<&str> = arg.doc_long.split("\n\n").collect();
    if paragraphs.is_empty() {
        return Err(ioerr(format!("missing docs for --{}", arg.name)));
    }
    let first = format!("  {}", paragraphs[0].replace("\n", "\n  "));
    if paragraphs.len() == 1 {
        return Ok(first);
    }
    Ok(format!("{}\n+\n{}", first, paragraphs[1..].join("\n+\n")))
}

fn ioerr(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}

fn rustc_version() -> String {
    let rustc = env::var_os("RUSTC").unwrap_or(OsString::from("rustc"));
    let output = process::Command::new(&rustc)
        .arg("--version")
        .output()
        .unwrap()
        .stdout;
    String::from_utf8(output).unwrap()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    fn parse(mut s: &str) -> Result<Version, String> {
        if !s.starts_with("rustc ") {
            return Err(format!("unrecognized version string: {}", s));
        }
        s = &s["rustc ".len()..];

        let parts: Vec<&str> = s.split(".").collect();
        if parts.len() < 3 {
            return Err(format!("not enough version parts: {:?}", parts));
        }

        let mut num = String::new();
        for c in parts[0].chars() {
            if !c.is_digit(10) {
                break;
            }
            num.push(c);
        }
        let major = num.parse::<u32>().map_err(|e| e.to_string())?;

        num.clear();
        for c in parts[1].chars() {
            if !c.is_digit(10) {
                break;
            }
            num.push(c);
        }
        let minor = num.parse::<u32>().map_err(|e| e.to_string())?;

        num.clear();
        for c in parts[2].chars() {
            if !c.is_digit(10) {
                break;
            }
            num.push(c);
        }
        let patch = num.parse::<u32>().map_err(|e| e.to_string())?;

        Ok(Version { major, minor, patch })
    }
}
