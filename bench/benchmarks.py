from typing import List
import os
import threading
from .bench import Benchmark, BenchmarkSuite, MIMALLOC_BENCH_SRC_DIR, MIMALLOC_BENCH_EXTERN_DIR

class cfrac(Benchmark):
    name = 'cfrac'

    def run(self, env: List[str]):
        self.measure('./cfrac 17545186520507317056371138836327483792789528', env=env)

class espresso(Benchmark):
    name = 'espresso'

    def run(self, env: List[str]):
        self.measure('./espresso ../../bench/espresso/largest.espresso', env=env)

class barnes(Benchmark):
    name = 'barnes'

    def run(self, env: List[str]):
        self.measure('./barnes', env=env, infile=f'{MIMALLOC_BENCH_SRC_DIR}/barnes/input')

class lean(Benchmark):
    name = 'lean'

    def run(self, env: List[str]):
        self.exec('make clean-olean', cwd=f'{MIMALLOC_BENCH_EXTERN_DIR}/lean/out/release')
        self.measure('../bin/lean --make -j 8', env=env, cwd=f'{MIMALLOC_BENCH_EXTERN_DIR}/lean/library')

class larson(Benchmark):
    name = 'larson'

    def run(self, env: List[str]):
        procs = len(os.sched_getaffinity(0))
        self.measure(f'./larson 5 8 1000 5000 100 4141 {procs}', env=env)
        # TODO: Fix with relative time

BENCHMARK_SUITE = BenchmarkSuite([
    cfrac(), espresso(), barnes(), lean(), larson(),
])