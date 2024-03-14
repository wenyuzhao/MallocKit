use harness::{bench, Bencher};
use mallockit_bench::Bench;

#[bench(oneshot)]
fn bench(bencher: &Bencher) {
    let malloc = std::env::var("MALLOC").unwrap();
    println!("MALLOC: {}", malloc);
    let mut bench = Bench::new("barnes").alloc(&malloc);
    bencher.time(|| bench.run());
}
