//! # Safety boundary
//!
//! The critical difference between FaF (standalone server) and Tachyon (library):
//! if we panic or hang, we CANNOT crash the host process (Node.js, Bun, etc.).
//!
//! This module provides:
//! - `catch_handler_mut`: wraps a handler in `catch_unwind` so panics don't propagate
//! - `SafeResult`: unified error type for the FFI boundary

use std::panic::{catch_unwind, AssertUnwindSafe};

/// Result type that crosses the FFI boundary safely.
#[derive(Debug)]
pub enum SafeResult<T> {
    Ok(T),
    Panic(String),
    Error(String),
}

impl<T> SafeResult<T> {
    pub fn into_result(self) -> Result<T, String> {
        match self {
            SafeResult::Ok(v) => Ok(v),
            SafeResult::Panic(msg) => Err(format!("Handler panicked: {}", msg)),
            SafeResult::Error(msg) => Err(msg),
        }
    }
}

/// Wrap a closure so panics are caught even when it captures mutable state.
/// This is needed because most real handlers capture `&mut` references
/// (to the request buffer, response writer, etc.).
pub fn catch_handler_mut<F, T>(f: F) -> SafeResult<T>
where
    F: FnOnce() -> Result<T, String>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(val)) => SafeResult::Ok(val),
        Ok(Err(e)) => SafeResult::Error(e),
        Err(panic_payload) => {
            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };
            SafeResult::Panic(msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catch_normal() {
        let result = catch_handler_mut(|| Ok::<_, String>(42));
        assert!(matches!(result, SafeResult::Ok(42)));
    }

    #[test]
    fn catch_panic() {
        let result = catch_handler_mut(|| -> Result<i32, String> {
            panic!("boom");
        });
        match result {
            SafeResult::Panic(msg) => assert_eq!(msg, "boom"),
            other => panic!("Expected Panic, got {:?}", other),
        }
    }

    #[test]
    fn catch_error() {
        let result = catch_handler_mut(|| Err::<i32, _>("bad input".to_string()));
        assert!(matches!(result, SafeResult::Error(_)));
    }

    #[test]
    fn into_result_ok() {
        let result = catch_handler_mut(|| Ok::<_, String>(99));
        assert_eq!(result.into_result().unwrap(), 99);
    }

    #[test]
    fn into_result_panic() {
        let result = catch_handler_mut(|| -> Result<i32, String> { panic!("oops") });
        let err = result.into_result().unwrap_err();
        assert!(err.contains("panicked"));
        assert!(err.contains("oops"));
    }
}
