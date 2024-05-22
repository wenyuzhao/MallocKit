cargo build -p hoard --release --features malloc

env LD_PRELOAD=./target/release/libhoard.so cargo --help