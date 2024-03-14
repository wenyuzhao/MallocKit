use harness::{bench, Bencher};
use mallockit_bench::Bench;

#[bench(oneshot)]
fn bench(bencher: &Bencher) {
    let malloc = std::env::var("MALLOC").unwrap();
    let mut bench = Bench::new("lean", &malloc);
    bencher.time(|| bench.run());
    bench.finalize(bencher)
}
