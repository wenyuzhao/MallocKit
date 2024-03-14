use harness::{bench, Bencher};

#[bench(oneshot)]
fn bench(bencher: &Bencher) {
    let malloc = std::env::var("MALLOC").unwrap();
    println!("MALLOC: {}", malloc);
    bencher.time(|| mallockit_bench::run("barnes", &malloc));
}
