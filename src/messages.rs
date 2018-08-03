use std::sync::atomic::{ATOMIC_BOOL_INIT, AtomicBool, Ordering};

static MESSAGES: AtomicBool = ATOMIC_BOOL_INIT;
static IGNORE_MESSAGES: AtomicBool = ATOMIC_BOOL_INIT;

#[macro_export]
macro_rules! message {
    ($($tt:tt)*) => {
        if ::messages::messages() {
            eprintln!($($tt)*);
        }
    }
}

#[macro_export]
macro_rules! ignore_message {
    ($($tt:tt)*) => {
        if ::messages::messages() && ::messages::ignore_messages() {
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
