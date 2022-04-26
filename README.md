# MallocKit

## Getting Started

```console
$ cargo build --release
$ env LD_PRELOAD=./target/release/libbump.so cargo # or some other command
```
#### Run on macOS

```console
$ env DYLD_INSERT_LIBRARIES=./target/release/libbump.dylib cargo # or some other command
```

*Note: If you'd like to hijack the system apps and libraries as well, disable System Integrity Protection (SIP). Do this at your own risk 😉*

## Tests

```console
$ cargo test
```

## Benchmarking

Please use a linux distribution (e.g. Ubuntu or Fedora) and run `cd bench && make setup` to fetch and build all the benchmarks and third-party malloc algorithms.

```
python3 -m bench -a mi hd sys hoard -i 10 -e duration_time cycles cache-misses cache-references dTLB-load-misses dTLB-loads instructions page-faults --build
```

After the benchmark run is finished, please use `bench/visual.ipynb` for visualization.

_Other usages of the benchmark tool:_

* `python3 -m bench -a hoard -b cfrac --build --record -e dTLB-loads` followed by `perf report` to record and analyze perf event data.
* `python3 -m bench -a hoard -b cfrac --build --test` for a quick run of a single benchmark.
* `python3 -m bench -a hoard -b cfrac --build --lldb` to run the benchmark binary with _lldb_.

## TODO

- [x] Linux support (x86_64)
- [x] MacOS support (x86_64)
- [ ] Windows support (x86_64)
- [ ] Performance
- [ ] Linux support (aarch64)
- [ ] MacOS support (aarch64)
- [ ] Windows support (aarch64)
- [ ] Rust allocator interface
