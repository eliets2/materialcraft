use crate::{FormatError, Result};

pub(crate) fn guard<T>(f: impl FnOnce() -> Result<T>, err: impl FnOnce() -> FormatError) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| Err(err()))
}
