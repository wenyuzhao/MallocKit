profile=debug
malloc=bump

CFLAGS=
CC=clang


ifeq ($(profile), debug)
    target_dir=./target/debug
    cargo_build_flag=
else
    target_dir=./target/release
    cargo_build_flag=--release
    CFLAGS += -O3
endif

# dylib_ext=dylib
dylib_ext=so
dylib=$(target_dir)/lib$(malloc).$(dylib_ext)
# dylib_env=DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES=$(dylib)
dylib_env=LD_PRELOAD=$(dylib)

build: FORCE
	@cargo build $(cargo_build_flag)
	@llvm-objdump -d -S  $(target_dir)/lib$(malloc).a > $(target_dir)/lib$(malloc).s 2>/dev/null
	@$(CC) $(CFLAGS) ./test.c

# build: FORCE
# 	cargo build $(cargo_build_flag)
# 	clang -fuse-ld=lld -g -O3 -flto ./test.c $(target_dir)/libbump.a -o test
# 	llvm-objdump -D -S -m -g -C  ./test > test.s

program = gcc ./test.c -o ./target/test

test: build
	$(dylib_env) time $(program)

# GDB to LLDB command map: https://lldb.llvm.org/use/map.html
lldb: build
	rust-lldb -b -o "settings set auto-confirm true" -o "env $(dylib_env)" -o "run" -- $(program)

FORCE: