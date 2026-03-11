/// Asserts that an expression matches a given error pattern.
///
/// # Example
/// ```ignore
/// assert_err!(result, Err(MyError::NotFound));
/// ```
#[macro_export]
macro_rules! assert_err {
    ($value:expr, $error:pat) => {
        assert!(matches!($value, $error));
    };
}

/// Awaits an async expression with a millisecond timeout, panicking on expiry.
///
/// # Example
/// ```ignore
/// let value = async_ok!(200, some_future);
/// ```
#[macro_export]
macro_rules! async_ok {
    ($timeout:expr, $fn:expr) => {
        tokio::time::timeout(std::time::Duration::from_millis($timeout), $fn)
            .await
            .expect("Operation timed out.")
    };
}

/// Asserts that an expression matches a given pattern.
///
/// # Example
/// ```ignore
/// assert_matches!(value, Some(42));
/// ```
#[macro_export]
macro_rules! assert_matches {
    ($act:expr, $exp:pat) => {
        assert!(matches!($act, $exp));
    };
}
