use harness::{bench, Bencher};
use mallockit_bench::Bench;

#[bench(oneshot)]
fn bench(bencher: &Bencher) {
    let mut bench = Bench::new("rptest");
    bencher.time(|| bench.run());
    bench.finalize(bencher)
}
