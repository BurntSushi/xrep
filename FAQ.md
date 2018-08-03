## FAQ

* [Does ripgrep support configuration files?](#config)
* [What's changed in ripgrep recently?](#changelog)
* [When is the next release?](#release)
* [Does ripgrep have a man page?](#manpage)
* [Does ripgrep have support for shell auto-completion?](#complete)
* [How do I use lookaround and/or backreferences?](#fancy)
* [How do I configure ripgrep's colors?](#colors)
* [How do I enable true colors on Windows?](#truecolors-windows)
* [How do I stop ripgrep from messing up colors when I kill it?](#stop-ripgrep)
* [How can I get results in a consistent order?](#order)
* [How do I search files that aren't UTF-8?](#encoding)
* [How do I search compressed files?](#compressed)
* [How do I search over multiple lines?](#multiline)
* [How do I get around the regex size limit?](#size-limit)
* [How do I make the `-f/--file` flag faster?](#dfa-size)
* [How do I make the output look like The Silver Searcher's output?](#silver-searcher-output)
* [When I run `rg`, why does it execute some other command?](#rg-other-cmd)
* [How do I create an alias for ripgrep on Windows?](#rg-alias-windows)
* [How do I create a PowerShell profile?](#powershell-profile)
* [How do I pipe non-ASCII content to ripgrep on Windows?](#pipe-non-ascii-windows)
* [How can I search and replace with ripgrep?](#search-and-replace)
* [How is ripgrep licensed?](#license)
* [Can ripgrep replace grep?](#posix4ever)
* [What does the "rip" in ripgrep mean?](#intentcountsforsomething)


<h3 name="config">
Does ripgrep support configuration files?
</h3>

Yes. See the
[guide's section on configuration files](GUIDE.md#configuration-file).


<h3 name="changelog">
What's changed in ripgrep recently?
</h3>

Please consult ripgrep's [CHANGELOG](CHANGELOG.md).


<h3 name="release">
When is the next release?
</h3>

ripgrep is a project whose contributors are volunteers. A release schedule
adds undue stress to said volunteers. Therefore, releases are made on a best
effort basis and no dates **will ever be given**.

One exception to this is high impact bugs. If a ripgrep release contains a
significant regression, then there will generally be a strong push to get a
patch release out with a fix.


<h3 name="manpage">
Does ripgrep have a man page?
</h3>

Yes! Whenever ripgrep is compiled on a system with `asciidoc` present, then a
man page is generated from ripgrep's argv parser. After compiling ripgrep, you
can find the man page like so from the root of the repository:

```
$ find ./target -name rg.1 -print0 | xargs -0 ls -t | head -n1
./target/debug/build/ripgrep-79899d0edd4129ca/out/rg.1
```

Running `man -l ./target/debug/build/ripgrep-79899d0edd4129ca/out/rg.1` will
show the man page in your normal pager.

Note that the man page's documentation for options is equivalent to the output
shown in `rg --help`. To see more condensed documentation (one line per flag),
run `rg -h`.

The man page is also included in all
[ripgrep binary releases](https://github.com/BurntSushi/ripgrep/releases).


<h3 name="complete">
Does ripgrep have support for shell auto-completion?
</h3>

Yes! Shell completions can be found in the
[same directory as the man page](#manpage)
after building ripgrep. Zsh completions are maintained separately and committed
to the repository in `complete/_rg`.

Shell completions are also included in all
[ripgrep binary releases](https://github.com/BurntSushi/ripgrep/releases).

For **bash**, move `rg.bash` to
`$XDG_CONFIG_HOME/bash_completion` or `/etc/bash_completion.d/`.

For **fish**, move `rg.fish` to `$HOME/.config/fish/completions/`.

For **zsh**, move `_rg` to one of your `$fpath` directories.

For **PowerShell**, add `. _rg.ps1` to your PowerShell
[profile](https://technet.microsoft.com/en-us/library/bb613488(v=vs.85).aspx)
(note the leading period). If the `_rg.ps1` file is not on your `PATH`, do
`. /path/to/_rg.ps1` instead.


<h3 name="order">
How can I get results in a consistent order?
</h3>

By default, ripgrep uses parallelism to execute its search because this makes
the search much faster on most modern systems. This in turn means that ripgrep
has a non-deterministic aspect to it, since the interleaving of threads during
the execution of the program is itself non-deterministic. This has the effect
of printing results in a somewhat arbitrary order, and this order can change
from run to run of ripgrep.

The only way to make the order of results consistent is to ask ripgrep to
sort the output. Currently, this will disable all parallelism. (On smaller
repositories, you might not notice much of a performance difference!) You
can achieve this with the `--sort-files` flag.

There is more discussion on this topic here:
https://github.com/BurntSushi/ripgrep/issues/152


<h3 name="encoding">
How do I search files that aren't UTF-8?
</h3>

See the [guide's section on file encoding](GUIDE.md#file-encoding).


<h3 name="compressed">
How do I search compressed files?
</h3>

ripgrep's `-z/--search-zip` flag will cause it to search compressed files
automatically. Currently, this supports gzip, bzip2, lzma, lz4 and xz only and
requires the corresponding `gzip`, `bzip2` and `xz` binaries to be installed on
your system. (That is, ripgrep does decompression by shelling out to another
process.)

ripgrep currently does not search archive formats, so `*.tar.gz` files, for
example, are skipped.


<h3 name="multiline">
How do I search over multiple lines?
</h3>

This isn't currently possible. ripgrep is fundamentally a line-oriented search
tool. With that said,
[multiline search is a planned opt-in feature](https://github.com/BurntSushi/ripgrep/issues/176).


<h3 name="fancy">
How do I use lookaround and/or backreferences?
</h3>

ripgrep's default regex engine does not support lookaround or backreferences.
This is primarily because the default regex engine is implemented using finite
state machines in order to guarantee a linear worst case time complexity on all
inputs. Backreferences are not possible to implement in this paradigm, and
lookaround appears difficult to do efficiently.

However, ripgrep optionally supports using PCRE2 as the regex engine instead of
the default one based on finite state machines. You can enable PCRE2 with the
`-P/--pcre2` flag. For example, in the root of the ripgrep repo, you can easily
find all palindromes:

```
$ rg -P '(\w{10})\1'
tests/misc.rs
483:    cmd.arg("--max-filesize").arg("44444444444444444444");
globset/src/glob.rs
1206:    matches!(match7, "a*a*a*a*a*a*a*a*a", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
```

If your version of ripgrep doesn't support PCRE2, then you'll get an error
message when you try to use the `-P/--pcre2` flag:

```
$ rg -P '(\w{10})\1'
PCRE2 is not available in this build of ripgrep
```

Most of the releases distributed by the ripgrep project here on GitHub will
come bundled with PCRE2 enabled. If you installed ripgrep through a different
means (like your system's package manager), then please reach out to the
maintainer of that package to see whether it's possible to enable the PCRE2
feature.


<h3 name="colors">
How do I configure ripgrep's colors?
</h3>

ripgrep has two flags related to colors:

* `--color` controls *when* to use colors.
* `--colors` controls *which* colors to use.

The `--color` flag accepts one of the following possible values: `never`,
`auto`, `always` or `ansi`. The `auto` value is the default and will cause
ripgrep to only enable colors when it is printing to a terminal. But if you
pipe ripgrep to a file or some other process, then it will suppress colors.

The --colors` flag is a bit more complicated. The general format is:

```
--colors '{type}:{attribute}:{value}'
```

* `{type}` should be one of `path`, `line`, `column` or `match`. Each of these
  correspond to the four different types of things that ripgrep will add color
  to in its output. Select the type whose color you want to change.
* `{attribute}` should be one of `fg`, `bg` or `style`, corresponding to
  foreground color, background color, or miscellaneous styling (such as whether
  to bold the output or not).
* `{value}` is determined by the value of `{attribute}`. If
  `{attribute}` is `style`, then `{value}` should be one of `nobold`,
  `bold`, `nointense`, `intense`, `nounderline` or `underline`. If
  `{attribute}` is `fg` or `bg`, then `{value}` should be a color.

A color is specified by either one of eight of English names, a single 256-bit
number or an RGB triple (with over 16 million possible values, or "true
color").

The color names are `red`, `blue`, `green`, `cyan`, `magenta`, `yellow`,
`white` or `black`.

A single 256-bit number is a value in the range 0-255 (inclusive). It can
either be in decimal format (e.g., `62`) or hexadecimal format (e.g., `0x3E`).

An RGB triple corresponds to three numbers (decimal or hexadecimal) separated
by commas.

As a special case, `--colors '{type}:none'` will clear all colors and styles
associated with `{type}`, which lets you start with a clean slate (instead of
building on top of ripgrep's default color settings).

Here's an example that makes highlights the matches with a nice blue background
with bolded white text:

```
$ rg somepattern \
    --colors 'match:none' \
    --colors 'match:bg:0x33,0x66,0xFF' \
    --colors 'match:fg:white' \
    --colors 'match:style:bold'
```

Colors are an ideal candidate to set in your
[configuration file](GUIDE.md#configuration-file). See the
[question on emulating The Silver Searcher's output style](#silver-searcher-output)
for an example specific to colors.


<h3 name="truecolors-windows">
How do I enable true colors on Windows?
</h3>

First, see the previous question's
[answer on configuring colors](#colors).

Secondly, coloring on Windows is a bit complicated. If you're using a terminal
like Cygwin, then it's likely true color support already works out of the box.
However, if you are using a normal Windows console (`cmd` or `PowerShell`) and
a version of Windows prior to 10, then there is no known way to get true
color support. If you are on Windows 10 and using a Windows console, then
true colors should work out of the box with one caveat: you might need to
clear ripgrep's default color settings first. That is, instead of this:

```
$ rg somepattern --colors 'match:fg:0x33,0x66,0xFF'
```

you should do this

```
$ rg somepattern --colors 'match:none' --colors 'match:fg:0x33,0x66,0xFF'
```

This is because ripgrep might set the default style for `match` to `bold`, and
it seems like Windows 10's VT100 support doesn't permit bold and true color
ANSI escapes to be used simultaneously. The work-around above will clear
ripgrep's default styling, allowing you to craft it exactly as desired.


<h3 name="stop-ripgrep">
How do I stop ripgrep from messing up colors when I kill it?
</h3>

Type in `color` in cmd.exe (Command Prompt) and `echo -ne "\033[0m"` on
Unix-like systems to restore your original foreground color.

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
available
[here](https://github.com/BurntSushi/ripgrep/issues/281#issuecomment-269093893).


<h3 name="size-limit">
How do I get around the regex size limit?
</h3>

If you've given ripgrep a particularly large pattern (or a large number of
smaller patterns), then it is possible that it will fail to compile because it
hit a pre-set limit. For example:

```
$ rg '\pL{1000}'
Compiled regex exceeds size limit of 10485760 bytes.
```

(Note: `\pL{1000}` may look small, but `\pL` is the character class containing
all Unicode letters, which is quite large. *And* it's repeated 1000 times.)

In this case, you can work around by simply increasing the limit:

```
$ rg '\pL{1000}' --regex-size-limit 1G
```

Increasing the limit to 1GB does not necessarily mean that ripgrep will use
that much memory. The limit just says that it's allowed to (approximately) use
that much memory for constructing the regular expression.


<h3 name="dfa-size">
How do I make the <code>-f/--file</code> flag faster?
</h3>

The `-f/--file` permits one to give a file to ripgrep which contains a pattern
on each line. ripgrep will then report any line that matches any of the
patterns.

If this pattern file gets too big, then it is possible ripgrep will slow down
dramatically. *Typically* this is because an internal cache is too small, and
will cause ripgrep to spill over to a slower but more robust regular expression
engine. If this is indeed the problem, then it is possible to increase this
cache and regain speed. The cache can be controlled via the `--dfa-size-limit`
flag. For example, using `--dfa-size-limit 1G` will set the cache size to 1GB.
(Note that this doesn't mean ripgrep will use 1GB of memory automatically, but
it will allow the regex engine to if it needs to.)


<h3 name="silver-searcher-output">
How do I make the output look like The Silver Searcher's output?
</h3>

Use the `--colors` flag, like so:

```
rg --colors line:fg:yellow      \
   --colors line:style:bold     \
   --colors path:fg:green       \
   --colors path:style:bold     \
   --colors match:fg:black      \
   --colors match:bg:yellow     \
   --colors match:style:nobold  \
   foo
```

Alternatively, add your color configuration to your ripgrep config file (which
is activated by setting the `RIPGREP_CONFIG_PATH` environment variable to point
to your config file). For example:

```
$ cat $HOME/.config/ripgrep/rc
--colors=line:fg:yellow
--colors=line:style:bold
--colors=path:fg:green
--colors=path:style:bold
--colors=match:fg:black
--colors=match:bg:yellow
--colors=match:style:nobold
$ RIPGREP_CONFIG_PATH=$HOME/.config/ripgrep/rc rg foo
```


<h3 name="rg-other-cmd">
When I run <code>rg</code>, why does it execute some other command?
</h3>

It's likely that you have a shell alias or even another tool called `rg` which
is interfering with ripgrep. Run `which rg` to see what it is.

(Notably, the Rails plug-in for
[Oh My Zsh](https://github.com/robbyrussell/oh-my-zsh/wiki/Plugins#rails) sets
up an `rg` alias for `rails generate`.)

Problems like this can be resolved in one of several ways:

* If you're using the OMZ Rails plug-in, disable it by editing the `plugins`
  array in your zsh configuration.
* Temporarily bypass an existing `rg` alias by calling ripgrep as
  `command rg`, `\rg`, or `'rg'`.
* Temporarily bypass an existing alias or another tool named `rg` by calling
  ripgrep by its full path (e.g., `/usr/bin/rg` or `/usr/local/bin/rg`).
* Permanently disable an existing `rg` alias by adding `unalias rg` to the
  bottom of your shell configuration file (e.g., `.bash_profile` or `.zshrc`).
* Give ripgrep its own alias that doesn't conflict with other tools/aliases by
  adding a line like the following to the bottom of your shell configuration
  file: `alias ripgrep='command rg'`.


<h3 name="rg-alias-windows">
How do I create an alias for ripgrep on Windows?
</h3>

Often you can find a need to make alias for commands you use a lot that set
certain flags. But PowerShell function aliases do not behave like your typical
linux shell alias. You always need to propagate arguments and `stdin` input.
But it cannot be done simply as
`function grep() { $input | rg.exe --hidden $args }`

Use below example as reference to how setup alias in PowerShell.

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

PowerShell special variables:

* input - is powershell `stdin` object that allows you to access its content.
* args - is array of arguments passed to this function.

This alias checks whether there is `stdin` input and propagates only if there
is some lines. Otherwise empty `$input` will make powershell to trigger `rg` to
search empty `stdin`.


<h3 name="powershell-profile">
How do I create a PowerShell profile?
</h3>

To customize powershell on start-up, there is a special PowerShell script that
has to be created. In order to find its location, type `$profile`.
See
[Microsoft's documentation](https://technet.microsoft.com/en-us/library/bb613488(v=vs.85).aspx)
for more details.

Any PowerShell code in this file gets evaluated at the start of console. This
way you can have own aliases to be created at start.


<h3 name="pipe-non-ascii-windows">
How do I pipe non-ASCII content to ripgrep on Windows?
</h3>

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

<h3 name="search-and-replace">
How can I search and replace with ripgrep?
</h3>

Using ripgrep alone, you can't. ripgrep is a search tool that will never
touch your files. However, the output of ripgrep can be piped to other tools
that do modify files on disk. See
[this issue](https://github.com/BurntSushi/ripgrep/issues/74) for more
information.

sed is one such tool that can modify files on disk. sed can take a filename
and a substitution command to search and replace in the specified file.
Files containing matching patterns can be provided to sed using

```
rg foo --files-with-matches
```

The output of this command is a list of filenames that contain a match for
the `foo` pattern.

This list can be piped into `xargs`, which will split the filenames from
standard input into arguments for the command following xargs. You can use this
combination to pipe a list of filenames into sed for replacement. For example:

```
rg foo --files-with-matches | xargs sed -i 's/foo/bar/g'
```

will replace all instances of 'foo' with 'bar' in the files in which
ripgrep finds the foo pattern. The `-i` flag to sed indicates that you are
editing files in place, and `s/foo/bar/g` says that you are performing a
**s**ubstitution of the pattren `foo` for `bar`, and that you are doing this
substitution **g**lobally (all occurrences of the pattern in each file).

Note: the above command assumes that you are using GNU sed. If you are using
BSD sed (the default on macOS and FreeBSD) then you must modify the above
command to be the following:

```
rg foo --files-with-matches | xargs sed -i '' 's/foo/bar/g'
```

The `-i` flag in BSD sed requires a file extension to be given to make backups
for all modified files. Specifying the empty string prevents file backups from
being made.

Finally, if any of your file paths contain whitespace in them, then you might
need to delimit your file paths with a NUL terminator. This requires telling
ripgrep to output NUL bytes between each path, and telling xargs to read paths
delimited by NUL bytes:

```
rg foo --files-with-matches -0 | xargs -0 sed -i 's/foo/bar/g'
```

To learn more about sed, see the sed manual
[here](https://www.gnu.org/software/sed/manual/sed.html).

Additionally, Facebook has a tool called
[fastmod](https://github.com/facebookincubator/fastmod)
that uses some of the same libraries as ripgrep and might provide a more
ergonomic search-and-replace experience.


<h3 name="license">
How is ripgrep licensed?
</h3>

ripgrep is dual licensed under the
[Unlicense](https://unlicense.org/)
and MIT licenses. Specifically, you may use ripgrep under the terms of either
license.

The reason why ripgrep is dual licensed this way is two-fold:

1. I, as ripgrep's author, would like to participate in a small bit of
   ideological activism by promoting the Unlicense's goal: to disclaim
   copyright monopoly interest.
2. I, as ripgrep's author, would like as many people to use rigprep as
   possible. Since the Unlicense is not a proven or well known license, ripgrep
   is also offered under the MIT license, which is ubiquitous and accepted by
   almost everyone.

More specifically, ripgrep and all its dependencies are compatible with this
licensing choice. In particular, ripgrep's dependencies (direct and transitive)
will always be limited to permissive licenses. That is, ripgrep will never
depend on code that is not permissively licensed. This means rejecting any
dependency that uses a copyleft license such as the GPL, LGPL, MPL or any of
the Creative Commons ShareAlike licenses. Whether the license is "weak"
copyleft or not does not matter; ripgrep will **not** depend on it.


<h3 name="posix4ever">
Can ripgrep replace grep?
</h3>

Yes and no.

If, upon hearing that "ripgrep can replace grep," you *actually* hear, "ripgrep
can be used in every instance grep can be used, in exactly the same way, for
the same use cases, with exactly the same bug-for-bug behavior," then no,
ripgrep trivially *cannot* replace grep. Moreover, ripgrep will *never* replace
grep.

If, upon hearing that "ripgrep can replace grep," you *actually* hear, "ripgrep
can replace grep in some cases and not in other use cases," then yes, that is
indeed true!

Let's go over some of those use cases in favor of ripgrep. Some of these may
not apply to you. That's OK. There may be other use cases not listed here that
do apply to you. That's OK too.

(For all claims related to performance in the following words, see my
[blog post](https://blog.burntsushi.net/ripgrep/)
introducing ripgrep.)

* Are you frequently searching a repository of code? If so, ripgrep might be a
  good choice since there's likely a good chunk of your repository that you
  don't want to search. grep, can, of course, be made to filter files using
  recursive search, and if you don't mind writing out the requisite `--exclude`
  rules or writing wrapper scripts, then grep might be sufficient. (I'm not
  kidding, I myself did this with grep for almost a decade before writing
  ripgrep.) But if you instead enjoy having a search tool respect your
  `.gitignore`, then ripgrep might be perfect for you!
* Are you frequently searching non-ASCII text that is UTF-8 encoded? One of
  ripgrep's key features is that it can handle Unicode features in your
  patterns in a way that tends to be faster than GNU grep. Unicode features
  in ripgrep are enabled by default; there is no need to configure your locale
  settings to use ripgrep properly because ripgrep doesn't respect your locale
  settings.
* Do you need to search UTF-16 files and you don't want to bother explicitly
  transcoding them? Great. ripgrep does this for you automatically. No need
  to enable it.
* Do you need to search a large directory of large files? ripgrep uses
  parallelism by default, which tends to make it faster than a standard
  `grep -r` search. However, if you're OK writing the occasional
  `find ./ -print0 | xargs -P8 -0 grep` command, then maybe grep is good
  enough.

Here are some cases where you might *not* want to use ripgrep. The same caveats
for the previous section apply.

* Are you writing portable shell scripts intended to work in a variety of
  environments? Great, probably not a good idea to use ripgrep! ripgrep is has
  nowhere near the ubquity of grep, so if you do use ripgrep, you might need
  to futz with the installation process more than you would with grep.
* Do you care about POSIX compatibility? If so, then you can't use ripgrep
  because it never was, isn't and never will be POSIX compatible.
* Do you hate tools that try to do something smart? If so, ripgrep is all about
  being smart, so you might prefer to just stick with grep.
* Is there a particular feature of grep you rely on that ripgrep either doesn't
  have or never will have? If the former, file a bug report, maybe ripgrep can
  do it! If the latter, well, then, just use grep.


<h3 name="intentcountsforsomething">
What does the "rip" in ripgrep mean?
</h3>

When I first started writing ripgrep, I called it `rep`, intending it to be a
shorter variant of `grep`. Soon after, I renamed it to `xrep` since `rep`
wasn't obvious enough of a name for my taste. And also because adding `x` to
anything always makes it better, right?

Before ripgrep's first public release, I decided that I didn't like `xrep`. I
thought it was slightly awkward to type, and despite my previous praise of the
letter `x`, I kind of thought it was pretty lame. Being someone who really
likes Rust, I wanted to call it "rustgrep" or maybe "rgrep" for short. But I
thought that was just as lame, and maybe a little too in-your-face. But I
wanted to continue using `r` so I could at least pretend Rust had something to
do with it.

I spent a couple of days trying to think of very short words that began with
the letter `r` that were even somewhat related to the task of searching. I
don't remember how it popped into my head, but "rip" came up as something that
meant "fast," as in, "to rip through your text." The fact that RIP is also
an initialism for "Rest in Peace" (as in, "ripgrep kills grep") never really
dawned on me. Perhaps the coincidence is too striking to believe that, but
I didn't realize it until someone explicitly pointed it out to me after the
initial public release. I admit that I found it mildly amusing, but if I had
realized it myself before the public release, I probably would have pressed on
and chose a different name. Alas, renaming things after a release is hard, so I
decided to mush on.

Given the fact that
[ripgrep never was, is or will be a 100% drop-in replacement for
grep](#posix4ever),
ripgrep is neither actually a "grep killer" nor was it ever intended to be. It
certainly does eat into some of its use cases, but that's nothing that other
tools like ack or The Silver Searcher weren't already doing.
