/*!
This crate provides featureful and fast printers that interoperate with the
[`grep-searcher`](https://docs.rs/grep-searcher)
crate.

# Brief overview

The [`Standard`](struct.Standard.html) printer shows results in a human
readable format, and is modeled after the formats used by standard grep-like
tools. Features include, but are not limited to, cross platform terminal
coloring, search & replace, multi-line result handling and reporting summary
statistics.

The [`JSON`](struct.JSON.html) printer shows results in a machine readable
format. To facilitate a stream of search results, the format uses
[JSON Lines](http://jsonlines.org/)
by emitting a series of messages as search results are found.

The [`Summary`](struct.Summary.html) printer shows *aggregate* results for a
single search in a human readable format, and is modeled after similar formats
found in standard grep-like tools. This printer is useful for showing the total
number of matches and/or printing file paths that either contain or don't
contain matches.

# Example

This example shows how to create a "standard" printer and execute a search.

```
extern crate grep_regex;
extern crate grep_printer;
extern crate grep_searcher;

use std::error::Error;

use grep_regex::RegexMatcher;
use grep_printer::Standard;
use grep_searcher::Searcher;

const SHERLOCK: &'static [u8] = b"\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";

# fn main() { example().unwrap(); }
fn example() -> Result<(), Box<Error>> {
    let matcher = RegexMatcher::new(r"Sherlock")?;
    let mut printer = Standard::new_no_color(vec![]);
    Searcher::new().search_slice(&matcher, SHERLOCK, printer.sink(&matcher))?;

    // into_inner gives us back the underlying writer we provided to
    // new_no_color, which is wrapped in a termcolor::NoColor. Thus, a second
    // into_inner gives us back the actual buffer.
    let output = String::from_utf8(printer.into_inner().into_inner())?;
    let expected = "\
1:For the Doctor Watsons of this world, as opposed to the Sherlock
3:be, to a very large extent, the result of luck. Sherlock Holmes
";
    assert_eq!(output, expected);
    Ok(())
}
```
*/

#![deny(missing_docs)]

#[cfg(feature = "serde1")]
extern crate base64;
extern crate bstr;
extern crate grep_matcher;
#[cfg(test)]
extern crate grep_regex;
extern crate grep_searcher;
#[cfg(feature = "serde1")]
extern crate serde;
#[cfg(feature = "serde1")]
#[macro_use]
extern crate serde_derive;
#[cfg(feature = "serde1")]
extern crate serde_json;
extern crate termcolor;

pub use color::{default_color_specs, ColorError, ColorSpecs, UserColorSpec};
#[cfg(feature = "serde1")]
pub use json::{JSONBuilder, JSONSink, JSON};
pub use standard::{Standard, StandardBuilder, StandardSink};
pub use stats::Stats;
pub use summary::{Summary, SummaryBuilder, SummaryKind, SummarySink};
pub use util::PrinterPath;

#[macro_use]
mod macros;

mod color;
mod counter;
#[cfg(feature = "serde1")]
mod json;
#[cfg(feature = "serde1")]
mod jsont;
mod standard;
mod stats;
mod summary;
mod util;
