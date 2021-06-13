#[macro_use]
mod _runner;

#[mallockit::malloc_test]
fn clang(malloc: &str) {
    _runner::test(
        malloc,
        "clang++",
        &[
            &tests_dir!("test.cpp"),
            "-std=c++14",
            "-O3",
            "-o",
            "/dev/null",
        ],
    );
}
