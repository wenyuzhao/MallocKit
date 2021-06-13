#[macro_use]
mod _runner;

#[mallockit::malloc_test]
fn cargo(malloc: &str) {
    _runner::test(malloc, "cargo", &["--help"]);
}
