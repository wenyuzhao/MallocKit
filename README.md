# MallocKit

## Getting Started

```console
$ cargo build --release
$ env LD_PRELOAD=./target/release/libbump.so cargo # or some other command
```
#### Run on macOS

```console
$ env DYLD_INSERT_LIBRARIES=./target/release/libbump.so cargo # or some other command
```

## Tests

```console
$ rake test
```

## Benchmarking

...WIP

## TODO

- [ ] Performance
- [ ] Rust allocator interface
