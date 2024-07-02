# Benchmarking Instructions

1. Clone the mimalloc-bench repo: `git submodule update --init`
2. Install the benchmarking tool: `cargo install harness-cli`
3. Build benchmarks and mallocs: `docker compose up --build`
4. Run: `cd bench && cargo harness run --upload`

[latest results](https://r.harness.rs/?p=7A5saxywVxwz2i5P)