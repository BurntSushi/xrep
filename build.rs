#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

use std::env;
use std::fs;
use std::process;

use clap::Shell;

#[allow(dead_code)]
#[path = "src/app.rs"]
mod app;

fn main() {
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

    // Use clap to build completion files.
    let mut app = app::app();
    app.gen_completions("rg", Shell::Bash, &outdir);
    app.gen_completions("rg", Shell::Fish, &outdir);
    app.gen_completions("rg", Shell::PowerShell, &outdir);
    // Note that we do not use clap's support for zsh. Instead, zsh completions
    // are manually maintained in `complete/_rg`.

    // Make the current git hash available to the build.
    let result = process::Command::new("git")
        .args(&["rev-parse", "--short=10", "HEAD"])
        .output();
    if let Ok(output) = result {
        let hash = String::from_utf8_lossy(&output.stdout);
        println!("cargo:rustc-env=RIPGREP_BUILD_GIT_HASH={}", hash);
    }
}
