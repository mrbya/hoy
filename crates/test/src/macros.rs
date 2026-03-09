#[macro_export]
macro_rules! assert_err {
    ($value:expr, $error:pat) => {
        assert!(matches!($value, $error));
    };
}

#[macro_export]
macro_rules! async_ok {
    ($timeout:expr, $fn:expr) => {
        tokio::time::timeout(std::time::Duration::from_millis($timeout), $fn)
            .await
            .expect("Operation timed out.")
    };
}
