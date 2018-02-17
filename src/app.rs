// This module defines the set of command line arguments that ripgrep supports,
// including some light validation.
//
// This module is purposely written in a bare-bones way, since it is included
// in ripgrep's build.rs file as a way to generate completion files for common
// shells.
//
// The only other place that ripgrep deals with clap is in src/args.rs, which
// is where we read clap's configuration from the end user's arguments and turn
// it into a ripgrep-specific configuration type that is not coupled with clap.

use clap::{self, App, AppSettings};

const ABOUT: &str = "
ripgrep (rg) recursively searches your current directory for a regex pattern.
By default, ripgrep will respect your `.gitignore` and automatically skip
hidden files/directories and binary files.

ripgrep's regex engine uses finite automata and guarantees linear time
searching. Because of this, features like backreferences and arbitrary
lookaround are not supported.

ripgrep supports configuration files. Set RIPGREP_CONFIG_PATH to a
configuration file. The file can specify one shell argument per line. Lines
starting with '#' are ignored. For more details, see the man page or the
README.

Project home page: https://github.com/BurntSushi/ripgrep

Use -h for short descriptions and --help for more details.";

const USAGE: &str = "
    rg [OPTIONS] PATTERN [PATH ...]
    rg [OPTIONS] [-e PATTERN ...] [-f FILE ...] [PATH ...]
    rg [OPTIONS] --files [PATH ...]
    rg [OPTIONS] --type-list";

const TEMPLATE: &str = "\
{bin} {version}
{author}
{about}

USAGE:{usage}

ARGS:
{positionals}

OPTIONS:
{unified}";

/// Build a clap application parameterized by usage strings.
pub fn app() -> App<'static, 'static> {
    // We need to specify our version in a static because we've painted clap
    // into a corner. We've told it that every string we give it will be
    // 'static, but we need to build the version string dynamically. We can
    // fake the 'static lifetime with lazy_static.
    lazy_static! {
        static ref LONG_VERSION: String = long_version(None);
    }

    let mut app = App::new("ripgrep")
        .author(crate_authors!())
        .version(crate_version!())
        .long_version(LONG_VERSION.as_str())
        .about(ABOUT)
        .max_term_width(100)
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::AllArgsOverrideSelf)
        .usage(USAGE)
        .template(TEMPLATE)
        .help_message("Prints help information. Use --help for more details.");
    for arg in all_args_and_flags() {
        app = app.arg(arg.claparg);
    }
    app
}

/// Return the "long" format of ripgrep's version string.
///
/// If a revision hash is given, then it is used. If one isn't given, then
/// the RIPGREP_BUILD_GIT_HASH env var is inspect for it. If that isn't set,
/// then a revision hash is not included in the version string returned.
pub fn long_version(revision_hash: Option<&str>) -> String {
    // Let's say whether faster CPU instructions are enabled or not.
    let mut features = vec![];
    if cfg!(feature = "simd-accel") {
        features.push("+SIMD");
    } else {
        features.push("-SIMD");
    }
    if cfg!(feature = "avx-accel") {
        features.push("+AVX");
    } else {
        features.push("-AVX");
    }
    // Do we have a git hash?
    // (Yes, if ripgrep was built on a machine with `git` installed.)
    let hash = match revision_hash.or(option_env!("RIPGREP_BUILD_GIT_HASH")) {
        None => String::new(),
        Some(githash) => format!(" (rev {})", githash),
    };
    // Put everything together.
    format!("{}{}\n{}", crate_version!(), hash, features.join(" "))
}

/// Arg is a light alias for a clap::Arg that is specialized to compile time
/// string literals.
type Arg = clap::Arg<'static, 'static>;

/// RGArg is a light wrapper around a clap::Arg and also contains some metadata
/// about the underlying Arg so that it can be inspected for other purposes
/// (e.g., hopefully generating a man page).
///
/// Note that this type is purposely overly constrained to ripgrep's particular
/// use of clap.
#[allow(dead_code)]
#[derive(Clone)]
pub struct RGArg {
    /// The underlying clap argument.
    claparg: Arg,
    /// The name of this argument. This is always present and is the name
    /// used in the code to find the value of an argument at runtime.
    pub name: &'static str,
    /// A short documentation string describing this argument. This string
    /// should fit on a single line and be a complete sentence.
    ///
    /// This is shown in the `-h` output.
    pub doc_short: &'static str,
    /// A longer documentation string describing this argument. This usually
    /// starts with the contents of `doc_short`. This is also usually many
    /// lines, potentially paragraphs, and may contain examples and additional
    /// prose.
    ///
    /// This is shown in the `--help` output.
    pub doc_long: &'static str,
    /// Whether this flag is hidden or not.
    ///
    /// This is typically used for uncommon flags that only serve to override
    /// other flags. For example, --no-ignore is a prominent flag that disables
    /// ripgrep's gitignore functionality, but --ignore re-enables it. Since
    /// gitignore support is enabled by default, use of the --ignore flag is
    /// somewhat niche and relegated to special cases when users make use of
    /// configuration files to set defaults.
    ///
    /// Generally, these flags should be documented in the documentation for
    /// the flag they override.
    pub hidden: bool,
    /// The type of this argument.
    pub kind: RGArgKind,
}

