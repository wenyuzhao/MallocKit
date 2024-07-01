use harness::{bench, Bencher};
use mallockit_bench::Bench;

#[bench(oneshot)]
fn bench(bencher: &Bencher) {
    let mut bench = Bench::new("sh8bench");
    bencher.time(|| bench.run());
    bench.finalize(bencher)
}
