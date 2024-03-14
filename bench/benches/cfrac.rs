use harness::{bench, Bencher};

#[bench(oneshot)]
fn bench(bencher: &Bencher) {
    let malloc = std::env::var("MALLOC").unwrap();
    println!("MALLOC: {}", malloc);
    let mut bench = Bench::new("cfrac").alloc(&malloc);
    bencher.time(|| bench.run());
}
