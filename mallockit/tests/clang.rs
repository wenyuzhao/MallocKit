#[macro_use]
mod runner;

#[mallockit::malloc_test]
fn clang(malloc: &str) {
    runner::test(malloc, "clang", &[&tests_dir!("test.c"), "-o", "/dev/null"]);
}
