setup: setup-mimalloc-bench setup-bench-tools

setup-mimalloc-bench:
	cd mimalloc-bench && ./build-bench-env.sh bench je tc mi hd redis rocksdb lean

setup-packages:
	cd mimalloc-bench && ./build-bench-env.sh packages
	sudo sysctl -w kernel.perf_event_paranoid=-1