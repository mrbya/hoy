fn main() {
    hoy::hoy();
}

#[cfg(test)]
#[allow(dead_code, unused)]
mod tests {
    use crate::main;

    #[test]
    fn test_main() {
        main();
    }
}
