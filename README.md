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

```
python3 -m bench -a mi hd sys hoard -i 10 --build
```

Then please use `bench/visual.ipynb` for visualization.

Run `python3 -m bench -a hoard -b cfrac --build --record -e dTLB-loads` followed by `perf report` to record and analyze perf event data.

## TODO

- [x] Linux support
- [x] MacOS support
- [ ] Performance
- [ ] Rust allocator interface
- [ ] Windows support
