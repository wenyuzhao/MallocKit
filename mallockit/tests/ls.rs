#[macro_use]
mod _runner;

#[mallockit::malloc_test]
fn ls(malloc: &str) {
    _runner::test(malloc, "ls", &["-al"]);
}
