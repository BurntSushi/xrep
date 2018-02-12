0.8.0 (2017-02-11)
==================
This is a new minor version releae of ripgrep that satisfies several popular
feature requests (config files, search compressed files, true colors), fixes
many bugs and improves the quality of life for ripgrep maintainers. This
release also includes greatly improved documentation in the form of a
[User Guide](GUIDE.md) and a [FAQ](FAQ.md).

This release increases the **minimum supported Rust version** from 1.17 to
1.20.

**BREAKING CHANGES**:

Note that these are all very minor and unlikely to impact most users.

* In order to support configuration files, flag overrides needed to be
  rethought. In some cases, this changed ripgrep's behavior. For example,
  in ripgrep 0.7.1, `rg foo -s -i` will perform a case sensitive search
  since the `-s/--case-sensitive` flag was defined to always take precedence
  over the `-i/--ignore-case` flag, regardless of position. In ripgrep 0.8.0
  however, the override rule for all flags has changed to "the most recent
  flag wins among competing flags." That is, `rg foo -s -i` now performs a
  case insensitive search.
* The `-M/--max-columns` flag was tweaked so that specifying a value of `0`
  now makes ripgrep behave as if the flag was absent. This makes it possible
  to set a default value in a configuration file and then override it. The
  previous ripgrep behavior was to suppress all matching non-empty lines.
* In all globs, `[^...]` is now equivalent to `[!...]` (indicating class
  negation). Previously, `^` had no special significance in a character class.
* For **downstream packagers**, the directory hierarchy in ripgrep's archive
  releases has changed. The root directory now only contains the executable,
  README and license. There is now a new directory called `doc` which contains
  the man page (previously in the root), a user guide (new), a FAQ (new) and
  the CHANGELOG (previously not included in release). The `complete`
  directory remains the same.

Feature enhancements:

* Added or improved file type filtering for
  Apache Avro, C++, GN, Google Closure Templates, Jupyter notebooks, man pages,
  Protocol Buffers, Smarty and Web IDL.
