setup: setup-mimalloc-bench setup-bench-tools

setup-mimalloc-bench:
	cd mimalloc-bench && ./build-bench-env.sh all no-mesh no-nomesh no-gd no-sg no-tcg

setup-bench-tools:
	sudo apt install python3 python3-pip linux-tools-common linux-tools-generic
	sudo sysctl -w kernel.perf_event_paranoid=-1
	perf stat -- ls
	sudo pip3 install -r requirements.txt