#[macro_export]
macro_rules! assert_err {
    ($value:expr, $error:pat) => {
        assert!(matches!($value, $error));
    };
}
