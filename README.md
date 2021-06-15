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

## Tests

```console
$ cargo test
```

## Benchmarking

...WIP

## TODO

- [x] Linux support
- [x] MacOS support
- [ ] Performance
- [ ] Rust allocator interface
- [ ] Windows support
