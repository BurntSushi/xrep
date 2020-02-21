use std::sync::atomic::{AtomicBool, Ordering};

static MESSAGES: AtomicBool = AtomicBool::new(false);
static IGNORE_MESSAGES: AtomicBool = AtomicBool::new(false);
static ERRORED: AtomicBool = AtomicBool::new(false);

/// Emit a non-fatal error message, unless messages were disabled.
#[macro_export]
macro_rules! message {
    ($($tt:tt)*) => {
        if crate::messages::messages() {
            eprintln!($($tt)*);
        }
    }
}

/// Like message, but sets ripgrep's "errored" flag, which controls the exit
/// status.
#[macro_export]
macro_rules! err_message {
    ($($tt:tt)*) => {
        crate::messages::set_errored();
        message!($($tt)*);
    }
}

/// Emit a non-fatal ignore-related error message (like a parse error), unless
/// ignore-messages were disabled.
#[macro_export]
macro_rules! ignore_message {
    ($($tt:tt)*) => {
        if crate::messages::messages() && crate::messages::ignore_messages() {
            eprintln!($($tt)*);
        }
    }
}

/// Returns true if and only if messages should be shown.
pub fn messages() -> bool {
    MESSAGES.load(Ordering::SeqCst)
}

/// Set whether messages should be shown or not.
///
/// By default, they are not shown.
pub fn set_messages(yes: bool) {
    MESSAGES.store(yes, Ordering::SeqCst)
}

/// Returns true if and only if "ignore" related messages should be shown.
pub fn ignore_messages() -> bool {
    IGNORE_MESSAGES.load(Ordering::SeqCst)
}

/// Set whether "ignore" related messages should be shown or not.
///
/// By default, they are not shown.
///
/// Note that this is overridden if `messages` is disabled. Namely, if
/// `messages` is disabled, then "ignore" messages are never shown, regardless
/// of this setting.
pub fn set_ignore_messages(yes: bool) {
    IGNORE_MESSAGES.store(yes, Ordering::SeqCst)
}

/// Returns true if and only if ripgrep came across a non-fatal error.
pub fn errored() -> bool {
    ERRORED.load(Ordering::SeqCst)
}

/// Indicate that ripgrep has come across a non-fatal error.
pub fn set_errored() {
    ERRORED.store(true, Ordering::SeqCst);
}
