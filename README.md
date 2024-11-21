# MallocKit

## Getting Started

```console
$ cargo build -p hoard --release --features malloc
$ env LD_PRELOAD=./target/release/libhoard.so cargo --help # or some other command
```
#### Run on macOS

```console
$ env DYLD_INSERT_LIBRARIES=./target/release/libhoard.dylib cargo --help # or some other command
```

*Note: If you'd like to hijack the system apps and libraries as well, disable System Integrity Protection (SIP). Do this at your own risk 😉*

## Tests

```console
$ cargo test
```

## TODO

- [x] Linux/x86_64 support
- [x] MacOS/x86_64 support
- [ ] Windows/x86_64 support
- [x] Performance
- [x] Linux/aarch64 support
- [x] MacOS/aarch64 support (arm64 only. arm64e is currently unsupported)
- [ ] Windows/aarch64 support
- [x] Rust allocator interface
