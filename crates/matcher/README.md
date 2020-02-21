grep-matcher
------------
This crate provides a low level interface for describing regular expression
matchers. The `grep` crate uses this interface in order to make the regex
engine it uses pluggable.

[![Linux build status](https://api.travis-ci.org/BurntSushi/ripgrep.svg)](https://travis-ci.org/BurntSushi/ripgrep)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/BurntSushi/ripgrep?svg=true)](https://ci.appveyor.com/project/BurntSushi/ripgrep)
[![](https://img.shields.io/crates/v/grep-matcher.svg)](https://crates.io/crates/grep-matcher)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).

### Documentation

[https://docs.rs/grep-matcher](https://docs.rs/grep-matcher)

**NOTE:** You probably don't want to use this crate directly. Instead, you
should prefer the facade defined in the
[`grep`](https://docs.rs/grep)
crate.


### Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
grep-matcher = "0.1"
```

and this to your crate root:

```rust
extern crate grep_matcher;
```
