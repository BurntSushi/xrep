ripgrep (rg)
------------
ripgrep is a line-oriented search tool that recursively searches your current
directory for a regex pattern while respecting your gitignore rules. ripgrep
has first class support on Windows, macOS and Linux, with binary downloads
available for [every release](https://github.com/BurntSushi/ripgrep/releases).
ripgrep is similar to other popular search tools like The Silver Searcher,
ack and grep.

[![Linux build status](https://travis-ci.org/BurntSushi/ripgrep.svg?branch=master)](https://travis-ci.org/BurntSushi/ripgrep)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/BurntSushi/ripgrep?svg=true)](https://ci.appveyor.com/project/BurntSushi/ripgrep)
[![Crates.io](https://img.shields.io/crates/v/ripgrep.svg)](https://crates.io/crates/ripgrep)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).


### CHANGELOG

Please see the [CHANGELOG](CHANGELOG.md) for a release history.

### Documentation quick links

* [Installation](#installation)
* [User Guide](GUIDE.md)
* [Frequently Asked Questions](FAQ.md)
* [Regex syntax](https://docs.rs/regex/0.2.5/regex/#syntax)
* [Configuration files](GUIDE.md#configuration-file)
* [Shell completions](FAQ.md#complete)
* [Building](#building)


### Screenshot of search results

[![A screenshot of a sample search with ripgrep](http://burntsushi.net/stuff/ripgrep1.png)](http://burntsushi.net/stuff/ripgrep1.png)


### Quick examples comparing tools

This example searches the entire Linux kernel source tree (after running
`make defconfig && make -j8`) for `[A-Z]+_SUSPEND`, where all matches must be
words. Timings were collected on a system with an Intel i7-6900K 3.2 GHz, and
ripgrep was compiled with SIMD enabled.

Please remember that a single benchmark is never enough! See my
[blog post on ripgrep](http://blog.burntsushi.net/ripgrep/)
for a very detailed comparison with more benchmarks and analysis.

| Tool | Command | Line count | Time |
| ---- | ------- | ---------- | ---- |
| ripgrep (Unicode) | `rg -n -w '[A-Z]+_SUSPEND'` | 450 | **0.106s** |
| [git grep](https://www.kernel.org/pub/software/scm/git/docs/git-grep.html) | `LC_ALL=C git grep -E -n -w '[A-Z]+_SUSPEND'` | 450 | 0.553s |
| [The Silver Searcher](https://github.com/ggreer/the_silver_searcher) | `ag -w '[A-Z]+_SUSPEND'` | 450 | 0.589s |
| [git grep (Unicode)](https://www.kernel.org/pub/software/scm/git/docs/git-grep.html) | `LC_ALL=en_US.UTF-8 git grep -E -n -w '[A-Z]+_SUSPEND'` | 450 | 2.266s |
| [sift](https://github.com/svent/sift) | `sift --git -n -w '[A-Z]+_SUSPEND'` | 450 | 3.505s |
| [ack](https://github.com/petdance/ack2) | `ack -w '[A-Z]+_SUSPEND'` | 1878 | 6.823s |
| [The Platinum Searcher](https://github.com/monochromegane/the_platinum_searcher) | `pt -w -e '[A-Z]+_SUSPEND'` | 450 | 14.208s |

(Yes, `ack` [has](https://github.com/petdance/ack2/issues/445) a
[bug](https://github.com/petdance/ack2/issues/14).)

Here's another benchmark that disregards gitignore files and searches with a
whitelist instead. The corpus is the same as in the previous benchmark, and the
flags passed to each command ensure that they are doing equivalent work:

| Tool | Command | Line count | Time |
| ---- | ------- | ---------- | ---- |
| ripgrep | `rg -L -u -tc -n -w '[A-Z]+_SUSPEND'` | 404 | **0.079s** |
| [ucg](https://github.com/gvansickle/ucg) | `ucg --type=cc -w '[A-Z]+_SUSPEND'` | 390 | 0.163s |
| [GNU grep](https://www.gnu.org/software/grep/) | `egrep -R -n --include='*.c' --include='*.h' -w '[A-Z]+_SUSPEND'` | 404 | 0.611s |

(`ucg` [has slightly different behavior in the presence of symbolic links](https://github.com/gvansickle/ucg/issues/106).)

And finally, a straight-up comparison between ripgrep and GNU grep on a single
large file (~9.3GB,
[`OpenSubtitles2016.raw.en.gz`](http://opus.lingfil.uu.se/OpenSubtitles2016/mono/OpenSubtitles2016.raw.en.gz)):

| Tool | Command | Line count | Time |
| ---- | ------- | ---------- | ---- |
| ripgrep | `rg -w 'Sherlock [A-Z]\w+'` | 5268 | **2.108s** |
| [GNU grep](https://www.gnu.org/software/grep/) | `LC_ALL=C egrep -w 'Sherlock [A-Z]\w+'` | 5268 | 7.014s |

In the above benchmark, passing the `-n` flag (for showing line numbers)
increases the times to `2.640s` for ripgrep and `10.277s` for GNU grep.


### Why should I use ripgrep?

* It can replace many use cases served by both The Silver Searcher and GNU grep
  because it is generally faster than both. (See [the FAQ](FAQ.md#posix4ever)
  for more details on whether ripgrep can truly replace grep.)
* Like The Silver Searcher, ripgrep defaults to recursive directory search
  and won't search files ignored by your `.gitignore` files. It also ignores
  hidden and binary files by default. ripgrep also implements full support
  for `.gitignore`, whereas there are many bugs related to that functionality
  in The Silver Searcher.
* ripgrep can search specific types of files. For example, `rg -tpy foo`
  limits your search to Python files and `rg -Tjs foo` excludes Javascript
  files from your search. ripgrep can be taught about new file types with
  custom matching rules.
* ripgrep supports many features found in `grep`, such as showing the context
  of search results, searching multiple patterns, highlighting matches with
  color and full Unicode support. Unlike GNU grep, ripgrep stays fast while
  supporting Unicode (which is always on).
* ripgrep supports searching files in text encodings other than UTF-8, such
  as UTF-16, latin-1, GBK, EUC-JP, Shift_JIS and more. (Some support for
  automatically detecting UTF-16 is provided. Other text encodings must be
  specifically specified with the `-E/--encoding` flag.)
* ripgrep supports searching files compressed in a common format (gzip, xz,
  lzma, bzip2 or lz4) with the `-z/--search-zip` flag.
* ripgrep supports arbitrary input preprocessing filters which could be PDF
  text extraction, less supported decompression, decrypting, automatic encoding
  detection and so on.

In other words, use ripgrep if you like speed, filtering by default, fewer
bugs, and Unicode support.


### Why shouldn't I use ripgrep?

I'd like to try to convince you why you *shouldn't* use ripgrep. This should
give you a glimpse at some important downsides or missing features of
ripgrep.

* ripgrep uses a regex engine based on finite automata, so if you want fancy
  regex features such as backreferences or lookaround, ripgrep won't provide
  them to you. ripgrep does support lots of things though, including, but not
  limited to: lazy quantification (e.g., `a+?`), repetitions (e.g., `a{2,5}`),
  begin/end assertions (e.g., `^\w+$`), word boundaries (e.g., `\bfoo\b`), and
  support for Unicode categories (e.g., `\p{Sc}` to match currency symbols or
  `\p{Lu}` to match any uppercase letter). (Fancier regexes will never be
  supported.)
* ripgrep doesn't have multiline search. (Will happen as an opt-in feature.)

In other words, if you like fancy regexes or multiline search, then ripgrep
may not quite meet your needs (yet).


### Is it really faster than everything else?

Generally, yes. A large number of benchmarks with detailed analysis for each is
[available on my blog](http://blog.burntsushi.net/ripgrep/).

Summarizing, ripgrep is fast because:

* It is built on top of
  [Rust's regex engine](https://github.com/rust-lang-nursery/regex).
  Rust's regex engine uses finite automata, SIMD and aggressive literal
  optimizations to make searching very fast.
* Rust's regex library maintains performance with full Unicode support by
  building UTF-8 decoding directly into its deterministic finite automaton
  engine.
* It supports searching with either memory maps or by searching incrementally
  with an intermediate buffer. The former is better for single files and the
  latter is better for large directories. ripgrep chooses the best searching
  strategy for you automatically.
* Applies your ignore patterns in `.gitignore` files using a
  [`RegexSet`](https://docs.rs/regex/1.0.0/regex/struct.RegexSet.html).
  That means a single file path can be matched against multiple glob patterns
  simultaneously.
* It uses a lock-free parallel recursive directory iterator, courtesy of
  [`crossbeam`](https://docs.rs/crossbeam) and
  [`ignore`](https://docs.rs/ignore).


### Feature comparison

Andy Lester, author of [ack](https://beyondgrep.com/), has published an
excellent table comparing the features of ack, ag, git-grep, GNU grep and
ripgrep: https://beyondgrep.com/feature-comparison/


### Installation

The binary name for ripgrep is `rg`.

**[Archives of precompiled binaries for ripgrep are available for Windows,
macOS and Linux.](https://github.com/BurntSushi/ripgrep/releases)** Users of
platforms not explicitly mentioned below are advised to download one of these
archives.

Linux binaries are static executables. Windows binaries are available either as
built with MinGW (GNU) or with Microsoft Visual C++ (MSVC). When possible,
prefer MSVC over GNU, but you'll need to have the [Microsoft VC++ 2015
redistributable](https://www.microsoft.com/en-us/download/details.aspx?id=48145)
installed.

If you're a **macOS Homebrew** or a **Linuxbrew** user,
then you can install ripgrep either
from homebrew-core, (compiled with rust stable, no SIMD):

```
$ brew install ripgrep
```

or you can install a binary compiled with rust nightly (including SIMD and all
optimizations) by utilizing a custom tap:

```
$ brew tap burntsushi/ripgrep https://github.com/BurntSushi/ripgrep.git
$ brew install ripgrep-bin
```

If you're a **MacPorts** user, then you can install ripgrep from the
[official ports](https://www.macports.org/ports.php?by=name&substr=ripgrep):

```
$ sudo port install ripgrep
```

If you're a **Windows Chocolatey** user, then you can install ripgrep from the [official repo](https://chocolatey.org/packages/ripgrep):

```
$ choco install ripgrep
```

If you're a **Windows Scoop** user, then you can install ripgrep from the [official bucket](https://github.com/lukesampson/scoop/blob/master/bucket/ripgrep.json):

```
$ scoop install ripgrep
```

If you're an **Arch Linux** user, then you can install ripgrep from the official repos:

```
$ pacman -S ripgrep
```

If you're a **Gentoo** user, you can install ripgrep from the [official repo](https://packages.gentoo.org/packages/sys-apps/ripgrep):

```
$ emerge sys-apps/ripgrep
```

If you're a **Fedora 27+** user, you can install ripgrep from official repositories.

```
$ sudo dnf install ripgrep
```

If you're a **Fedora 24+** user, you can install ripgrep from [copr](https://copr.fedorainfracloud.org/coprs/carlwgeorge/ripgrep/):

```
$ sudo dnf copr enable carlwgeorge/ripgrep
$ sudo dnf install ripgrep
```

If you're an **openSUSE Tumbleweed** user, you can install ripgrep from the [official repo](http://software.opensuse.org/package/ripgrep):

```
$ sudo zypper install ripgrep
```

If you're a **RHEL/CentOS 7** user, you can install ripgrep from [copr](https://copr.fedorainfracloud.org/coprs/carlwgeorge/ripgrep/):

```
$ sudo yum-config-manager --add-repo=https://copr.fedorainfracloud.org/coprs/carlwgeorge/ripgrep/repo/epel-7/carlwgeorge-ripgrep-epel-7.repo
$ sudo yum install ripgrep
```

If you're a **Nix** user, you can install ripgrep from
[nixpkgs](https://github.com/NixOS/nixpkgs/blob/master/pkgs/tools/text/ripgrep/default.nix):

```
$ nix-env --install ripgrep
$ # (Or using the attribute name, which is also ripgrep.)
```

If you're a **Debian** user (or a user of a Debian derivative like **Ubuntu**),
then ripgrep can be installed using a binary `.deb` file provided in each
[ripgrep release](https://github.com/BurntSushi/ripgrep/releases). Note that
ripgrep is not in the official Debian or Ubuntu repositories.

```
$ curl -LO https://github.com/BurntSushi/ripgrep/releases/download/0.9.0/ripgrep_0.9.0_amd64.deb
$ sudo dpkg -i ripgrep_0.9.0_amd64.deb
```

If you run Debian Buster (currently Debian testing) or Debian sid, ripgrep is 
[officially maintained by Debian](https://tracker.debian.org/pkg/rust-ripgrep).
```
$ sudo apt-get install ripgrep
```

(N.B. Various snaps for ripgrep on Ubuntu are also available, but none of them
seem to work right and generate a number of very strange bug reports that I
don't know how to fix and don't have the time to fix. Therefore, it is no
longer a recommended installation option.)

If you're a **FreeBSD** user, then you can install ripgrep from the [official ports](https://www.freshports.org/textproc/ripgrep/):

```
# pkg install ripgrep
```

If you're an **OpenBSD** user, then you can install ripgrep from the [official ports](http://openports.se/textproc/ripgrep):

```
$ doas pkg_add ripgrep
```

If you're a **NetBSD** user, then you can install ripgrep from [pkgsrc](http://pkgsrc.se/textproc/ripgrep):

```
# pkgin install ripgrep
```

If you're a **Rust programmer**, ripgrep can be installed with `cargo`.
* Note that the minimum supported version of Rust for ripgrep is **1.23.0**,
  although ripgrep may work with older versions.
* Note that the binary may be bigger than expected because it contains debug
  symbols. This is intentional. To remove debug symbols and therefore reduce
  the file size, run `strip` on the binary.

```
$ cargo install ripgrep
```

When compiling with Rust 1.27 or newer, this will automatically enable SIMD
optimizations for search.

ripgrep isn't currently in any other package repositories.
[I'd like to change that](https://github.com/BurntSushi/ripgrep/issues/10).


### Building

ripgrep is written in Rust, so you'll need to grab a
[Rust installation](https://www.rust-lang.org/) in order to compile it.
ripgrep compiles with Rust 1.23.0 (stable) or newer. Building is easy:

```
$ git clone https://github.com/BurntSushi/ripgrep
$ cd ripgrep
$ cargo build --release
$ ./target/release/rg --version
0.1.3
```

If you have a Rust nightly compiler and a recent Intel CPU, then you can enable
additional optional SIMD acceleration like so:

```
RUSTFLAGS="-C target-cpu=native" cargo build --release --features 'simd-accel avx-accel'
```

If your machine doesn't support AVX instructions, then simply remove
`avx-accel` from the features list. Similarly for SIMD (which corresponds
roughly to SSE instructions).

The `simd-accel` and `avx-accel` features enable SIMD support in certain
ripgrep dependencies (responsible for counting lines and transcoding). They
are not necessary to get SIMD optimizations for search; those are enabled
automatically. Hopefully, some day, the `simd-accel` and `avx-accel` features
will similarly become unnecessary.


### Running tests

ripgrep is relatively well-tested, including both unit tests and integration
tests. To run the full test suite, use:

```
$ cargo test --all
```

from the repository root.
