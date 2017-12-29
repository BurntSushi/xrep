/*!
This crate provides a safe and simple Windows specific API to control
text attributes in the Windows console. Text attributes are limited to
foreground/background colors, as well as whether to make colors intense or not.

Note that on non-Windows platforms, this crate is empty but will compile.

# Example

```no_run
# #[cfg(windows)]
# {
use wincolor::{Console, Color, Intense};

let mut con = Console::stdout().unwrap();
con.fg(Intense::Yes, Color::Cyan).unwrap();
println!("This text will be intense cyan.");
con.reset().unwrap();
println!("This text will be normal.");
# }
```
*/

#![deny(missing_docs)]

#[cfg(windows)]
extern crate winapi;

#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win;
