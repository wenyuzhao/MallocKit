#[macro_use]
mod runner;

#[test]
fn clang() {
    runner::test("clang", &[&tests_dir!("test.c"), "-o", "/dev/null"]);
}