/// The kind of a ripgrep argument.
///
/// This can be one of three possibilities: a positional argument, a boolean
/// switch flag or a flag that accepts exactly one argument. Each variant
/// stores argument type specific data.
///
/// Note that clap supports more types of arguments than this, but we don't
/// (and probably shouldn't) use them in ripgrep.
///
/// Finally, note that we don't capture *all* state about an argument in this
/// type. Some state is only known to clap. There isn't any particular reason
/// why; the state we do capture is motivated by use cases (like generating
/// documentation).
#[derive(Clone)]
pub enum RGArgKind {
    /// A positional argument.
    Positional {
        /// The name of the value used in the `-h/--help` output. By
        /// convention, this is an all-uppercase string. e.g., `PATH` or
        /// `PATTERN`.
        value_name: &'static str,
        /// Whether an argument can be repeated multiple times or not.
        ///
        /// The only argument this applies to is PATH, where an end user can
        /// specify multiple paths for ripgrep to search.
        ///
        /// If this is disabled, then an argument can only be provided once.
        /// For example, PATTERN is one such argument. (Note that the
        /// -e/--regexp flag is distinct from the positional PATTERN argument,
        /// and it can be provided multiple times.)
        multiple: bool,
    },
    /// A boolean switch.
    Switch {
        /// The long name of a flag. This is always non-empty.
        long: &'static str,
        /// The short name of a flag. This is empty if a flag only has a long
        /// name.
        short: Option<&'static str>,
        /// Whether this switch can be provided multiple times where meaning
        /// is attached to the number of times this flag is given.
        ///
        /// Note that every switch can be provided multiple times. This
        /// particular state indicates whether all instances of a switch are
        /// relevant or not.
        ///
        /// For example, the -u/--unrestricted flag can be provided multiple
        /// times where each repeated use of it indicates more relaxing of
        /// ripgrep's filtering. Conversely, the -i/--ignore-case flag can
        /// also be provided multiple times, but it is simply considered either
        /// present or not. In these cases, -u/--unrestricted has `multiple`
        /// set to `true` while -i/--ignore-case has `multiple` set to `false`.
        multiple: bool,
    },
    /// A flag the accepts a single value.
    Flag {
        /// The long name of a flag. This is always non-empty.
        long: &'static str,
        /// The short name of a flag. This is empty if a flag only has a long
        /// name.
        short: Option<&'static str>,
        /// The name of the value used in the `-h/--help` output. By
        /// convention, this is an all-uppercase string. e.g., `PATH` or
        /// `PATTERN`.
        value_name: &'static str,
        /// Whether this flag can be provided multiple times with multiple
        /// distinct values.
        ///
        /// Note that every flag can be provided multiple times. This
        /// particular state indicates whether all instances of a flag are
        /// relevant or not.
        ///
        /// For example, the -g/--glob flag can be provided multiple times and
        /// all of its values should be interpreted by ripgrep. Conversely,
        /// while the -C/--context flag can also be provided multiple times,
        /// only its last instance is used while all previous instances are
        /// ignored. In these cases, -g/--glob has `multiple` set to `true`
        /// while -C/--context has `multiple` set to `false`.
        multiple: bool,
        /// A set of possible values for this flag. If an end user provides
        /// any value other than what's in this set, then clap will report an
        /// error.
        possible_values: Vec<&'static str>,
    }
}

impl RGArg {
    /// Create a positional argument.
    ///
    /// The `long_name` parameter is the name of the argument, e.g., `pattern`.
    /// The `value_name` parameter is a name that describes the type of
    /// argument this flag accepts. It should be in uppercase, e.g., PATH or
    /// PATTERN.
    fn positional(name: &'static str, value_name: &'static str) -> RGArg {
        RGArg {
            claparg: Arg::with_name(name).value_name(value_name),
            name: name,
            doc_short: "",
            doc_long: "",
            hidden: false,
            kind: RGArgKind::Positional {
                value_name: value_name,
                multiple: false,
            },
        }
    }

    /// Create a boolean switch.
    ///
    /// The `long_name` parameter is the name of the flag, e.g., `--long-name`.
    ///
    /// All switches may be repeated an arbitrary number of times. If a switch
    /// is truly boolean, that consumers of clap's configuration should only
    /// check whether the flag is present or not. Otherwise, consumers may
    /// inspect the number of times the switch is used.
    fn switch(long_name: &'static str) -> RGArg {
        let claparg = Arg::with_name(long_name)
            .long(long_name);
        RGArg {
            claparg: claparg,
            name: long_name,
            doc_short: "",
            doc_long: "",
            hidden: false,
            kind: RGArgKind::Switch {
                long: long_name,
                short: None,
                multiple: false,
            },
        }
    }

    /// Create a flag. A flag always accepts exactly one argument.
    ///
    /// The `long_name` parameter is the name of the flag, e.g., `--long-name`.
    /// The `value_name` parameter is a name that describes the type of
    /// argument this flag accepts. It should be in uppercase, e.g., PATH or
    /// PATTERN.
    ///
    /// All flags may be repeated an arbitrary number of times. If a flag has
    /// only one logical value, that consumers of clap's configuration should
    /// only use the last value.
    fn flag(long_name: &'static str, value_name: &'static str) -> RGArg {
        let claparg = Arg::with_name(long_name)
            .long(long_name)
            .value_name(value_name)
            .takes_value(true)
            .number_of_values(1);
        RGArg {
            claparg: claparg,
            name: long_name,
            doc_short: "",
            doc_long: "",
            hidden: false,
            kind: RGArgKind::Flag {
                long: long_name,
                short: None,
                value_name: value_name,
                multiple: false,
                possible_values: vec![],
            }
        }
    }

    /// Set the short flag name.
    ///
    /// This panics if this arg isn't a switch or a flag.
    fn short(mut self, name: &'static str) -> RGArg {
        match self.kind {
            RGArgKind::Positional{..} => panic!("expected switch or flag"),
            RGArgKind::Switch { ref mut short, .. } => {
                *short = Some(name);
            }
            RGArgKind::Flag { ref mut short, .. } => {
                *short = Some(name);
            }
        }
        self.claparg = self.claparg.short(name);
        self
    }

    /// Set the "short" help text.
    ///
    /// This should be a single line. It is shown in the `-h` output.
    fn help(mut self, text: &'static str) -> RGArg {
        self.doc_short = text;
        self.claparg = self.claparg.help(text);
        self
    }

    /// Set the "long" help text.
    ///
    /// This should be at least a single line, usually longer. It is shown in
    /// the `--help` output.
    fn long_help(mut self, text: &'static str) -> RGArg {
        self.doc_long = text;
        self.claparg = self.claparg.long_help(text);
        self
    }

    /// Enable this argument to accept multiple values.
    ///
    /// Note that while switches and flags can always be repeated an arbitrary
    /// number of times, this particular method enables the flag to be
    /// logically repeated where each occurrence of the flag may have
    /// significance. That is, when this is disabled, then a switch is either
    /// present or not and a flag has exactly one value (the last one given).
    /// When this is enabled, then a switch has a count corresponding to the
    /// number of times it is used and a flag's value is a list of all values
    /// given.
    ///
    /// For the most part, this distinction is resolved by consumers of clap's
    /// configuration.
    fn multiple(mut self) -> RGArg {
        // Why not put `multiple` on RGArg proper? Because it's useful to
        // document it distinct for each different kind. See RGArgKind docs.
        match self.kind {
            RGArgKind::Positional { ref mut multiple, .. } => {
                *multiple = true;
            }
            RGArgKind::Switch { ref mut multiple, .. } => {
                *multiple = true;
            }
            RGArgKind::Flag { ref mut multiple, .. } => {
                *multiple = true;
            }
        }
        self.claparg = self.claparg.multiple(true);
        self
    }