* [FEATURE #196](https://github.com/BurntSushi/ripgrep/issues/196):
  Support a configuration file. See
  [the new user guide](GUIDE.md#configuration-file)
  for details.
* [FEATURE #261](https://github.com/BurntSushi/ripgrep/issues/261):
  Add extended or "true" color support. Works in Windows 10!
  [See the FAQ for details.](FAQ.md#colors)
* [FEATURE #539](https://github.com/BurntSushi/ripgrep/issues/539):
  Search gzip, bzip2, lzma or xz files when given `-z/--search-zip` flag.
* [FEATURE #544](https://github.com/BurntSushi/ripgrep/issues/544):
  Add support for line number alignment via a new `--line-number-width` flag.
* [FEATURE #654](https://github.com/BurntSushi/ripgrep/pull/654):
  Support linuxbrew in ripgrep's Brew tap.
* [FEATURE #673](https://github.com/BurntSushi/ripgrep/issues/673):
  Bring back `.rgignore` files. (A higher precedent, application specific
  version of `.ignore`.)
* [FEATURE #676](https://github.com/BurntSushi/ripgrep/issues/676):
  Provide ARM binaries. **WARNING:** This will be provided on a best effort
  basis.
* [FEATURE #709](https://github.com/BurntSushi/ripgrep/issues/709):
  Suggest `-F/--fixed-strings` flag on a regex syntax error.
* [FEATURE #740](https://github.com/BurntSushi/ripgrep/issues/740):
  Add a `--passthru` flag that causes ripgrep to print every line it reads.
* [FEATURE #785](https://github.com/BurntSushi/ripgrep/pull/785):
  Overhaul documentation. Cleaned up README, added user guide and FAQ.
* [FEATURE 7f5c07](https://github.com/BurntSushi/ripgrep/commit/7f5c07434be92103b5bf7e216b9c7494aed2d8cb):
  Add hidden flags for convenient overrides (e.g., `--no-text`).

Bug fixes:

* [BUG #553](https://github.com/BurntSushi/ripgrep/issues/553):
  Permit flags to be repeated.
* [BUG #633](https://github.com/BurntSushi/ripgrep/issues/633):
  Fix a bug where ripgrep would panic on Windows while following symlinks.
* [BUG #649](https://github.com/BurntSushi/ripgrep/issues/649):
  Fix handling of `!**/` in `.gitignore`.
* [BUG #663](https://github.com/BurntSushi/ripgrep/issues/663):
  **BREAKING CHANGE:** Support `[^...]` glob syntax (as identical to `[!...]`).
* [BUG #693](https://github.com/BurntSushi/ripgrep/issues/693):
  Don't display context separators when not printing matches.
* [BUG #705](https://github.com/BurntSushi/ripgrep/issues/705):
  Fix a bug that prevented ripgrep from searching OneDrive directories.
* [BUG #717](https://github.com/BurntSushi/ripgrep/issues/717):
  Improve `--smart-case` uppercase character detection.
* [BUG #725](https://github.com/BurntSushi/ripgrep/issues/725):
  Clarify that globs do not override explicitly given paths to search.
* [BUG #742](https://github.com/BurntSushi/ripgrep/pull/742):
  Write ANSI reset code as `\x1B[0m` instead of `\x1B[m`.
* [BUG #747](https://github.com/BurntSushi/ripgrep/issues/747):
  Remove `yarn.lock` from YAML file type.
* [BUG #760](https://github.com/BurntSushi/ripgrep/issues/760):
  ripgrep can now search `/sys/devices/system/cpu/vulnerabilities/*` files.
* [BUG #761](https://github.com/BurntSushi/ripgrep/issues/761):
  Fix handling of gitignore patterns that contain a `/`.
* [BUG #776](https://github.com/BurntSushi/ripgrep/pull/776):
  **BREAKING CHANGE:** `--max-columns=0` now disables the limit.
* [BUG #779](https://github.com/BurntSushi/ripgrep/issues/779):
  Clarify documentation for `--files-without-match`.
* [BUG #780](https://github.com/BurntSushi/ripgrep/issues/780),
  [BUG #781](https://github.com/BurntSushi/ripgrep/issues/781):
  Fix bug where ripgrep missed some matching lines.

Maintenance fixes:

* [MAINT #772](https://github.com/BurntSushi/ripgrep/pull/772):
  Drop `env_logger` in favor of simpler logger to avoid many new dependencies.
* [MAINT #772](https://github.com/BurntSushi/ripgrep/pull/772):
  Add git revision hash to ripgrep's version string.
* [MAINT #772](https://github.com/BurntSushi/ripgrep/pull/772):
  (Seemingly) improve compile times.
* [MAINT #776](https://github.com/BurntSushi/ripgrep/pull/776):
  Automatically generate man page during build.
* [MAINT #786](https://github.com/BurntSushi/ripgrep/pull/786):
  Remove use of `unsafe` in `globset`. :tada:
* [MAINT e9d448](https://github.com/BurntSushi/ripgrep/commit/e9d448e93bb4e1fb3b0c1afc29adb5af6ed5283d):
  Add an issue template (has already drastically improved bug reports).
* [MAINT ae2d03](https://github.com/BurntSushi/ripgrep/commit/ae2d036dd4ba2a46acac9c2d77c32e7c667eb850):
  Remove the `compile` script.

Friends of ripgrep:

I'd like to extend my gratitude to
[@balajisivaraman](https://github.com/balajisivaraman)
for their recent hard work in a number of areas, and in particular, for
implementing the "search compressed files" feature. Their work in sketching out
a specification for that and other work has been exemplary.

Thanks
[@balajisivaraman](https://github.com/balajisivaraman)!


0.7.1 (2017-10-22)
==================
This is a patch release of ripgrep that includes a fix to very bad regression
introduced in ripgrep 0.7.0.

Bug fixes:

* [BUG #648](https://github.com/BurntSushi/ripgrep/issues/648):
  Fix a bug where it was very easy to exceed standard file descriptor limits.


0.7.0 (2017-10-20)
==================
This is a new minor version release of ripgrep that includes mostly bug fixes.

ripgrep continues to require Rust 1.17, and there are no known breaking changes
introduced in this release.

Feature enhancements:

* Added or improved file type filtering for config & license files, Elm,
  Purescript, Standard ML, sh, systemd, Terraform
* [FEATURE #593](https://github.com/BurntSushi/ripgrep/pull/593):
  Using both `-o/--only-matching` and `-r/--replace` does the right thing.

Bug fixes:

* [BUG #200](https://github.com/BurntSushi/ripgrep/issues/200):
  ripgrep will stop when its pipe is closed.
* [BUG #402](https://github.com/BurntSushi/ripgrep/issues/402):
  Fix context printing bug when the `-m/--max-count` flag is used.
* [BUG #521](https://github.com/BurntSushi/ripgrep/issues/521):
  Fix interaction between `-r/--replace` and terminal colors.
* [BUG #559](https://github.com/BurntSushi/ripgrep/issues/559):
  Ignore test that tried reading a non-UTF-8 file path on macOS.
* [BUG #599](https://github.com/BurntSushi/ripgrep/issues/599):
  Fix color escapes on empty matches.
* [BUG #600](https://github.com/BurntSushi/ripgrep/issues/600):
  Avoid expensive (on Windows) file handle check when using --files.
* [BUG #618](https://github.com/BurntSushi/ripgrep/issues/618):
  Clarify installation instructions for Ubuntu users.
* [BUG #633](https://github.com/BurntSushi/ripgrep/issues/633):
  Faster symlink loop checking on Windows.


0.6.0 (2017-08-23)
==================
This is a new minor version release of ripgrep that includes many bug fixes
and a few new features such as `--iglob` and `-x/--line-regexp`.

Note that this release increases the minimum supported Rust version from 1.12
to 1.17.

Feature enhancements:

* Added or improved file type filtering for BitBake, C++, Cabal, cshtml, Julia,
  Make, msbuild, QMake, Yocto
* [FEATURE #163](https://github.com/BurntSushi/ripgrep/issues/163):
  Add an `--iglob` flag that is like `-g/--glob`, but matches globs
  case insensitively.
* [FEATURE #520](https://github.com/BurntSushi/ripgrep/pull/518):
  Add `-x/--line-regexp` flag, which requires a match to span an entire line.
* [FEATURE #551](https://github.com/BurntSushi/ripgrep/pull/551),
  [FEATURE #554](https://github.com/BurntSushi/ripgrep/pull/554):
  `ignore`: add new `matched_path_or_any_parents` method.

Bug fixes:

* [BUG #342](https://github.com/BurntSushi/ripgrep/issues/342):
  Fix invisible text in some PowerShell environments by changing the
  default color scheme on Windows.
* [BUG #413](https://github.com/BurntSushi/ripgrep/issues/413):
  Release binaries on Unix are now `strip`'d by default. This decreases
  binary size by an order of magnitude.
* [BUG #483](https://github.com/BurntSushi/ripgrep/issues/483):
  When `--quiet` is passed, `--files` should be quiet.
* [BUG #488](https://github.com/BurntSushi/ripgrep/pull/488):
  When `--vimgrep` is passed, `--with-filename` should be enabled
  automatically.
* [BUG #493](https://github.com/BurntSushi/ripgrep/issues/493):
  Fix another bug in the implementation of the `-o/--only-matching`
  flag.
* [BUG #499](https://github.com/BurntSushi/ripgrep/pull/499):
  Permit certain flags to override others.
* [BUG #523](https://github.com/BurntSushi/ripgrep/pull/523):
  `wincolor`: Re-fetch Windows console on all calls.
* [BUG #523](https://github.com/BurntSushi/ripgrep/issues/524):
  `--version` now shows enabled compile-time features.
* [BUG #532](https://github.com/BurntSushi/ripgrep/issues/532),
  [BUG #536](https://github.com/BurntSushi/ripgrep/pull/536),
  [BUG #538](https://github.com/BurntSushi/ripgrep/pull/538),
  [BUG #540](https://github.com/BurntSushi/ripgrep/pull/540),
  [BUG #560](https://github.com/BurntSushi/ripgrep/pull/560),
  [BUG #565](https://github.com/BurntSushi/ripgrep/pull/565):
  Improve zsh completion.
* [BUG #578](https://github.com/BurntSushi/ripgrep/pull/578):
  Enable SIMD for `encoding_rs` when appropriate.
* [BUG #580](https://github.com/BurntSushi/ripgrep/issues/580):
  Fix `-w/--word-regexp` in the presence of capturing groups.
* [BUG #581](https://github.com/BurntSushi/ripgrep/issues/581):
  Document that ripgrep may terminate unexpectedly when searching via
  memory maps (which can happen using default settings).

Friends of ripgrep:

I'd like to give a big Thank You to @okdana for their recent hard work on
ripgrep. This includes new features like `--line-regexp`, heroic effort on
zsh auto-completion and thinking through some thorny argv issues with me.

I'd also like to thank @ericbn for their work on improving ripgrep's argv
parsing by allowing some flags to override others.

Thanks @okdana and @ericbn!


0.5.2 (2017-05-11)
==================
Feature enhancements:

* Added or improved file type filtering for Nix.
* [FEATURE #362](https://github.com/BurntSushi/ripgrep/issues/362):
  Add `--regex-size-limit` and `--dfa-size-limit` flags.
* [FEATURE #444](https://github.com/BurntSushi/ripgrep/issues/444):
  Improve error messages for invalid globs.

Bug fixes:

* [BUG #442](https://github.com/BurntSushi/ripgrep/issues/442):
  Fix line wrapping in `--help` output.
* [BUG #451](https://github.com/BurntSushi/ripgrep/issues/451):
  Fix bug with duplicate output when using `-o/--only-matching` flag.


0.5.1 (2017-04-09)
==================
Feature enhancements:

* Added or improved file type filtering for vim.
* [FEATURE #34](https://github.com/BurntSushi/ripgrep/issues/34):
  Add a `-o/--only-matching` flag.
* [FEATURE #377](https://github.com/BurntSushi/ripgrep/issues/377):
  Column numbers can now be customized with a color. (The default is
  no color.)
* [FEATURE #419](https://github.com/BurntSushi/ripgrep/issues/419):
  Added `-0` short flag option for `--null`.

Bug fixes:

* [BUG #381](https://github.com/BurntSushi/ripgrep/issues/381):
  Include license text in all subcrates.
* [BUG #418](https://github.com/BurntSushi/ripgrep/issues/418),
  [BUG #426](https://github.com/BurntSushi/ripgrep/issues/426),
  [BUG #439](https://github.com/BurntSushi/ripgrep/issues/439):
  Fix a few bugs with `-h/--help` output.


0.5.0 (2017-03-12)
==================
This is a new minor version release of ripgrep that includes one minor breaking
change, bug fixes and several new features including support for text encodings
other than UTF-8.

A notable accomplishment with respect to Rust is that ripgrep proper now only
contains a single `unsafe` use (for accessing the contents of a memory map).

The **breaking change** is:

* [FEATURE #380](https://github.com/BurntSushi/ripgrep/issues/380):
  Line numbers are now hidden by default when ripgrep is printing to a tty
  **and** the only thing searched is stdin.

Feature enhancements:

* Added or improved file type filtering for Ceylon, CSS, Elixir, HTML, log,
  SASS, SVG, Twig
* [FEATURE #1](https://github.com/BurntSushi/ripgrep/issues/1):
  Add support for additional text encodings, including automatic detection for
  UTF-16 via BOM sniffing. Explicit text encoding support with the
  `-E/--encoding` flag was also added for latin-1, GBK, EUC-JP
  and Shift_JIS, among others. The full list can be found here:
  https://encoding.spec.whatwg.org/#concept-encoding-get
* [FEATURE #129](https://github.com/BurntSushi/ripgrep/issues/129):
  Add a new `-M/--max-columns` flag that omits lines longer than the given
  number of bytes. (Disabled by default!)
* [FEATURE #369](https://github.com/BurntSushi/ripgrep/issues/369):
  A new flag, `--max-filesize`, was added for limiting searches to files with
  a maximum file size.

Bug fixes:

* [BUG #52](https://github.com/BurntSushi/ripgrep/issues/52),
  [BUG #311](https://github.com/BurntSushi/ripgrep/issues/311):
  Tweak how binary files are detected and handled. (We are slightly less
  conservative and will no longer use memory without bound.)
* [BUG #326](https://github.com/BurntSushi/ripgrep/issues/326):
  When --files flag is given, we should never attempt to parse positional
  arguments as regexes.
* [BUG #327](https://github.com/BurntSushi/ripgrep/issues/327):
  Permit the --heading flag to override the --no-heading flag.
* [BUG #340](https://github.com/BurntSushi/ripgrep/pull/340):
  Clarify that the `-u/--unrestricted` flags are aliases.
* [BUG #343](https://github.com/BurntSushi/ripgrep/pull/343):
  Global git ignore config should use `$HOME/.config/git/ignore` and not
  `$HOME/git/ignore`.
* [BUG #345](https://github.com/BurntSushi/ripgrep/pull/345):
  Clarify docs for `-g/--glob` flag.
* [BUG #381](https://github.com/BurntSushi/ripgrep/issues/381):
  Add license files to each sub-crate.
* [BUG #383](https://github.com/BurntSushi/ripgrep/issues/383):
  Use latest version of clap (for argv parsing).
* [BUG #392](https://github.com/BurntSushi/ripgrep/issues/391):
  Fix translation of set globs (e.g., `{foo,bar,quux}`) to regexes.
* [BUG #401](https://github.com/BurntSushi/ripgrep/pull/401):
  Add PowerShell completion file to Windows release.
* [BUG #405](https://github.com/BurntSushi/ripgrep/issues/405):
  Fix bug when excluding absolute paths with the `-g/--glob` flag.


0.4.0
=====
This is a new minor version release of ripgrep that includes a couple very
minor breaking changes, a few new features and lots of bug fixes.

This version of ripgrep upgrades its `regex` dependency from `0.1` to `0.2`,
which includes a few minor syntax changes:

* POSIX character classes now require double bracketing. Previously, the regex
  `[:upper:]` would parse as the `upper` POSIX character class. Now it parses
  as the character class containing the characters `:upper:`. The fix to this
  change is to use `[[:upper:]]` instead. Note that variants like
  `[[:upper:][:blank:]]` continue to work.
* The character `[` must always be escaped inside a character class.
* The characters `&`, `-` and `~` must be escaped if any one of them are
  repeated consecutively. For example, `[&]`, `[\&]`, `[\&\&]`, `[&-&]` are all
  equivalent while `[&&]` is illegal. (The motivation for this and the prior
  change is to provide a backwards compatible path for adding character class
  set notation.)

Feature enhancements:

* Added or improved file type filtering for Crystal, Kotlin, Perl, PowerShell,
  Ruby, Swig
* [FEATURE #83](https://github.com/BurntSushi/ripgrep/issues/83):
  Type definitions can now include other type definitions.
* [FEATURE #243](https://github.com/BurntSushi/ripgrep/issues/243):
  **BREAKING CHANGE**: The `--column` flag now implies `--line-number`.
* [FEATURE #263](https://github.com/BurntSushi/ripgrep/issues/263):
  Add a new `--sort-files` flag.
* [FEATURE #275](https://github.com/BurntSushi/ripgrep/issues/275):
  Add a new `--path-separator` flag. Useful in cygwin.

Bug fixes:

* [BUG #182](https://github.com/BurntSushi/ripgrep/issues/182):
  Redux: use more portable ANSI color escape sequences when possible.
* [BUG #258](https://github.com/BurntSushi/ripgrep/issues/258):
  Fix bug that caused ripgrep's parallel iterator to spin and burn CPU.
* [BUG #262](https://github.com/BurntSushi/ripgrep/issues/262):
  Document how to install shell completion files.
* [BUG #266](https://github.com/BurntSushi/ripgrep/issues/266),
  [BUG #293](https://github.com/BurntSushi/ripgrep/issues/293):
  Fix handling of bold styling and change the default colors.
* [BUG #268](https://github.com/BurntSushi/ripgrep/issues/268):
  Make lack of backreference support more explicit.
* [BUG #271](https://github.com/BurntSushi/ripgrep/issues/271):
  Remove `~` dependency on clap.
* [BUG #277](https://github.com/BurntSushi/ripgrep/issues/277):
  Fix cosmetic issue in `globset` crate docs.
* [BUG #279](https://github.com/BurntSushi/ripgrep/issues/279):
  ripgrep did not terminate when `-q/--quiet` was given.
* [BUG #281](https://github.com/BurntSushi/ripgrep/issues/281):
  **BREAKING CHANGE**: Completely remove `^C` handling from ripgrep.
* [BUG #284](https://github.com/BurntSushi/ripgrep/issues/284):
  Make docs for `-g/--glob` clearer.
* [BUG #286](https://github.com/BurntSushi/ripgrep/pull/286):
  When stdout is redirected to a file, don't search that file.
* [BUG #287](https://github.com/BurntSushi/ripgrep/pull/287):
  Fix ZSH completions.
* [BUG #295](https://github.com/BurntSushi/ripgrep/pull/295):
  Remove superfluous `memmap` dependency in `grep` crate.
* [BUG #308](https://github.com/BurntSushi/ripgrep/pull/308):
  Improve docs for `-r/--replace`.
* [BUG #313](https://github.com/BurntSushi/ripgrep/pull/313):
  Update bytecount dep to latest version.
* [BUG #318](https://github.com/BurntSushi/ripgrep/pull/318):
  Fix invalid UTF-8 output bug in Windows consoles.


0.3.2
=====
Feature enhancements:

* Added or improved file type filtering for Less, Sass, stylus, Zsh

Bug fixes:

* [BUG #229](https://github.com/BurntSushi/ripgrep/issues/229):
  Make smart case slightly less conservative.
* [BUG #247](https://github.com/BurntSushi/ripgrep/issues/247):
  Clarify use of --heading/--no-heading.
* [BUG #251](https://github.com/BurntSushi/ripgrep/issues/251),
  [BUG #264](https://github.com/BurntSushi/ripgrep/issues/264),
  [BUG #267](https://github.com/BurntSushi/ripgrep/issues/267):
  Fix matching bug caused by literal optimizations.
* [BUG #256](https://github.com/BurntSushi/ripgrep/issues/256):
  Fix bug that caused `rg foo` and `rg foo/` to have different behavior
  when `foo` was a symlink.
* [BUG #270](https://github.com/BurntSushi/ripgrep/issues/270):
  Fix bug where patterns starting with a `-` couldn't be used with the
  `-e/--regexp` flag. (This resolves a regression that was introduced in
  ripgrep 0.3.0.)


0.3.1
=====
Bug fixes:

* [BUG #242](https://github.com/BurntSushi/ripgrep/issues/242):
  ripgrep didn't respect `--colors foo:none` correctly. Now it does.


0.3.0
=====
This is a new minor version release of ripgrep that includes two breaking
changes with lots of bug fixes and some new features and performance
improvements. Notably, if you had a problem with colors or piping on Windows
before, then that should now be fixed in this release.

**BREAKING CHANGES**:

* ripgrep now requires Rust 1.11 to compile. Previously, it could build on
  Rust 1.9. The cause of this was the move from
  [Docopt to Clap](https://github.com/BurntSushi/ripgrep/pull/233)
  for argument parsing.
* The `-e/--regexp` flag can no longer accept a pattern starting with a `-`.
  There are two work-arounds: `rg -- -foo` and `rg [-]foo` or `rg -e [-]foo`
  will all search for the same `-foo` pattern. The cause of this was the move
  from [Docopt to Clap](https://github.com/BurntSushi/ripgrep/pull/233)
  for argument parsing.
  [This may get fixed in the
  future.](https://github.com/kbknapp/clap-rs/issues/742).

Performance improvements:

* [PERF #33](https://github.com/BurntSushi/ripgrep/issues/33):
  ripgrep now performs similar to GNU grep on small corpora.
* [PERF #136](https://github.com/BurntSushi/ripgrep/issues/136):
  ripgrep no longer slows down because of argument parsing when given a large
  argument list.

Feature enhancements:

* Added or improved file type filtering for Elixir.
* [FEATURE #7](https://github.com/BurntSushi/ripgrep/issues/7):
  Add a `-f/--file` flag that causes ripgrep to read patterns from a file.
* [FEATURE #51](https://github.com/BurntSushi/ripgrep/issues/51):
  Add a `--colors` flag that enables one to customize the colors used in
  ripgrep's output.
* [FEATURE #138](https://github.com/BurntSushi/ripgrep/issues/138):
  Add a `--files-without-match` flag that shows only file paths that contain
  zero matches.
* [FEATURE #230](https://github.com/BurntSushi/ripgrep/issues/230):
  Add completion files to the release (Bash, Fish and PowerShell).

Bug fixes:

* [BUG #37](https://github.com/BurntSushi/ripgrep/issues/37):
  Use correct ANSI escape sequences when `TERM=screen.linux`.
* [BUG #94](https://github.com/BurntSushi/ripgrep/issues/94):
  ripgrep now detects stdin on Windows automatically.
* [BUG #117](https://github.com/BurntSushi/ripgrep/issues/117):
  Colors should now work correctly and automatically inside mintty.
* [BUG #182](https://github.com/BurntSushi/ripgrep/issues/182):
  Colors should now work within Emacs. In particular, `--color=always` will
  emit colors regardless of the current environment.
* [BUG #189](https://github.com/BurntSushi/ripgrep/issues/189):
  Show less content when running `rg -h`. The full help content can be
  accessed with `rg --help`.
* [BUG #210](https://github.com/BurntSushi/ripgrep/issues/210):
  Support non-UTF-8 file names on Unix platforms.
* [BUG #231](https://github.com/BurntSushi/ripgrep/issues/231):
  Switch from block buffering to line buffering.
* [BUG #241](https://github.com/BurntSushi/ripgrep/issues/241):
  Some error messages weren't suppressed when `--no-messages` was used.


0.2.9
=====
Bug fixes:

* [BUG #226](https://github.com/BurntSushi/ripgrep/issues/226):
  File paths explicitly given on the command line weren't searched in parallel.
  (This was a regression in `0.2.7`.)
* [BUG #228](https://github.com/BurntSushi/ripgrep/issues/228):
  If a directory was given to `--ignore-file`, ripgrep's memory usage would
  grow without bound.


0.2.8
=====
Bug fixes:

* Fixed a bug with the SIMD/AVX features for using bytecount in commit
  `4ca15a`.


0.2.7
=====
Performance improvements:

* [PERF #223](https://github.com/BurntSushi/ripgrep/pull/223):
  Added a parallel recursive directory iterator. This results in major
  performance improvements on large repositories.
* [PERF #11](https://github.com/BurntSushi/ripgrep/pull/11):
  ripgrep now uses the `bytecount` library for counting new lines. In some
  cases, ripgrep runs twice as fast. Use
  `RUSTFLAGS="-C target-cpu=native" cargo build --release --features 'simd-accel avx-accel'`
  to get the fastest possible binary.

Feature enhancements:

* Added or improved file type filtering for Agda, Tex, Taskpaper, Markdown,
  asciidoc, textile, rdoc, org, creole, wiki, pod, C#, PDF, C, C++.
* [FEATURE #149](https://github.com/BurntSushi/ripgrep/issues/149):
  Add a new `--no-messages` flag that suppresses error messages.
  Note that `rg foo 2> /dev/null` also works.
* [FEATURE #159](https://github.com/BurntSushi/ripgrep/issues/159):
  Add a new `-m/--max-count` flag that limits the total number of matches
  printed for each file searched.

Bug fixes:

* [BUG #199](https://github.com/BurntSushi/ripgrep/issues/199):
  Fixed a bug where `-S/--smart-case` wasn't being applied correctly to
  literal optimizations.
* [BUG #203](https://github.com/BurntSushi/ripgrep/issues/203):
  Mention the full name, ripgrep, in more places. It now appears in
  the output of `--help` and `--version`. The repository URL is now also
  in the output of `--help` and the man page.
* [BUG #215](https://github.com/BurntSushi/ripgrep/issues/215):
  Include small note about how to search for a pattern that starts with a `-`.


0.2.6
=====
Feature enhancements:

* Added or improved file type filtering for Fish.

Bug fixes:

* [BUG #206](https://github.com/BurntSushi/ripgrep/issues/206):
  Fixed a regression with `-g/--glob` flag in `0.2.5`.


0.2.5
=====
Feature enhancements:

* Added or improved file type filtering for Groovy, Handlebars, Tcl, zsh and
  Python.
* [FEATURE #9](https://github.com/BurntSushi/ripgrep/issues/9):
  Support global gitignore config and `.git/info/exclude` files.
* [FEATURE #45](https://github.com/BurntSushi/ripgrep/issues/45):
  Add --ignore-file flag for specifying additional ignore files.
* [FEATURE #202](https://github.com/BurntSushi/ripgrep/pull/202):
  Introduce a new
  [`ignore`](https://github.com/BurntSushi/ripgrep/tree/master/ignore)
  crate that encapsulates all of ripgrep's gitignore matching logic.

Bug fixes:

* [BUG #44](https://github.com/BurntSushi/ripgrep/issues/44):
  ripgrep runs slowly when given lots of positional arguments that are
  directories.
* [BUG #119](https://github.com/BurntSushi/ripgrep/issues/119):
  ripgrep didn't reset terminal colors if it was interrupted by `^C`.
  Fixed in [PR #187](https://github.com/BurntSushi/ripgrep/pull/187).
* [BUG #184](https://github.com/BurntSushi/ripgrep/issues/184):
  Fixed a bug related to interpreting gitignore files in parent directories.


0.2.4
=====
SKIPPED.


0.2.3
=====
Bug fixes:

* [BUG #164](https://github.com/BurntSushi/ripgrep/issues/164):
  Fixes a segfault on macos builds.
* [BUG #167](https://github.com/BurntSushi/ripgrep/issues/167):
  Clarify documentation for --threads.


0.2.2
=====
Packaging updates:

* `ripgrep` is now in homebrew-core. `brew install ripgrep` will do the trick
  on a Mac.
* `ripgrep` is now in the Archlinux community repository.
  `pacman -S ripgrep` will do the trick on Archlinux.
* Support has been discontinued for i686-darwin.
* Glob matching has been moved out into its own crate:
  [`globset`](https://crates.io/crates/globset).

Feature enhancements:

* Added or improved file type filtering for CMake, config, Jinja, Markdown,
  Spark.
* [FEATURE #109](https://github.com/BurntSushi/ripgrep/issues/109):
  Add a --max-depth flag for directory traversal.
* [FEATURE #124](https://github.com/BurntSushi/ripgrep/issues/124):
  Add -s/--case-sensitive flag. Overrides --smart-case.
* [FEATURE #139](https://github.com/BurntSushi/ripgrep/pull/139):
  The `ripgrep` repo is now a Homebrew tap. This is useful for installing
  SIMD accelerated binaries, which aren't available in homebrew-core.

Bug fixes:

* [BUG #87](https://github.com/BurntSushi/ripgrep/issues/87),
  [BUG #127](https://github.com/BurntSushi/ripgrep/issues/127),
  [BUG #131](https://github.com/BurntSushi/ripgrep/issues/131):
  Various issues related to glob matching.
* [BUG #116](https://github.com/BurntSushi/ripgrep/issues/116):
  --quiet should stop search after first match.
* [BUG #121](https://github.com/BurntSushi/ripgrep/pull/121):
  --color always should show colors, even when --vimgrep is used.
* [BUG #122](https://github.com/BurntSushi/ripgrep/pull/122):
  Colorize file path at beginning of line.
* [BUG #134](https://github.com/BurntSushi/ripgrep/issues/134):
  Processing a large ignore file (thousands of globs) was very slow.
* [BUG #137](https://github.com/BurntSushi/ripgrep/issues/137):
  Always follow symlinks when given as an explicit argument.
* [BUG #147](https://github.com/BurntSushi/ripgrep/issues/147):
  Clarify documentation for --replace.


0.2.1
=====
Feature enhancements:

* Added or improved file type filtering for Clojure and SystemVerilog.
* [FEATURE #89](https://github.com/BurntSushi/ripgrep/issues/89):
  Add a --null flag that outputs a NUL byte after every file path.

Bug fixes:

* [BUG #98](https://github.com/BurntSushi/ripgrep/issues/98):
  Fix a bug in single threaded mode when if opening a file failed, ripgrep
  quit instead of continuing the search.
* [BUG #99](https://github.com/BurntSushi/ripgrep/issues/99):
  Fix another bug in single threaded mode where empty lines were being printed
  by mistake.
* [BUG #105](https://github.com/BurntSushi/ripgrep/issues/105):
  Fix an off-by-one error with --column.
* [BUG #106](https://github.com/BurntSushi/ripgrep/issues/106):
  Fix a bug where a whitespace only line in a gitignore file caused ripgrep
  to panic (i.e., crash).


0.2.0
=====
Feature enhancements:

* Added or improved file type filtering for VB, R, F#, Swift, Nim, Javascript,
  TypeScript
* [FEATURE #20](https://github.com/BurntSushi/ripgrep/issues/20):
  Adds a --no-filename flag.
* [FEATURE #26](https://github.com/BurntSushi/ripgrep/issues/26):
  Adds --files-with-matches flag. Like --count, but only prints file paths
  and doesn't need to count every match.
* [FEATURE #40](https://github.com/BurntSushi/ripgrep/issues/40):
  Switch from using `.rgignore` to `.ignore`. Note that `.rgignore` is
  still supported, but deprecated.
* [FEATURE #68](https://github.com/BurntSushi/ripgrep/issues/68):
  Add --no-ignore-vcs flag that ignores .gitignore but not .ignore.
* [FEATURE #70](https://github.com/BurntSushi/ripgrep/issues/70):
  Add -S/--smart-case flag (but is disabled by default).
* [FEATURE #80](https://github.com/BurntSushi/ripgrep/issues/80):
  Add support for `{foo,bar}` globs.

Many many bug fixes. Thanks every for reporting these and helping make
`ripgrep` better! (Note that I haven't captured every tracking issue here,
some were closed as duplicates.)

* [BUG #8](https://github.com/BurntSushi/ripgrep/issues/8):
  Don't use an intermediate buffer when --threads=1. (Permits constant memory
  usage.)
* [BUG #15](https://github.com/BurntSushi/ripgrep/issues/15):
  Improves the documentation for --type-add.
* [BUG #16](https://github.com/BurntSushi/ripgrep/issues/16),
  [BUG #49](https://github.com/BurntSushi/ripgrep/issues/49),
  [BUG #50](https://github.com/BurntSushi/ripgrep/issues/50),
  [BUG #65](https://github.com/BurntSushi/ripgrep/issues/65):
  Some gitignore globs were being treated as anchored when they weren't.
* [BUG #18](https://github.com/BurntSushi/ripgrep/issues/18):
  --vimgrep reported incorrect column number.
* [BUG #19](https://github.com/BurntSushi/ripgrep/issues/19):
  ripgrep was hanging waiting on stdin in some Windows terminals. Note that
  this introduced a new bug:
  [#94](https://github.com/BurntSushi/ripgrep/issues/94).
* [BUG #21](https://github.com/BurntSushi/ripgrep/issues/21):
  Removes leading `./` when printing file paths.
* [BUG #22](https://github.com/BurntSushi/ripgrep/issues/22):
  Running `rg --help | echo` caused `rg` to panic.
* [BUG #24](https://github.com/BurntSushi/ripgrep/issues/22):
  Clarify the central purpose of rg in its usage message.
* [BUG #25](https://github.com/BurntSushi/ripgrep/issues/25):
  Anchored gitignore globs weren't applied in subdirectories correctly.
* [BUG #30](https://github.com/BurntSushi/ripgrep/issues/30):
  Globs like `foo/**` should match contents of `foo`, but not `foo` itself.
* [BUG #35](https://github.com/BurntSushi/ripgrep/issues/35),
  [BUG #81](https://github.com/BurntSushi/ripgrep/issues/81):
  When automatically detecting stdin, only read if it's a file or a fifo.
  i.e., ignore stdin in `rg foo < /dev/null`.
* [BUG #36](https://github.com/BurntSushi/ripgrep/issues/36):
  Don't automatically pick memory maps on MacOS. Ever.
* [BUG #38](https://github.com/BurntSushi/ripgrep/issues/38):
  Trailing whitespace in gitignore wasn't being ignored.
* [BUG #43](https://github.com/BurntSushi/ripgrep/issues/43):
  --glob didn't work with directories.
* [BUG #46](https://github.com/BurntSushi/ripgrep/issues/46):
  Use one fewer worker thread than what is provided on CLI.
* [BUG #47](https://github.com/BurntSushi/ripgrep/issues/47):
  --help/--version now work even if other options are set.
* [BUG #55](https://github.com/BurntSushi/ripgrep/issues/55):
  ripgrep was refusing to search /proc/cpuinfo. Fixed by disabling memory
  maps for files with zero size.
* [BUG #64](https://github.com/BurntSushi/ripgrep/issues/64):
  The first path given with --files set was ignored.
* [BUG #67](https://github.com/BurntSushi/ripgrep/issues/67):
  Sometimes whitelist globs like `!/dir` weren't interpreted as anchored.
* [BUG #77](https://github.com/BurntSushi/ripgrep/issues/77):
  When -q/--quiet flag was passed, ripgrep kept searching even after a match
  was found.
* [BUG #90](https://github.com/BurntSushi/ripgrep/issues/90):
  Permit whitelisting hidden files.
* [BUG #93](https://github.com/BurntSushi/ripgrep/issues/93):
  ripgrep was extracting an erroneous inner literal from a repeated pattern.
