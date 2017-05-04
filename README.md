ripgrep (rg)
------------
`ripgrep` is a line oriented search tool that combines the usability of The
Silver Searcher (similar to `ack`) with the raw speed of GNU grep. `ripgrep`
works by recursively searching your current directory for a regex pattern.
`ripgrep` has first class support on Windows, Mac and Linux, with binary
downloads available for
[every release](https://github.com/BurntSushi/ripgrep/releases).

[![Linux build status](https://travis-ci.org/BurntSushi/ripgrep.svg?branch=master)](https://travis-ci.org/BurntSushi/ripgrep)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/BurntSushi/ripgrep?svg=true)](https://ci.appveyor.com/project/BurntSushi/ripgrep)
[![](https://img.shields.io/crates/v/ripgrep.svg)](https://crates.io/crates/ripgrep)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).

### CHANGELOG

Please see the [CHANGELOG](CHANGELOG.md) for a release history.

### Screenshot of search results

[![A screenshot of a sample search with ripgrep](http://burntsushi.net/stuff/ripgrep1.png)](http://burntsushi.net/stuff/ripgrep1.png)

### Quick examples comparing tools

This example searches the entire Linux kernel source tree (after running
`make defconfig && make -j8`) for `[A-Z]+_SUSPEND`, where all matches must be
words. Timings were collected on a system with an Intel i7-6900K 3.2 GHz, and
ripgrep was compiled using the `compile` script in this repo.

Please remember that a single benchmark is never enough! See my
[blog post on `ripgrep`](http://blog.burntsushi.net/ripgrep/)
for a very detailed comparison with more benchmarks and analysis.

| Tool | Command | Line count | Time |
| ---- | ------- | ---------- | ---- |
| ripgrep (Unicode) | `rg -n -w '[A-Z]+_SUSPEND'` | 450 | **0.134s** |
| [The Silver Searcher](https://github.com/ggreer/the_silver_searcher) | `ag -w '[A-Z]+_SUSPEND'` | 450 | 0.753s |
| [git grep](https://www.kernel.org/pub/software/scm/git/docs/git-grep.html) | `LC_ALL=C git grep -E -n -w '[A-Z]+_SUSPEND'` | 450 | 0.823s |
| [git grep (Unicode)](https://www.kernel.org/pub/software/scm/git/docs/git-grep.html) | `LC_ALL=en_US.UTF-8 git grep -E -n -w '[A-Z]+_SUSPEND'` | 450 | 2.880s |
| [sift](https://github.com/svent/sift) | `sift --git -n -w '[A-Z]+_SUSPEND'` | 450 | 3.656s |
| [The Platinum Searcher](https://github.com/monochromegane/the_platinum_searcher) | `pt -w -e '[A-Z]+_SUSPEND'` | 450 | 12.369s |
| [ack](https://github.com/petdance/ack2) | `ack -w '[A-Z]+_SUSPEND'` | 1878 | 16.952s |

(Yes, `ack` [has](https://github.com/petdance/ack2/issues/445) a
[bug](https://github.com/petdance/ack2/issues/14).)

Here's another benchmark that disregards gitignore files and searches with a
whitelist instead. The corpus is the same as in the previous benchmark, and the
flags passed to each command ensures that they are doing equivalent work:

| Tool | Command | Line count | Time |
| ---- | ------- | ---------- | ---- |
| ripgrep | `rg -L -u -tc -n -w '[A-Z]+_SUSPEND'` | 404 | **0.108s** |
| [ucg](https://github.com/gvansickle/ucg) | `ucg --type=cc -w '[A-Z]+_SUSPEND'` | 392 | 0.219s |
| [GNU grep](https://www.gnu.org/software/grep/) | `egrep -R -n --include='*.c' --include='*.h' -w '[A-Z]+_SUSPEND'` | 404 | 0.733s |

(`ucg` [has slightly different behavior in the presence of symbolic links](https://github.com/gvansickle/ucg/issues/106).)

And finally, a straight up comparison between ripgrep and GNU grep on a single
large file (~9.3GB,
[`OpenSubtitles2016.raw.en.gz`](http://opus.lingfil.uu.se/OpenSubtitles2016/mono/OpenSubtitles2016.raw.en.gz)):

| Tool | Command | Line count | Time |
| ---- | ------- | ---------- | ---- |
| ripgrep | `rg -w 'Sherlock [A-Z]\w+'` | 5268 | **2.520s** |
| [GNU grep](https://www.gnu.org/software/grep/) | `LC_ALL=C egrep -w 'Sherlock [A-Z]\w+'` | 5268 | 7.143s |

In the above benchmark, passing the `-n` flag (for showing line numbers)
increases the times to `3.081s` for ripgrep and `11.403s` for GNU grep.

### Why should I use `ripgrep`?

* It can replace both The Silver Searcher and GNU grep because it is faster
  than both. (N.B. It is not, strictly speaking, a "drop-in" replacement for
  both, but the feature sets are far more similar than different.)
* Like The Silver Searcher, `ripgrep` defaults to recursive directory search
  and won't search files ignored by your `.gitignore` files. It also ignores
  hidden and binary files by default. `ripgrep` also implements full support
  for `.gitignore`, where as there are many bugs related to that functionality
  in The Silver Searcher.
* `ripgrep` can search specific types of files. For example, `rg -tpy foo`
  limits your search to Python files and `rg -Tjs foo` excludes Javascript
  files from your search. `ripgrep` can be taught about new file types with
  custom matching rules.
* `ripgrep` supports many features found in `grep`, such as showing the context
  of search results, searching multiple patterns, highlighting matches with
  color and full Unicode support. Unlike GNU grep, `ripgrep` stays fast while
  supporting Unicode (which is always on).
* `ripgrep` supports searching files in text encodings other than UTF-8, such
  as UTF-16, latin-1, GBK, EUC-JP, Shift_JIS and more. (Some support for
  automatically detecting UTF-16 is provided. Other text encodings must be
  specifically specified with the `-E/--encoding` flag.)

In other words, use `ripgrep` if you like speed, filtering by default, fewer
bugs and Unicode support.

### Why shouldn't I use `ripgrep`?

I'd like to try to convince you why you *shouldn't* use `ripgrep`. This should
give you a glimpse at some important downsides or missing features of
`ripgrep`.

* `ripgrep` uses a regex engine based on finite automata, so if you want fancy
  regex features such as backreferences or look around, `ripgrep` won't give
  them to you. `ripgrep` does support lots of things though, including, but not
  limited to: lazy quantification (e.g., `a+?`), repetitions (e.g., `a{2,5}`),
  begin/end assertions (e.g., `^\w+$`), word boundaries (e.g., `\bfoo\b`), and
  support for Unicode categories (e.g., `\p{Sc}` to match currency symbols or
  `\p{Lu}` to match any uppercase letter). (Fancier regexes will never be
  supported.)
* `ripgrep` doesn't yet support searching compressed files. (Likely to be
  supported in the future.)
* `ripgrep` doesn't have multiline search. (Unlikely to ever be supported.)

In other words, if you like fancy regexes, searching compressed files or
multiline search, then `ripgrep` may not quite meet your needs (yet).

### Is it really faster than everything else?

Yes. A large number of benchmarks with detailed analysis for each is
[available on my blog](http://blog.burntsushi.net/ripgrep/).

Summarizing, `ripgrep` is fast because:

* It is built on top of
  [Rust's regex engine](https://github.com/rust-lang-nursery/regex).
  Rust's regex engine uses finite automata, SIMD and aggressive literal
  optimizations to make searching very fast.
* Rust's regex library maintains performance with full Unicode support by
  building UTF-8 decoding directly into its deterministic finite automaton
  engine.
* It supports searching with either memory maps or by searching incrementally
  with an intermediate buffer. The former is better for single files and the
  latter is better for large directories. `ripgrep` chooses the best searching
  strategy for you automatically.
* Applies your ignore patterns in `.gitignore` files using a
  [`RegexSet`](https://doc.rust-lang.org/regex/regex/struct.RegexSet.html).
  That means a single file path can be matched against multiple glob patterns
  simultaneously.
* It uses a lock-free parallel recursive directory iterator, courtesy of
  [`crossbeam`](https://docs.rs/crossbeam) and
  [`ignore`](https://docs.rs/ignore).

### Installation

The binary name for `ripgrep` is `rg`.

[Binaries for `ripgrep` are available for Windows, Mac and
Linux.](https://github.com/BurntSushi/ripgrep/releases) Linux binaries are
static executables. Windows binaries are available either as built with MinGW
(GNU) or with Microsoft Visual C++ (MSVC). When possible, prefer MSVC over GNU,
but you'll need to have the
[Microsoft VC++ 2015 redistributable](https://www.microsoft.com/en-us/download/details.aspx?id=48145)
installed.

If you're a **Mac OS X Homebrew** user, then you can install ripgrep either
from homebrew-core, (compiled with rust stable, no SIMD):

```
$ brew install ripgrep
```

or you can install a binary compiled with rust nightly (including SIMD and all
optimizations) by utilizing a custom tap:

```
$ brew tap burntsushi/ripgrep https://github.com/BurntSushi/ripgrep.git
$ brew install burntsushi/ripgrep/ripgrep-bin
```

If you're a **Windows Chocolatey** user, then you can install `ripgrep` from the [official repo](https://chocolatey.org/packages/ripgrep):

```
$ choco install ripgrep
```

If you're an **Arch Linux** user, then you can install `ripgrep` from the official repos:

```
$ pacman -S ripgrep
```

If you're a **Gentoo** user, you can install `ripgrep` from the [official repo](https://packages.gentoo.org/packages/sys-apps/ripgrep):

```
$ emerge ripgrep
```

If you're a **Fedora 24+** user, you can install `ripgrep` from [copr](https://copr.fedorainfracloud.org/coprs/carlgeorge/ripgrep/):

```
$ dnf copr enable carlgeorge/ripgrep
$ dnf install ripgrep
```

If you're a **RHEL/CentOS 7** user, you can install `ripgrep` from [copr](https://copr.fedorainfracloud.org/coprs/carlgeorge/ripgrep/):

```
$ yum-config-manager --add-repo=https://copr.fedorainfracloud.org/coprs/carlgeorge/ripgrep/repo/epel-7/carlgeorge-ripgrep-epel-7.repo
$ yum install ripgrep
```

If you're a **Nix** user, you can install `ripgrep` from
[nixpkgs](https://github.com/NixOS/nixpkgs/blob/master/pkgs/tools/text/ripgrep/default.nix):

```
$ nix-env --install ripgrep
$ # (Or using the attribute name, which is also `ripgrep`.)
```

If you're a **Rust programmer**, `ripgrep` can be installed with `cargo`. Note
that this requires you to have **Rust 1.12 or newer** installed.

```
$ cargo install ripgrep
```

`ripgrep` isn't currently in any other package repositories.
[I'd like to change that](https://github.com/BurntSushi/ripgrep/issues/10).

### Whirlwind tour

The command line usage of `ripgrep` doesn't differ much from other tools that
perform a similar function, so you probably already know how to use `ripgrep`.
The full details can be found in `rg --help`, but let's go on a whirlwind tour.

`ripgrep` detects when its printing to a terminal, and will automatically
colorize your output and show line numbers, just like The Silver Searcher.
Coloring works on Windows too! Colors can be controlled more granularly with
the `--color` flag.

One last thing before we get started: generally speaking, `ripgrep` assumes the
input is reading is UTF-8. However, if ripgrep notices a file is encoded as
UTF-16, then it will know how to search it. For other encodings, you'll need to
explicitly specify them with the `-E/--encoding` flag.

To recursively search the current directory, while respecting all `.gitignore`
files, ignore hidden files and directories and skip binary files:

```
$ rg foobar
```

The above command also respects all `.ignore` files, including in parent
directories. `.ignore` files can be used when `.gitignore` files are
insufficient. In all cases, `.ignore` patterns take precedence over
`.gitignore`.

To ignore all ignore files, use `-u`. To additionally search hidden files
and directories, use `-uu`. To additionally search binary files, use `-uuu`.
(In other words, "search everything, dammit!") In particular, `rg -uuu` is
similar to `grep -a -r`.

```
$ rg -uu foobar  # similar to `grep -r`
$ rg -uuu foobar  # similar to `grep -a -r`
```

(Tip: If your ignore files aren't being adhered to like you expect, run your
search with the `--debug` flag.)

Make the search case insensitive with `-i`, invert the search with `-v` or
show the 2 lines before and after every search result with `-C2`.

Force all matches to be surrounded by word boundaries with `-w`.

Search and replace (find first and last names and swap them):

```
$ rg '([A-Z][a-z]+)\s+([A-Z][a-z]+)' --replace '$2, $1'
```

Named groups are supported:

```
$ rg '(?P<first>[A-Z][a-z]+)\s+(?P<last>[A-Z][a-z]+)' --replace '$last, $first'
```

Up the ante with full Unicode support, by matching any uppercase Unicode letter
followed by any sequence of lowercase Unicode letters (good luck doing this
with other search tools!):

```
$ rg '(\p{Lu}\p{Ll}+)\s+(\p{Lu}\p{Ll}+)' --replace '$2, $1'
```

Search only files matching a particular glob:

```
$ rg foo -g 'README.*'
```

<!--*-->

Or exclude files matching a particular glob:

```
$ rg foo -g '!*.min.js'
```

Search and return paths matching a particular glob (i.e., `-g` flag in ag/ack):

```
$ rg -g 'doc*' --files
```

Search only HTML and CSS files:

```
$ rg -thtml -tcss foobar
```

Search everything except for Javascript files:

```
$ rg -Tjs foobar
```

To see a list of types supported, run `rg --type-list`. To add a new type, use
`--type-add`, which must be accompanied by a pattern for searching (`rg` won't
persist your type settings):

```
$ rg --type-add 'foo:*.{foo,foobar}' -tfoo bar
```

The type `foo` will now match any file ending with the `.foo` or `.foobar`
extensions.

### Regex syntax

The syntax supported is
[documented as part of Rust's regex library](https://doc.rust-lang.org/regex/regex/index.html#syntax).

### Shell completions

Shell completion files are included in the release tarball for Bash, Fish, Zsh
and PowerShell.

For **bash**, move `rg.bash-completion` to `$XDG_CONFIG_HOME/bash_completion`
or `/etc/bash_completion.d/`.

For **fish**, move `rg.fish` to `$HOME/.config/fish/completions`.

For **PowerShell**, add `. _rg.ps1` to your PowerShell
[profile](https://technet.microsoft.com/en-us/library/bb613488(v=vs.85).aspx)
(note the leading period). If the `_rg.ps1` file is not on your `PATH`, do
`. /path/to/_rg.ps1` instead. 

### Building

`ripgrep` is written in Rust, so you'll need to grab a
[Rust installation](https://www.rust-lang.org/) in order to compile it.
`ripgrep` compiles with Rust 1.12 (stable) or newer. Building is easy:

```
$ git clone https://github.com/BurntSushi/ripgrep
$ cd ripgrep
$ cargo build --release
$ ./target/release/rg --version
0.1.3
```

If you have a Rust nightly compiler, then you can enable optional SIMD
acceleration like so:

```
RUSTFLAGS="-C target-cpu=native" cargo build --release --features 'simd-accel avx-accel'
```

If your machine doesn't support AVX instructions, then simply remove
`avx-accel` from the features list. Similarly for SIMD.

### Running tests

`ripgrep` is relatively well tested, including both unit tests and integration
tests. To run the full test suite, use:

```
$ cargo test
```

from the repository root.

### Tips

#### Windows Powershell

##### Powershell Profile

To customize powershell on start-up there is a special powershell script that has to be created.
In order to find its location run command `Get-Command $profile | Select-Object -ExpandProperty Definition`
See [more](https://technet.microsoft.com/en-us/library/bb613488(v=vs.85).aspx) for profile details.

Any powershell code in this file gets evaluated at the start of console.
This way you can have own aliases to be created at start.

##### Setup function alias

Often you can find a need to make alias for the favourite utility.

But powershell function aliases do not behave like your typical linux shell alias.

You always need to propagate arguments and **Stdin** input.
But it cannot be done simply as `function grep() { $input | rg.exe --hidden $args }`

Use below example as reference to how setup alias in powershell.

```powershell
function grep {
    $count = @($input).Count
    $input.Reset()

    if ($count) {
        $input | rg.exe --hidden $args
    }
    else {
        rg.exe --hidden $args
    }
}
```

Powershell special variables:
* input - is powershell **Stdin** object that allows you to access its content.
* args - is array of arguments passed to this function.

This alias checks whether there is **Stdin** input and propagates only if there is some lines.
Otherwise empty `$input` will make powershell to trigger `rg` to search empty **Stdin**

##### Piping non-ASCII content to ripgrep

When piping input into native executables in PowerShell, the encoding of the
input is controlled by the `$OutputEncoding` variable. By default, this is set
to US-ASCII, and any characters in the pipeline that don't have encodings in
US-ASCII are converted to `?` (question mark) characters.

To change this setting, set `$OutputEncoding` to a different encoding, as
represented by a .NET encoding object. Some common examples are below. The
value of this variable is reset when PowerShell restarts, so to make this
change take effect every time PowerShell is started add a line setting the
variable into your PowerShell profile.

Example `$OutputEncoding` settings:
* UTF-8 without BOM: `$OutputEncoding = [System.Text.UTF8Encoding]::new()`
* The console's output encoding:
`$OutputEncoding = [System.Console]::OutputEncoding`

If you continue to have encoding problems, you can also force the encoding
that the console will use for printing to UTF-8 with 
`[System.Console]::OutputEncoding = [System.Text.Encoding]::UTF8`. This
will also reset when PowerShell is restarted, so you can add that line
to your profile as well if you want to make the setting permanent.

### Known issues

#### I just hit Ctrl+C in the middle of ripgrep's output and now my terminal's foreground color is wrong!

Type in `color` in cmd.exe (Command Prompt) and `echo -ne "\033[0m"` on Unix
to restore your original foreground color.

In PowerShell, you can add the following code to your profile which will
restore the original foreground color when `Reset-ForegroundColor` is called.
Including the `Set-Alias` line will allow you to call it with simply `color`.

```powershell
$OrigFgColor = $Host.UI.RawUI.ForegroundColor
function Reset-ForegroundColor {
	$Host.UI.RawUI.ForegroundColor = $OrigFgColor
}
Set-Alias -Name color -Value Reset-ForegroundColor
```

PR [#187](https://github.com/BurntSushi/ripgrep/pull/187) fixed this, and it
was later deprecated in
[#281](https://github.com/BurntSushi/ripgrep/issues/281). A full explanation is
available [here][msys issue explanation].

[msys issue explanation]: https://github.com/BurntSushi/ripgrep/issues/281#issuecomment-269093893