    /// Hide this flag from all documentation.
    fn hidden(mut self) -> RGArg {
        self.hidden = true;
        self.claparg = self.claparg.hidden(true);
        self
    }

    /// Set the possible values for this argument. If this argument is not
    /// a flag, then this panics.
    ///
    /// If the end user provides any value other than what is given here, then
    /// clap will report an error to the user.
    ///
    /// Note that this will suppress clap's automatic output of possible values
    /// when using -h/--help, so users of this method should provide
    /// appropriate documentation for the choices in the "long" help text.
    fn possible_values(mut self, values: &[&'static str]) -> RGArg {
        match self.kind {
            RGArgKind::Positional{..} => panic!("expected flag"),
            RGArgKind::Switch{..} => panic!("expected flag"),
            RGArgKind::Flag { ref mut possible_values, .. } => {
                *possible_values = values.to_vec();
                self.claparg = self.claparg
                    .possible_values(values)
                    .hide_possible_values(true);
            }
        }
        self
    }

    /// Add an alias to this argument.
    ///
    /// Aliases are not show in the output of -h/--help.
    fn alias(mut self, name: &'static str) -> RGArg {
        self.claparg = self.claparg.alias(name);
        self
    }

    /// Permit this flag to have values that begin with a hypen.
    ///
    /// This panics if this arg is not a flag.
    fn allow_leading_hyphen(mut self) -> RGArg {
        match self.kind {
            RGArgKind::Positional{..} => panic!("expected flag"),
            RGArgKind::Switch{..} => panic!("expected flag"),
            RGArgKind::Flag {..} => {
                self.claparg = self.claparg.allow_hyphen_values(true);
            }
        }
        self
    }

    /// Sets this argument to a required argument, unless one of the given
    /// arguments is provided.
    fn required_unless(mut self, names: &[&'static str]) -> RGArg {
        self.claparg = self.claparg.required_unless_one(names);
        self
    }

    /// Sets conflicting arguments. That is, if this argument is used whenever
    /// any of the other arguments given here are used, then clap will report
    /// an error.
    fn conflicts(mut self, names: &[&'static str]) -> RGArg {
        self.claparg = self.claparg.conflicts_with_all(names);
        self
    }

    /// Sets an overriding argument. That is, if this argument and the given
    /// argument are both provided by an end user, then the "last" one will
    /// win. ripgrep will behave as if any previous instantiations did not
    /// happen.
    fn overrides(mut self, name: &'static str) -> RGArg {
        self.claparg = self.claparg.overrides_with(name);
        self
    }

    /// Sets the default value of this argument if and only if the argument
    /// given is present.
    fn default_value_if(
        mut self,
        value: &'static str,
        arg_name: &'static str,
    ) -> RGArg {
        self.claparg = self.claparg.default_value_if(arg_name, None, value);
        self
    }

    /// Indicate that any value given to this argument should be a number. If
    /// it's not a number, then clap will report an error to the end user.
    fn number(mut self) -> RGArg {
        self.claparg = self.claparg.validator(|val| {
            val.parse::<usize>().map(|_| ()).map_err(|err| err.to_string())
        });
        self
    }

    /// Indicate that any value given to this argument should be a valid
    /// line number width. A valid line number width cannot start with `0`
    /// to maintain compatibility with future improvements that add support
    /// for padding character specifies.
    fn line_number_width(mut self) -> RGArg {
        self.claparg = self.claparg.validator(|val| {
            if val.starts_with("0") {
                Err(String::from(
                    "Custom padding characters are currently not supported. \
                     Please enter only a numeric value."))
            } else {
                val.parse::<usize>().map(|_| ()).map_err(|err| err.to_string())
            }
        });
        self
    }
}

// We add an extra space to long descriptions so that a black line is inserted
// between flag descriptions in --help output.
macro_rules! long {
    ($lit:expr) => { concat!($lit, " ") }
}

/// Generate a sequence of all positional and flag arguments.
pub fn all_args_and_flags() -> Vec<RGArg> {
    let mut args = vec![];
    // The positional arguments must be defined first and in order.
    arg_pattern(&mut args);
    arg_path(&mut args);
    // Flags can be defined in any order, but we do it alphabetically.
    flag_after_context(&mut args);
    flag_before_context(&mut args);
    flag_case_sensitive(&mut args);
    flag_color(&mut args);
    flag_colors(&mut args);
    flag_column(&mut args);
    flag_context(&mut args);
    flag_context_separator(&mut args);
    flag_count(&mut args);
    flag_debug(&mut args);
    flag_dfa_size_limit(&mut args);
    flag_encoding(&mut args);
    flag_file(&mut args);
    flag_files(&mut args);
    flag_files_with_matches(&mut args);
    flag_files_without_match(&mut args);
    flag_fixed_strings(&mut args);
    flag_follow(&mut args);
    flag_glob(&mut args);
    flag_heading(&mut args);
    flag_hidden(&mut args);
    flag_iglob(&mut args);
    flag_ignore_case(&mut args);
    flag_ignore_file(&mut args);
    flag_invert_match(&mut args);
    flag_line_number(&mut args);
    flag_line_number_width(&mut args);
    flag_line_regexp(&mut args);
    flag_max_columns(&mut args);
    flag_max_count(&mut args);
    flag_max_filesize(&mut args);
    flag_maxdepth(&mut args);
    flag_mmap(&mut args);
    flag_no_config(&mut args);
    flag_no_ignore(&mut args);
    flag_no_ignore_parent(&mut args);
    flag_no_ignore_vcs(&mut args);
    flag_no_messages(&mut args);
    flag_null(&mut args);
    flag_only_matching(&mut args);
    flag_path_separator(&mut args);
    flag_passthru(&mut args);
    flag_pretty(&mut args);
    flag_quiet(&mut args);
    flag_regex_size_limit(&mut args);
    flag_regexp(&mut args);
    flag_replace(&mut args);
    flag_search_zip(&mut args);
    flag_smart_case(&mut args);
    flag_sort_files(&mut args);
    flag_text(&mut args);
    flag_threads(&mut args);
    flag_type(&mut args);
    flag_type_add(&mut args);
    flag_type_clear(&mut args);
    flag_type_list(&mut args);
    flag_type_not(&mut args);
    flag_unrestricted(&mut args);
    flag_vimgrep(&mut args);
    flag_with_filename(&mut args);
    flag_word_regexp(&mut args);
    args
}

fn arg_pattern(args: &mut Vec<RGArg>) {
    const SHORT: &str = "A regular expression used for searching.";
    const LONG: &str = long!("\
A regular expression used for searching. To match a pattern beginning with a
dash, use the -e/--regexp flag.

For example, to search for the literal '-foo', you can use this flag:

    rg -e -foo

You can also use the special '--' delimiter to indicate that no more flags
will be provided. Namely, the following is equivalent to the above:

    rg -- -foo
");
    let arg = RGArg::positional("pattern", "PATTERN")
        .help(SHORT).long_help(LONG)
        .required_unless(&[
            "file", "files", "regexp", "type-list",
        ]);
    args.push(arg);
}

fn arg_path(args: &mut Vec<RGArg>) {
    const SHORT: &str = "A file or directory to search.";
    const LONG: &str = long!("\
A file or directory to search. Directories are searched recursively. Paths \
specified on the command line override glob and ignore rules. \
");
    let arg = RGArg::positional("path", "PATH")
        .help(SHORT).long_help(LONG)
        .multiple();
    args.push(arg);
}

fn flag_after_context(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show NUM lines after each match.";
    const LONG: &str = long!("\
Show NUM lines after each match.

This overrides the --context flag.
");
    let arg = RGArg::flag("after-context", "NUM").short("A")
        .help(SHORT).long_help(LONG)
        .number()
        .overrides("context");
    args.push(arg);
}

fn flag_before_context(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show NUM lines before each match.";
    const LONG: &str = long!("\
Show NUM lines before each match.

This overrides the --context flag.
");
    let arg = RGArg::flag("before-context", "NUM").short("B")
        .help(SHORT).long_help(LONG)
        .number()
        .overrides("context");
    args.push(arg);
}

fn flag_case_sensitive(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Search case sensitively (default).";
    const LONG: &str = long!("\
Search case sensitively.

This overrides the -i/--ignore-case and -S/--smart-case flags.
");
    let arg = RGArg::switch("case-sensitive").short("s")
        .help(SHORT).long_help(LONG)
        .overrides("ignore-case")
        .overrides("smart-case");
    args.push(arg);
}

fn flag_color(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Controls when to use color.";
    const LONG: &str = long!("\
This flag controls when to use colors. The default setting is 'auto', which
means ripgrep will try to guess when to use colors. For example, if ripgrep is
printing to a terminal, then it will use colors, but if it is redirected to a
file or a pipe, then it will suppress color output. ripgrep will suppress color
output in some other circumstances as well. For example, if the TERM
environment variable is not set or set to 'dumb', then ripgrep will not use
colors.

The possible values for this flag are:

    never    Colors will never be used.
    auto     The default. ripgrep tries to be smart.
    always   Colors will always be used regardless of where output is sent.
    ansi     Like 'always', but emits ANSI escapes (even in a Windows console).

When the --vimgrep flag is given to ripgrep, then the default value for the
--color flag changes to 'never'.
");
    let arg = RGArg::flag("color", "WHEN")
        .help(SHORT).long_help(LONG)
        .possible_values(&["never", "auto", "always", "ansi"])
        .default_value_if("never", "vimgrep");
    args.push(arg);
}

fn flag_colors(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Configure color settings and styles.";
    const LONG: &str = long!("\
This flag specifies color settings for use in the output. This flag may be
provided multiple times. Settings are applied iteratively. Colors are limited
to one of eight choices: red, blue, green, cyan, magenta, yellow, white and
black. Styles are limited to nobold, bold, nointense or intense.

The format of the flag is `{type}:{attribute}:{value}`. `{type}` should be
one of path, line, column or match. `{attribute}` can be fg, bg or style.
`{value}` is either a color (for fg and bg) or a text style. A special format,
`{type}:none`, will clear all color settings for `{type}`.

For example, the following command will change the match color to magenta and
the background color for line numbers to yellow:

    rg --colors 'match:fg:magenta' --colors 'line:bg:yellow' foo.

Extended colors can be used for `{value}` when the terminal supports ANSI color
sequences. These are specified as either 'x' (256-color) or 'x,x,x' (24-bit
truecolor) where x is a number between 0 and 255 inclusive. x may be given as
a normal decimal number or a hexadecimal number, which is prefixed by `0x`.

For example, the following command will change the match background color to
that represented by the rgb value (0,128,255):

    rg --colors 'match:bg:0,128,255'

or, equivalently,

    rg --colors 'match:bg:0x0,0x80,0xFF'

Note that the the intense and nointense style flags will have no effect when
used alongside these extended color codes.
");
    let arg = RGArg::flag("colors", "COLOR_SPEC")
        .help(SHORT).long_help(LONG)
        .multiple();
    args.push(arg);
}

fn flag_column(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show column numbers.";
    const LONG: &str = long!("\
Show column numbers (1-based). This only shows the column numbers for the first
match on each line. This does not try to account for Unicode. One byte is equal
to one column. This implies --line-number.
");
    let arg = RGArg::switch("column")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_context(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show NUM lines before and after each match.";
    const LONG: &str = long!("\
Show NUM lines before and after each match. This is equivalent to providing
both the -B/--before-context and -A/--after-context flags with the same value.

This overrides both the -B/--before-context and -A/--after-context flags.
");
    let arg = RGArg::flag("context", "NUM").short("C")
        .help(SHORT).long_help(LONG)
        .number()
        .overrides("before-context")
        .overrides("after-context");
    args.push(arg);
}

fn flag_context_separator(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Set the context separator string.";
    const LONG: &str = long!("\
The string used to separate non-contiguous context lines in the output. Escape
sequences like \\x7F or \\t may be used. The default value is --.
");
    let arg = RGArg::flag("context-separator", "SEPARATOR")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_count(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Only show the count of matches for each file.";
    const LONG: &str = long!("\
This flag suppresses normal output and shows the number of lines that match
the given patterns for each file searched. Each file containing a match has its
path and count printed on each line. Note that this reports the number of lines
that match and not the total number of matches.

If only one file is given to ripgrep, then only the count is printed if there
is a match. The --with-filename flag can be used to force printing the file
path in this case.
");
    let arg = RGArg::switch("count").short("c")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_debug(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show debug messages.";
    const LONG: &str = long!("\
Show debug messages. Please use this when filing a bug report.
");
    let arg = RGArg::switch("debug")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_dfa_size_limit(args: &mut Vec<RGArg>) {
    const SHORT: &str = "The upper size limit of the regex DFA.";
    const LONG: &str = long!("\
The upper size limit of the regex DFA. The default limit is 10M. This should
only be changed on very large regex inputs where the (slower) fallback regex
engine may otherwise be used if the limit is reached.

The argument accepts the same size suffixes as allowed in with the
--max-filesize flag.
");
    let arg = RGArg::flag("dfa-size-limit", "NUM+SUFFIX?")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_encoding(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Specify the text encoding of files to search.";
    const LONG: &str = long!("\
Specify the text encoding that ripgrep will use on all files searched. The
default value is 'auto', which will cause ripgrep to do a best effort automatic
detection of encoding on a per-file basis. Other supported values can be found
in the list of labels here:
https://encoding.spec.whatwg.org/#concept-encoding-get
");
    let arg = RGArg::flag("encoding", "ENCODING").short("E")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_file(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Search for patterns from the given file.";
    const LONG: &str = long!("\
Search for patterns from the given file, with one pattern per line. When this
flag is used multiple times or in combination with the -e/--regexp flag,
then all patterns provided are searched. Empty pattern lines will match all
input lines, and the newline is not counted as part of the pattern.

A line is printed if and only if it matches at least one of the patterns.
");
    let arg = RGArg::flag("file", "PATH").short("f")
        .help(SHORT).long_help(LONG)
        .multiple()
        .allow_leading_hyphen();
    args.push(arg);
}

fn flag_files(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Print each file that would be searched.";
    const LONG: &str = long!("\
Print each file that would be searched without actually performing the search.
This is useful to determine whether a particular file is being search or not.
");
    let arg = RGArg::switch("files")
        .help(SHORT).long_help(LONG)
        // This also technically conflicts with pattern, but the first file
        // path will actually be in pattern.
        .conflicts(&["file", "regexp", "type-list"]);
    args.push(arg);
}

fn flag_files_with_matches(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Only print the paths with at least one match.";
    const LONG: &str = long!("\
Only print the paths with at least one match.

This overrides --files-without-match.
");
    let arg = RGArg::switch("files-with-matches").short("l")
        .help(SHORT).long_help(LONG)
        .overrides("files-without-match");
    args.push(arg);
}

fn flag_files_without_match(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Only print the paths that contain zero matches.";
    const LONG: &str = long!("\
Only print the paths that contain zero matches. This inverts/negates the
--files-with-matches flag.

This overrides --files-with-matches.
");
    let arg = RGArg::switch("files-without-match")
        .help(SHORT).long_help(LONG)
        .overrides("files-with-matches");
    args.push(arg);
}

fn flag_fixed_strings(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Treat the pattern as a literal string.";
    const LONG: &str = long!("\
Treat the pattern as a literal string instead of a regular expression. When
this flag is used, special regular expression meta characters such as .(){}*+
do not need to be escaped.
");
    let arg = RGArg::switch("fixed-strings").short("F")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_follow(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Follow symbolic links.";
    const LONG: &str = long!("\
When this flag is enabled, ripgrep will follow symbolic links while traversing
directories. This is disabled by default. Note that ripgrep will check for
symbolic link loops and report errors if it finds one.

This flag can be disabled with --no-follow.
");
    let arg = RGArg::switch("follow").short("L")
        .help(SHORT).long_help(LONG)
        .overrides("no-follow");
    args.push(arg);

    let arg = RGArg::switch("no-follow")
        .hidden()
        .overrides("follow");
    args.push(arg);
}

fn flag_glob(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Include or exclude files.";
    const LONG: &str = long!("\
Include or exclude files and directories for searching that match the given
glob. This always overrides any other ignore logic. Multiple glob flags may be
used. Globbing rules match .gitignore globs. Precede a glob with a ! to exclude
it.
");
    let arg = RGArg::flag("glob", "GLOB").short("g")
        .help(SHORT).long_help(LONG)
        .multiple()
        .allow_leading_hyphen();
    args.push(arg);
}

fn flag_heading(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Print matches grouped by each file.";
    const LONG: &str = long!("\
This flag prints the file path above clusters of matches from each file instead
of printing the file path as a prefix for each matched line. This is the
default mode when printing to a terminal.

This overrides the --no-heading flag.
");
    let arg = RGArg::switch("heading")
        .help(SHORT).long_help(LONG)
        .overrides("no-heading");
    args.push(arg);

    const NO_SHORT: &str = "Don't group matches by each file.";
    const NO_LONG: &str = long!("\
Don't group matches by each file. If --no-heading is provided in addition to
the -H/--with-filename flag, then file paths will be printed as a prefix for
every matched line. This is the default mode when not printing to a terminal.

This overrides the --heading flag.
");
    let arg = RGArg::switch("no-heading")
        .help(NO_SHORT).long_help(NO_LONG)
        .overrides("heading");
    args.push(arg);
}

fn flag_hidden(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Search hidden files and directories.";
    const LONG: &str = long!("\
Search hidden files and directories. By default, hidden files and directories
are skipped. Note that if a hidden file or a directory is whitelisted in an
ignore file, then it will be searched even if this flag isn't provided.

This flag can be disabled with --no-hidden.
");
    let arg = RGArg::switch("hidden")
        .help(SHORT).long_help(LONG)
        .overrides("no-hidden");
    args.push(arg);

    let arg = RGArg::switch("no-hidden")
        .hidden()
        .overrides("hidden");
    args.push(arg);
}

fn flag_iglob(args: &mut Vec<RGArg>) {
    const SHORT: &str =
        "Include or exclude files case insensitively.";
    const LONG: &str = long!("\
Include or exclude files and directories for searching that match the given
glob. This always overrides any other ignore logic. Multiple glob flags may be
used. Globbing rules match .gitignore globs. Precede a glob with a ! to exclude
it. Globs are matched case insensitively.
");
    let arg = RGArg::flag("iglob", "GLOB")
        .help(SHORT).long_help(LONG)
        .multiple()
        .allow_leading_hyphen();
    args.push(arg);
}

fn flag_ignore_case(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Case insensitive search.";
    const LONG: &str = long!("\
When this flag is provided, the given patterns will be searched case
insensitively. The case insensitivity rules used by ripgrep conform to
Unicode's \"simple\" case folding rules.

This flag overrides -s/--case-sensitive and -S/--smart-case.
");
    let arg = RGArg::switch("ignore-case").short("i")
        .help(SHORT).long_help(LONG)
        .overrides("case-sensitive")
        .overrides("smart-case");
    args.push(arg);
}

fn flag_ignore_file(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Specify additional ignore files.";
    const LONG: &str = long!("\
Specify one or more files which contain ignore patterns. These patterns are
applied after the patterns found in .gitignore and .ignore are applied. Ignore
patterns should be in the gitignore format and are matched relative to the
current working directory. Multiple additional ignore files can be specified
by using the --ignore-file flag several times. When specifying multiple ignore
files, earlier files have lower precedence than later files.

If you are looking for a way to include or exclude files and directories
directly on the command line, then used -g instead.
");
    let arg = RGArg::flag("ignore-file", "PATH")
        .help(SHORT).long_help(LONG)
        .multiple()
        .allow_leading_hyphen();
    args.push(arg);
}

fn flag_invert_match(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Invert matching.";
    const LONG: &str = long!("\
Invert matching. Show lines that do not match the given patterns.
");
    let arg = RGArg::switch("invert-match").short("v")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_line_number(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show line numbers.";
    const LONG: &str = long!("\
Show line numbers (1-based). This is enabled by default when searching in a
terminal.
");
    let arg = RGArg::switch("line-number").short("n")
        .help(SHORT).long_help(LONG)
        .overrides("no-line-number");
    args.push(arg);

    const NO_SHORT: &str = "Suppress line numbers.";
    const NO_LONG: &str = long!("\
Suppress line numbers. This is enabled by default when not searching in a
terminal.
");
    let arg = RGArg::switch("no-line-number").short("N")
        .help(NO_SHORT).long_help(NO_LONG)
        .overrides("line-number");
    args.push(arg);
}

fn flag_line_number_width(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Left pad line numbers up to NUM width.";
    const LONG: &str = long!("\
Left pad line numbers up to NUM width. Space is used as the default padding
character. This has no effect if --no-line-number is enabled.
");
    let arg = RGArg::flag("line-number-width", "NUM")
        .help(SHORT).long_help(LONG)
        .line_number_width();
    args.push(arg);
}

fn flag_line_regexp(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Only show matches surrounded by line boundaries.";
    const LONG: &str = long!("\
Only show matches surrounded by line boundaries. This is equivalent to putting
^...$ around all of the search patterns. In other words, this only prints lines
where the entire line participates in a match.

This overrides the --word-regexp flag.
");
    let arg = RGArg::switch("line-regexp").short("x")
        .help(SHORT).long_help(LONG)
        .overrides("word-regexp");
    args.push(arg);
}

fn flag_max_columns(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Don't print lines longer than this limit.";
    const LONG: &str = long!("\
Don't print lines longer than this limit in bytes. Longer lines are omitted,
and only the number of matches in that line is printed.

When this flag is omitted or is set to 0, then it has no effect.
");
    let arg = RGArg::flag("max-columns", "NUM").short("M")
        .help(SHORT).long_help(LONG)
        .number();
    args.push(arg);
}

fn flag_max_count(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Limit the number of matches.";
    const LONG: &str = long!("\
Limit the number of matching lines per file searched to NUM.
");
    let arg = RGArg::flag("max-count", "NUM").short("m")
        .help(SHORT).long_help(LONG)
        .number();
    args.push(arg);
}

fn flag_max_filesize(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Ignore files larger than NUM in size.";
    const LONG: &str = long!("\
Ignore files larger than NUM in size. This does not apply to directories.

The input format accepts suffixes of K, M or G which correspond to kilobytes,
megabytes and gigabytes, respectively. If no suffix is provided the input is
treated as bytes.

Examples: --max-filesize 50K or --max-filesize 80M
");
    let arg = RGArg::flag("max-filesize", "NUM+SUFFIX?")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_maxdepth(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Descend at most NUM directories.";
    const LONG: &str = long!("\
Limit the depth of directory traversal to NUM levels beyond the paths given. A
value of zero only searches the explicitly given paths themselves.

For example, 'rg --maxdepth 0 dir/' is a no-op because dir/ will not be
descended into. 'rg --maxdepth 1 dir/' will search only the direct children of
'dir'.
");
    let arg = RGArg::flag("maxdepth", "NUM")
        .help(SHORT).long_help(LONG)
        .number();
    args.push(arg);
}

fn flag_mmap(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Search using memory maps when possible.";
    const LONG: &str = long!("\
Search using memory maps when possible. This is enabled by default when ripgrep
thinks it will be faster.

Memory map searching doesn't currently support all options, so if an
incompatible option (e.g., --context) is given with --mmap, then memory maps
will not be used.

Note that ripgrep may abort unexpectedly when --mmap if it searches a file that
is simultaneously truncated.

This flag overrides --no-mmap.
");
    let arg = RGArg::switch("mmap")
        .help(SHORT).long_help(LONG)
        .overrides("no-mmap");
    args.push(arg);

    const NO_SHORT: &str = "Never use memory maps.";
    const NO_LONG: &str = long!("\
Never use memory maps, even when they might be faster.

This flag overrides --mmap.
");
    let arg = RGArg::switch("no-mmap")
        .help(NO_SHORT).long_help(NO_LONG)
        .overrides("mmap");
    args.push(arg);
}

fn flag_no_config(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Never read configuration files.";
    const LONG: &str = long!("\
Never read configuration files. When this flag is present, ripgrep will not
respect the RIPGREP_CONFIG_PATH environment variable.

If ripgrep ever grows a feature to automatically read configuration files in
pre-defined locations, then this flag will also disable that behavior as well.
");
    let arg = RGArg::switch("no-config")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_no_ignore(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Don't respect ignore files.";
    const LONG: &str = long!("\
Don't respect ignore files (.gitignore, .ignore, etc.). This implies
--no-ignore-parent and --no-ignore-vcs.

This flag can be disabled with the --ignore flag.
");
    let arg = RGArg::switch("no-ignore")
        .help(SHORT).long_help(LONG)
        .overrides("ignore");
    args.push(arg);

    let arg = RGArg::switch("ignore")
        .hidden()
        .overrides("no-ignore");
    args.push(arg);
}

fn flag_no_ignore_parent(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Don't respect ignore files in parent directories.";
    const LONG: &str = long!("\
Don't respect ignore files (.gitignore, .ignore, etc.) in parent directories.

This flag can be disabled with the --ignore-parent flag.
");
    let arg = RGArg::switch("no-ignore-parent")
        .help(SHORT).long_help(LONG)
        .overrides("ignore-parent");
    args.push(arg);

    let arg = RGArg::switch("ignore-parent")
        .hidden()
        .overrides("no-ignore-parent");
    args.push(arg);
}

fn flag_no_ignore_vcs(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Don't respect VCS ignore files.";
    const LONG: &str = long!("\
Don't respect version control ignore files (.gitignore, etc.). This implies
--no-ignore-parent for VCS files. Note that .ignore files will continue to be
respected.

This flag can be disabled with the --ignore-vcs flag.
");
    let arg = RGArg::switch("no-ignore-vcs")
        .help(SHORT).long_help(LONG)
        .overrides("ignore-vcs");
    args.push(arg);

    let arg = RGArg::switch("ignore-vcs")
        .hidden()
        .overrides("no-ignore-vcs");
    args.push(arg);
}

fn flag_no_messages(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Suppress all error messages.";
    const LONG: &str = long!("\
Suppress all error messages. This provides the same behavior as redirecting
stderr to /dev/null on Unix-like systems.

This flag can be disabled with the --messages flag.
");
    let arg = RGArg::switch("no-messages")
        .help(SHORT).long_help(LONG)
        .overrides("messages");
    args.push(arg);

    let arg = RGArg::switch("messages")
        .hidden()
        .overrides("no-messages");
    args.push(arg);
}

fn flag_null(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Print a NUL byte after file paths.";
    const LONG: &str = long!("\
Whenever a file path is printed, follow it with a NUL byte. This includes
printing file paths before matches, and when printing a list of matching files
such as with --count, --files-with-matches and --files. This option is useful
for use with xargs.
");
    let arg = RGArg::switch("null").short("0")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_only_matching(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Print only matches parts of a line.";
    const LONG: &str = long!("\
Print only the matched (non-empty) parts of a matching line, with each such
part on a separate output line.
");
    let arg = RGArg::switch("only-matching").short("o")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_path_separator(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Set the path separator.";
    const LONG: &str = long!("\
Set the path separator to use when printing file paths. This defaults to your
platform's path separator, which is / on Unix and \\ on Windows. This flag is
intended for overriding the default when the environment demands it (e.g.,
cygwin). A path separator is limited to a single byte.
");
    let arg = RGArg::flag("path-separator", "SEPARATOR")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_passthru(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Print both matching and non-matching lines.";
    const LONG: &str = long!("\
Print both matching and non-matching lines.

Another way to achieve a similar effect is by modifying your pattern to match
the empty string. For example, if you are searching using 'rg foo' then using
'rg \"^|foo\"' instead will emit every line in every file searched, but only
occurrences of 'foo' will be highlighted. This flag enables the same behavior
without needing to modify the pattern.

This flag conflicts with the --only-matching and --replace flags.
");
    let arg = RGArg::switch("passthru")
        .help(SHORT).long_help(LONG)
        .alias("passthrough")
        .conflicts(&["only-matching", "replace"]);
    args.push(arg);
}

fn flag_pretty(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Alias for --color always --heading --line-number.";
    const LONG: &str = long!("\
This is a convenience alias for '--color always --heading --line-number'. This
flag is useful when you still want pretty output even if you're piping ripgrep
to another program or file. For example: 'rg -p foo | less -R'.
");
    let arg = RGArg::switch("pretty").short("p")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_quiet(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Do not print anything to stdout.";
    const LONG: &str = long!("\
Do not print anything to stdout. If a match is found in a file, then ripgrep
will stop searching. This is useful when ripgrep is used only for its exit
code (which will be an error if no matches are found).
");
    let arg = RGArg::switch("quiet").short("q")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_regex_size_limit(args: &mut Vec<RGArg>) {
    const SHORT: &str = "The upper size limit of the compiled regex.";
    const LONG: &str = long!("\
The upper size limit of the compiled regex. The default limit is 10M.

The argument accepts the same size suffixes as allowed in the --max-filesize
flag.
");
    let arg = RGArg::flag("regex-size-limit", "NUM+SUFFIX?")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_regexp(args: &mut Vec<RGArg>) {
    const SHORT: &str = "A pattern to search for.";
    const LONG: &str = long!("\
A pattern to search for. This option can be provided multiple times, where
all patterns given are searched. Lines matching at least one of the provided
patterns are printed. This flag can also be used when searching for patterns
that start with a dash.

For example, to search for the literal '-foo', you can use this flag:

    rg -e -foo

You can also use the special '--' delimiter to indicate that no more flags
will be provided. Namely, the following is equivalent to the above:

    rg -- -foo
");
    let arg = RGArg::flag("regexp", "PATTERN").short("e")
        .help(SHORT).long_help(LONG)
        .multiple()
        .allow_leading_hyphen();
    args.push(arg);
}

fn flag_replace(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Replace matches with the given text.";
    const LONG: &str = long!("\
Replace every match with the text given when printing results. Neither this
flag nor any other ripgrep flag will modify your files.

Capture group indices (e.g., $5) and names (e.g., $foo) are supported in the
replacement string.

Note that the replacement by default replaces each match, and NOT the entire
line. To replace the entire line, you should match the entire line.

This flag can be used with the -o/--only-matching flag.
");
    let arg = RGArg::flag("replace", "REPLACEMENT_TEXT").short("r")
        .help(SHORT).long_help(LONG)
        .allow_leading_hyphen();
    args.push(arg);
}

fn flag_search_zip(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Search in compressed files.";
    const LONG: &str = long!("\
Search in compressed files. Currently gz, bz2, xz, and lzma files are
supported. This option expects the decompression binaries to be available in
your PATH.

This flag can be disabled with --no-search-zip.
");
    let arg = RGArg::switch("search-zip").short("z")
        .help(SHORT).long_help(LONG)
        .overrides("no-search-zip");
    args.push(arg);

    let arg = RGArg::switch("no-search-zip")
        .hidden()
        .overrides("search-zip");
    args.push(arg);
}

fn flag_smart_case(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Smart case search.";
    const LONG: &str = long!("\
Searches case insensitively if the pattern is all lowercase. Search case
sensitively otherwise.

This overrides the -s/--case-sensitive and -i/--ignore-case flags.
");
    let arg = RGArg::switch("smart-case").short("S")
        .help(SHORT).long_help(LONG)
        .overrides("case-sensitive")
        .overrides("ignore-case");
    args.push(arg);
}

fn flag_sort_files(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Sort results by file path. Implies --threads=1.";
    const LONG: &str = long!("\
Sort results by file path. Note that this currently disables all parallelism
and runs search in a single thread.

This flag can be disabled with --no-sort-files.
");
    let arg = RGArg::switch("sort-files")
        .help(SHORT).long_help(LONG)
        .overrides("no-sort-files");
    args.push(arg);

    let arg = RGArg::switch("no-sort-files")
        .hidden()
        .overrides("sort-files");
    args.push(arg);
}

fn flag_text(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Search binary files as if they were text.";
    const LONG: &str = long!("\
Search binary files as if they were text. When this flag is present, ripgrep's
binary file detection is disabled. This means that when a binary file is
searched, its contents may be printed if there is a match. This may cause
escape codes to be printed that alter the behavior of your terminal.

When binary file detection is enabled it is imperfect. In general, it uses
a simple heuristic. If a NUL byte is seen during search, then the file is
considered binary and search stops (unless this flag is present).

Note that when the `-u/--unrestricted` flag is provided for a third time, then
this flag is automatically enabled.

This flag can be disabled with --no-text.
");
    let arg = RGArg::switch("text").short("a")
        .help(SHORT).long_help(LONG)
        .overrides("no-text");
    args.push(arg);

    let arg = RGArg::switch("no-text")
        .hidden()
        .overrides("text");
    args.push(arg);
}

fn flag_threads(args: &mut Vec<RGArg>) {
    const SHORT: &str = "The approximate number of threads to use.";
    const LONG: &str = long!("\
The approximate number of threads to use. A value of 0 (which is the default)
causes ripgrep to choose the thread count using heuristics.
");
    let arg = RGArg::flag("threads", "NUM").short("j")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_type(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Only search files matching TYPE.";
    const LONG: &str = long!("\
Only search files matching TYPE. Multiple type flags may be provided. Use the
--type-list flag to list all available types.
");
    let arg = RGArg::flag("type", "TYPE").short("t")
        .help(SHORT).long_help(LONG)
        .multiple();
    args.push(arg);
}

fn flag_type_add(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Add a new glob for a file type.";
    const LONG: &str = long!("\
Add a new glob for a particular file type. Only one glob can be added at a
time. Multiple --type-add flags can be provided. Unless --type-clear is used,
globs are added to any existing globs defined inside of ripgrep.

Note that this MUST be passed to every invocation of ripgrep. Type settings are
NOT persisted.

Example:

    rg --type-add 'foo:*.foo' -tfoo PATTERN.

--type-add can also be used to include rules from other types with the special
include directive. The include directive permits specifying one or more other
type names (separated by a comma) that have been defined and its rules will
automatically be imported into the type specified. For example, to create a
type called src that matches C++, Python and Markdown files, one can use:

    --type-add 'src:include:cpp,py,md'

Additional glob rules can still be added to the src type by using the
--type-add flag again:

    --type-add 'src:include:cpp,py,md' --type-add 'src:*.foo'

Note that type names must consist only of Unicode letters or numbers.
Punctuation characters are not allowed.
");
    let arg = RGArg::flag("type-add", "TYPE_SPEC")
        .help(SHORT).long_help(LONG)
        .multiple();
    args.push(arg);
}

fn flag_type_clear(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Clear globs for a file type.";
    const LONG: &str = long!("\
Clear the file type globs previously defined for TYPE. This only clears the
default type definitions that are found inside of ripgrep.

Note that this MUST be passed to every invocation of ripgrep. Type settings are
NOT persisted.
");
    let arg = RGArg::flag("type-clear", "TYPE")
        .help(SHORT).long_help(LONG)
        .multiple();
    args.push(arg);
}

fn flag_type_not(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Do not search files matching TYPE.";
    const LONG: &str = long!("\
Do not search files matching TYPE. Multiple type-not flags may be provided. Use
the --type-list flag to list all available types.
");
    let arg = RGArg::flag("type-not", "TYPE").short("T")
        .help(SHORT).long_help(LONG)
        .multiple();
    args.push(arg);
}

fn flag_type_list(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show all supported file types.";
    const LONG: &str = long!("\
Show all supported file types and their corresponding globs.
");
    let arg = RGArg::switch("type-list")
        .help(SHORT).long_help(LONG)
        // This also technically conflicts with PATTERN, but the first file
        // path will actually be in PATTERN.
        .conflicts(&["file", "files", "pattern", "regexp"]);
    args.push(arg);
}

fn flag_unrestricted(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Reduce the level of \"smart\" searching.";
    const LONG: &str = long!("\
Reduce the level of \"smart\" searching. A single -u won't respect .gitignore
(etc.) files. Two -u flags will additionally search hidden files and
directories. Three -u flags will additionally search binary files.

-uu is roughly equivalent to grep -r and -uuu is roughly equivalent to grep -a
-r.
");
    let arg = RGArg::switch("unrestricted").short("u")
        .help(SHORT).long_help(LONG)
        .multiple();
    args.push(arg);
}

fn flag_vimgrep(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Show results in vim compatible format.";
    const LONG: &str = long!("\
Show results with every match on its own line, including line numbers and
column numbers. With this option, a line with more than one match will be
printed more than once.
");
    let arg = RGArg::switch("vimgrep")
        .help(SHORT).long_help(LONG);
    args.push(arg);
}

fn flag_with_filename(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Print the file path with the matched lines.";
    const LONG: &str = long!("\
Display the file path for matches. This is the default when more than one
file is searched. If --heading is enabled (the default when printing to a
terminal), the file path will be shown above clusters of matches from each
file; otherwise, the file name will be shown as a prefix for each matched line.

This flag overrides --no-filename.
");
    let arg = RGArg::switch("with-filename").short("H")
        .help(SHORT).long_help(LONG)
        .overrides("no-filename");
    args.push(arg);

    const NO_SHORT: &str = "Never print the file path with the matched lines.";
    const NO_LONG: &str = long!("\
Never print the file path with the matched lines. This is the default when
ripgrep is explicitly instructed to search one file or stdin.

This flag overrides --with-filename.
");
    let arg = RGArg::switch("no-filename")
        .help(NO_SHORT).long_help(NO_LONG)
        .overrides("with-filename");
    args.push(arg);
}

fn flag_word_regexp(args: &mut Vec<RGArg>) {
    const SHORT: &str = "Only show matches surrounded by word boundaries.";
    const LONG: &str = long!("\
Only show matches surrounded by word boundaries. This is roughly equivalent to
putting \\b before and after all of the search patterns.

This overrides the --line-regexp flag.
");
    let arg = RGArg::switch("word-regexp").short("w")
        .help(SHORT).long_help(LONG)
        .overrides("line-regexp");
    args.push(arg);
}
