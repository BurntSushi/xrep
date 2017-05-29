#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

use std::env;
use std::fs;

use clap::Shell;

#[allow(dead_code)]
#[path = "src/app.rs"]
mod app;

fn main() {
    let outdir = match env::var_os("OUT_DIR") {
        None => return,
        Some(outdir) => outdir,
    };
    fs::create_dir_all(&outdir).unwrap();

    let mut app = app::app();
    app.gen_completions("rg", Shell::Bash, &outdir);
    app.gen_completions("rg", Shell::Fish, &outdir);
    app.gen_completions("rg", Shell::PowerShell, &outdir);
}
