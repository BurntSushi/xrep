# NAME

rg - recursively search current directory for lines matching a pattern

# SYNOPSIS

rg [*options*] <*pattern*> [*<*path*> ...*]

rg [*options*] (-e PATTERN | -f FILE) ... [*<*path*> ...*]

rg [*options*] --files [*<*path*> ...*]

rg [*options*] --type-list

rg [*options*] --help

rg [*options*] --version

# DESCRIPTION

ripgrep (rg) combines the usability of The Silver Searcher (an ack clone) with
the raw speed of grep.

ripgrep's regex engine uses finite automata and guarantees linear time
searching. Because of this, features like backreferences and arbitrary
lookaround are not supported.

Project home page: https://github.com/BurntSushi/ripgrep

# COMMON OPTIONS

-a, --text
: Search binary files as if they were text.

-c, --count
: Only show count of line matches for each file.

--color *WHEN*
: Whether to use coloring in match. Valid values are never, always or auto.
  [default: auto]

-e, --regexp *PATTERN* ...
: Use PATTERN to search. This option can be provided multiple times, where all
  patterns given are searched. This is also useful when searching for patterns
  that start with a dash.

-F, --fixed-strings
: Treat the pattern as a literal string instead of a regular expression.

-g, --glob *GLOB* ...
: Include or exclude files for searching that match the given glob. This always
  overrides any other ignore logic if there is a conflict, but is otherwise
  applied in addition to ignore files (e.g., .gitignore or .ignore). Multiple
  glob flags may be used. Globbing rules match .gitignore globs. Precede a
  glob with a '!' to exclude it.

  The --glob flag subsumes the functionality of both the --include and
  --exclude flags commonly found in other tools.

    Values given to -g must be quoted or your shell will expand them and result
    in unexpected behavior.

    Combine with the --files flag to return matched filenames
    (i.e., to replicate ack/ag's -g flag).

    For example: rg -g '\<glob\>' --files

-h, --help
: Show this usage message.

-i, --ignore-case
: Case insensitive search. Overridden by --case-sensitive.

-n, --line-number
: Show line numbers (1-based). This is enabled by default at a tty.

-N, --no-line-number
: Suppress line numbers.

-q, --quiet
: Do not print anything to stdout. If a match is found in a file, stop
  searching that file.

-t, --type *TYPE* ...
: Only search files matching TYPE. Multiple type flags may be provided. Use the
  --type-list flag to list all available types.

-T, --type-not *TYPE* ...
: Do not search files matching TYPE. Multiple not-type flags may be provided.

-u, --unrestricted ...
: Reduce the level of 'smart' searching. A single -u doesn't respect .gitignore
  (etc.) files. Two -u flags will search hidden files and directories. Three
  -u flags will search binary files. -uu is equivalent to grep -r, and -uuu is
  equivalent to grep -a -r.

    Note that the -u flags are convenient aliases for other combinations of
    flags. -u aliases '--no-ignore'. -uu aliases '--no-ignore --hidden'.
    -uuu aliases '--no-ignore --hidden --text'.

-v, --invert-match
: Invert matching.

-w, --word-regexp
: Only show matches surrounded by word boundaries. This is equivalent to
  putting \\b before and after the search pattern.

# LESS COMMON OPTIONS

-A, --after-context *NUM*
: Show NUM lines after each match.

-B, --before-context *NUM*
: Show NUM lines before each match.

-C, --context *NUM*
: Show NUM lines before and after each match.

--colors *SPEC* ...
: This flag specifies color settings for use in the output. This flag may be
  provided multiple times. Settings are applied iteratively. Colors are limited
  to one of eight choices: red, blue, green, cyan, magenta, yellow, white and
  black. Styles are limited to nobold, bold, nointense or intense.

    The format of the flag is {type}:{attribute}:{value}. {type} should be one
    of path, line or match. {attribute} can be fg, bg or style. Value is either
    a color (for fg and bg) or a text style. A special format, {type}:none,
    will clear all color settings for {type}.

    For example, the following command will change the match color to magenta
    and the background color for line numbers to yellow:

    rg --colors 'match:fg:magenta' --colors 'line:bg:yellow' foo.

--column
: Show column numbers (1 based) in output. This only shows the column
  numbers for the first match on each line. Note that this doesn't try
  to account for Unicode. One byte is equal to one column. This implies
  --line-number.

--context-separator *SEPARATOR*
: The string to use when separating non-continuous context lines. Escape
  sequences may be used. [default: --]

--debug
: Show debug messages.

-E, --encoding *ENCODING*
: Specify the text encoding that ripgrep will use on all files
  searched. The default value is 'auto', which will cause ripgrep to do
  a best effort automatic detection of encoding on a per-file basis.
  Other supported values can be found in the list of labels here:
  https://encoding.spec.whatwg.org/#concept-encoding-get

-f, --file FILE ...
: Search for patterns from the given file, with one pattern per line. When this
  flag is used or multiple times or in combination with the -e/--regexp flag,
  then all patterns provided are searched. Empty pattern lines will match all
  input lines, and the newline is not counted as part of the pattern.

--files
: Print each file that would be searched (but don't search).

    Combine with the -g flag to return matched paths, for example:

    rg -g '\<glob\>' --files

-l, --files-with-matches
: Only show path of each file with matches.

--files-without-match
: Only show path of each file with no matches.

-H, --with-filename
: Prefix each match with the file name that contains it. This is the
  default when more than one file is searched.

--no-filename
: Never show the filename for a match. This is the default when
  one file is searched.

--heading
: Show the file name above clusters of matches from each file instead of
  showing the file name for every match. This is the default mode at a tty.

--no-heading
: Don't group matches by each file. If -H/--with-filename is enabled, then
  file names will be shown for every line matched. This is the default more
  when not at a tty.

--hidden
: Search hidden directories and files. (Hidden directories and files are
  skipped by default.)

--ignore-file FILE ...
: Specify additional ignore files for filtering file paths.
  Ignore files should be in the gitignore format and are matched
  relative to the current working directory. These ignore files
  have lower precedence than all other ignore files. When
  specifying multiple ignore files, earlier files have lower
  precedence than later files.

-L, --follow
: Follow symlinks.

-M, --max-columns *NUM*
: Don't print lines longer than this limit in bytes. Longer lines are omitted,
  and only the number of matches in that line is printed.

-m, --max-count *NUM*
: Limit the number of matching lines per file searched to NUM.

--max-filesize *NUM*+*SUFFIX*?
: Ignore files larger than *NUM* in size. Directories will never be ignored.

    *SUFFIX* is optional and may be one of K, M or G. These correspond to
    kilobytes, megabytes and gigabytes respectively. If omitted the input is
    treated as bytes.

--maxdepth *NUM*
: Descend at most NUM directories below the command line arguments.
  A value of zero searches only the starting-points themselves.

--mmap
: Search using memory maps when possible. This is enabled by default
  when ripgrep thinks it will be faster. (Note that mmap searching
  doesn't currently support the various context related options.)

--no-messages
: Suppress all error messages.

--no-mmap
: Never use memory maps, even when they might be faster.

--no-ignore
: Don't respect ignore files (.gitignore, .ignore, etc.)
  This implies --no-ignore-parent.

--no-ignore-parent
: Don't respect ignore files in parent directories.

--no-ignore-vcs
: Don't respect version control ignore files (e.g., .gitignore).
  Note that .ignore files will continue to be respected.

--null
: Whenever a file name is printed, follow it with a NUL byte.
  This includes printing filenames before matches, and when printing
  a list of matching files such as with --count, --files-with-matches
  and --files.

--path-separator *SEPARATOR*
: The path separator to use when printing file paths. This defaults to your
  platform's path separator, which is / on Unix and \\ on Windows. This flag is
  intended for overriding the default when the environment demands it (e.g.,
  cygwin). A path separator is limited to a single byte.

-p, --pretty
: Alias for --color=always --heading -n.

-r, --replace *ARG*
: Replace every match with the string given when printing search results.
  Neither this flag nor any other flag will modify your files.

    Capture group indices (e.g., $5) and names (e.g., $foo) are supported
    in the replacement string.

    Note that the replacement by default replaces each match, and NOT the
    entire line. To replace the entire line, you should match the entire line.
    For example, to emit only the first phone numbers in each line:

        rg '^.*([0-9]{3}-[0-9]{3}-[0-9]{4}).*$' --replace '$1'

-s, --case-sensitive
: Search case sensitively. This overrides --ignore-case and --smart-case.

-S, --smart-case
: Search case insensitively if the pattern is all lowercase.
  Search case sensitively otherwise. This is overridden by either
  --case-sensitive or --ignore-case.

--sort-files
: Sort results by file path. Note that this currently
  disables all parallelism and runs search in a single thread.

-j, --threads *ARG*
: The number of threads to use. 0 means use the number of logical CPUs
  (capped at 12). [default: 0]

--version
: Show the version number of ripgrep and exit.

--vimgrep
: Show results with every match on its own line, including line
  numbers and column numbers. (With this option, a line with more
  than one match of the regex will be printed more than once.)

# FILE TYPE MANAGEMENT OPTIONS

--type-list
: Show all supported file types and their associated globs.

--type-add *ARG* ...
: Add a new glob for a particular file type. Only one glob can be added
  at a time. Multiple --type-add flags can be provided. Unless --type-clear
  is used, globs are added to any existing globs inside of ripgrep. Note that
  this must be passed to every invocation of rg. Type settings are NOT
  persisted.

          Example: `rg --type-add 'foo:*.foo' -tfoo PATTERN`

      --type-add can also be used to include rules from other types
      with the special include directive. The include directive
      permits specifying one or more other type names (separated by a
      comma) that have been defined and its rules will automatically
      be imported into the type specified. For example, to create a
      type called src that matches C++, Python and Markdown files, one
      can use:

          `--type-add 'src:include:cpp,py,md'`

      Additional glob rules can still be added to the src type by
      using the --type-add flag again:

          `--type-add 'src:include:cpp,py,md' --type-add 'src:*.foo'`

      Note that type names must consist only of Unicode letters or
      numbers. Punctuation characters are not allowed.

--type-clear *TYPE* ...
: Clear the file type globs previously defined for TYPE. This only clears
  the default type definitions that are found inside of ripgrep. Note
  that this must be passed to every invocation of rg.

# SHELL COMPLETION

Shell completion files are included in the release tarball for Bash, Fish, Zsh
and PowerShell.

For **bash**, move `rg.bash-completion` to `$XDG_CONFIG_HOME/bash_completion`
or `/etc/bash_completion.d/`.

For **fish**, move `rg.fish` to `$HOME/.config/fish/completions`.
