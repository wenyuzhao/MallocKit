set -ex
cargo build
# nm ./target/debug/libbump.dylib
export LIBRARY_PATH="$LIBRARY_PATH:$PWD/target/debug"
echo $LIBRARY_PATH
clang ./test.c #$PWD/target/debug/libbump.a
# DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES=./target/debug/libbump.dylib ./a.out
# DYLD_PRINT_LIBRARIES=1 X=1 MallocPreScribble=1 DYLD_INSERT_LIBRARIES=./target/debug/libbump.dylib ./a.out
# ./a.out
# gdb --args ./a.out
# gdb -ex "set env DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES=./target/debug/libbump.dylib" ./a.out
lldb -o "env DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES=./target/debug/libbump.dylib" ./a.out
# objc[12543]: realized class 0x7fff88c0fcc8 has corrupt data pointer 0x10edddba0