grep-searcher
-------------
A high level library for executing fast line oriented searches. This handles
things like reporting contextual lines, counting lines, inverting a search,
detecting binary data, automatic UTF-16 transcoding and deciding whether or not
to use memory maps.

[![Linux build status](https://api.travis-ci.org/BurntSushi/ripgrep.svg)](https://travis-ci.org/BurntSushi/ripgrep)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/BurntSushi/ripgrep?svg=true)](https://ci.appveyor.com/project/BurntSushi/ripgrep)
[![](https://img.shields.io/crates/v/grep-searcher.svg)](https://crates.io/crates/grep-searcher)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).

### Documentation

[https://docs.rs/grep-searcher](https://docs.rs/grep-searcher)

**NOTE:** You probably don't want to use this crate directly. Instead, you
should prefer the facade defined in the
[`grep`](https://docs.rs/grep)
crate.


### Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
grep-searcher = "0.1"
```

and this to your crate root:

```rust
extern crate grep_searcher;
```
