from typing import List
import os
from .bench import Benchmark, BenchmarkSuite, MIMALLOC_BENCH_SRC_DIR, MIMALLOC_BENCH_EXTERN_DIR

PROCS = os.cpu_count()


class alloc_test1(Benchmark):
    name = 'alloc-test1'

    def run(self, env: List[str]):
        self.measure('./alloc-test 1', env=env)


class alloc_test(Benchmark):
    name = 'alloc-test'

    def run(self, env: List[str]):
        self.measure(f'./alloc-test {min(PROCS, 16)}', env=env)


class barnes(Benchmark):
    name = 'barnes'

    def run(self, env: List[str]):
        self.measure('./barnes', env=env,
                     infile=f'{MIMALLOC_BENCH_SRC_DIR}/barnes/input')


class cfrac(Benchmark):
    name = 'cfrac'

    def run(self, env: List[str]):
        self.measure(
            './cfrac 17545186520507317056371138836327483792789528', env=env)


class cscratch1(Benchmark):
    name = 'cache-scratch1'

    def run(self, env: List[str]):
        self.measure(f'./cache-scratch 1 1000 1 2000000  {PROCS}', env=env)


class cscratch(Benchmark):
    name = 'cache-scratch'

    def run(self, env: List[str]):
        self.measure(
            f'./cache-scratch {PROCS} 1000 1 2000000  {PROCS}', env=env)


class espresso(Benchmark):
    name = 'espresso'

    def run(self, env: List[str]):
        self.measure(
            './espresso ../../bench/espresso/largest.espresso', env=env)


class glibc_simple(Benchmark):
    name = 'glibc-simple'

    def run(self, env: List[str]):
        self.measure(f'./glibc-simple', env=env)


class glibc_thread(Benchmark):
    name = 'glibc-thread'

    def run(self, env: List[str]):
        self.measure(f'./glibc-thread {PROCS}', env=env)


class gs(Benchmark):
    name = 'gs'

    def run(self, env: List[str]):
        self.measure(
            f'gs -dBATCH -dNODISPLAY {MIMALLOC_BENCH_EXTERN_DIR}/large.pdf', env=env)


class larson(Benchmark):
    name = 'larson'

    def run(self, env: List[str]):
        self.measure(f'./larson 5 8 1000 5000 100 4141 {PROCS}', env=env)


class larson_sized(Benchmark):
    name = 'larson-sized'

    def run(self, env: List[str]):
        self.measure(f'./larson-sized 5 8 1000 5000 100 4141 {PROCS}', env=env)


class lean(Benchmark):
    name = 'lean'

    def run(self, env: List[str]):
        self.exec('make clean-olean',
                  cwd=f'{MIMALLOC_BENCH_EXTERN_DIR}/lean/out/release')
        self.measure(f'../bin/lean --make -j {min(PROCS, 8)}',
                     env=env, cwd=f'{MIMALLOC_BENCH_EXTERN_DIR}/lean/library')


class mstress(Benchmark):
    name = 'mstress'

    def run(self, env: List[str]):
        self.measure(f'./mstress {PROCS} 50 25', env=env)


class rptest(Benchmark):
    name = 'rptest'

    def run(self, env: List[str]):
        self.measure(f'./rptest {PROCS} 0 1 2 500 1000 100 8 16000', env=env)


class sh6bench(Benchmark):
    name = 'sh6bench'

    def run(self, env: List[str]):
        self.measure(f'./sh6bench {PROCS * 2}', env=env)


class sh8bench(Benchmark):
    name = 'sh8bench'

    def run(self, env: List[str]):
        # Note: `sh8bench` will return a non-zero value on normal completion.
        self.measure(f'./sh8bench {PROCS * 2} || true', env=env)


class xmalloc_test(Benchmark):
    name = 'xmalloc-test'

    def run(self, env: List[str]):
        self.measure(f'./xmalloc-test -w {PROCS} -t 5 -s 64', env=env)

# class sed(Benchmark):
#     name = 'sed'

#     def run(self, env: List[str]):
#         os.system('for i in {1..10000}; do echo "${i}.${i}.${i}.${i}" >> /tmp/sed_bench.txt; done')
#         self.measure('sed -E -n /^((.|.?){64}(.|.?)?(.|.?)){8}/p /tmp/sed_bench.txt', env=env)
#         os.system('rm /tmp/sed_bench.txt')


BENCHMARK_SUITE = BenchmarkSuite([
    alloc_test1(),
    alloc_test(),
    barnes(),
    cfrac(),
    cscratch1(),
    cscratch(),
    espresso(),
    glibc_simple(),
    glibc_thread(),
    gs(),
    larson(),
    larson_sized(),
    lean(),
    mstress(),
    rptest(),
    sh6bench(),
    sh8bench(),
    xmalloc_test(),
])

# BENCHMARK_SUITE = BenchmarkSuite([
#     alloc_test1(), alloc_test(),  cfrac(), lean(), larson(),
#     mstress(),
#     rptest(),
#     sh6bench(),
#     sh8bench(),
#     xmalloc_test(), cscratch(),
# ])
