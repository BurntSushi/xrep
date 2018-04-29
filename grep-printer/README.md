grep-printer
------------
Print results from line oriented searching in a human readable, aggregate or
JSON Lines format.

[![Linux build status](https://api.travis-ci.org/BurntSushi/ripgrep.svg)](https://travis-ci.org/BurntSushi/ripgrep)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/BurntSushi/ripgrep?svg=true)](https://ci.appveyor.com/project/BurntSushi/ripgrep)
[![](https://img.shields.io/crates/v/grep-printer.svg)](https://crates.io/crates/grep-printer)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).

### Documentation

[https://docs.rs/grep-printer](https://docs.rs/grep-printer)

**NOTE:** You probably don't want to use this crate directly. Instead, you
should prefer the facade defined in the
[`grep`](https://docs.rs/grep)
crate.


### Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
grep-printer = "0.1"
```

and this to your crate root:

```rust
extern crate grep_printer;
```
