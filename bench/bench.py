#!/usr/bin/env python3

import sys
import os
import subprocess
import multiprocessing
from pathlib import Path
import argparse
import toml
import pandas as pd
from io import StringIO

PROCS = multiprocessing.cpu_count()
DYLIB = 'so'

PERF_EVENTS = 'page-faults,dTLB-loads,dTLB-load-misses,cache-misses,cache-references'

CURRENT_FILE_DIR = os.path.dirname(os.path.abspath(__file__))
MALLOCKIT_DIR = os.path.dirname(CURRENT_FILE_DIR)
MIMALLOC_BENCH_DIR = f'{CURRENT_FILE_DIR}/mimalloc-bench'
OUT_BENCH_DIR = f'{MIMALLOC_BENCH_DIR}/out/bench'
EXTERN_DIR = f'{MIMALLOC_BENCH_DIR}/extern'
BENCH_DIR = f'{MIMALLOC_BENCH_DIR}/bench'

# Valid algorithms and benchmarks
ALL_ALGORITHMS = [ 'sys', 'mi', 'smi', 'tc', 'je', 'sn', 'hd' ]
ALL_BENCHMARKS = [ 'cfrac', 'espresso', 'barnes', 'leanN', 'xmalloc-testN', 'larsonN', 'cache-scratch1', 'cache-scratchN', 'mstressN' ]
if sys.platform != 'darwin':
    ALL_BENCHMARKS += [ 'rptestN', 'alloc-test1', 'alloc-testN', 'sh6benchN' ]
# -- find all mallockit algorithms
MALLOCTK_ALGORITHMS = []
with open(f'{MALLOCKIT_DIR}/Cargo.toml', 'r') as f:
    members = toml.load(f)['workspace']['members']
    filter_list = ['mallockit']
    MALLOCTK_ALGORITHMS = [ m for m in members if m not in filter_list ]
ALL_ALGORITHMS += MALLOCTK_ALGORITHMS

# Benchmark commands and environment variables
LD_PRELOAD = lambda x: f'LD_PRELOAD={EXTERN_DIR}/{x}'
ALGORITHM_ENVS = {
    'sys': '1=1',
    'mi': LD_PRELOAD(f'mimalloc/out/release/libmimalloc.{DYLIB}'),
    'smi': LD_PRELOAD(f'mimalloc/out/release/libmimalloc-secure.{DYLIB}'),
    'tc': LD_PRELOAD(f'gperftools/.libs/libtcmalloc_minimal.{DYLIB}'),
    'je': LD_PRELOAD(f'jemalloc/lib/libjemalloc.{DYLIB}'),
    'sn': LD_PRELOAD(f'snmalloc/release/libsnmallocshim.{DYLIB}'),
    'hd': LD_PRELOAD(f'Hoard/src/libhoard.{DYLIB}'),
}
MALLOCKIT_ALGORITHM_ENV = lambda name, debug=False: f'LD_PRELOAD={MALLOCKIT_DIR}/target/{"debug" if debug else "release"}/lib{name}.{DYLIB}'
TEST_CWD = {
    'leanN': f'{EXTERN_DIR}/lean/library'
}
TEST_COMMANDS = {
    'cfrac': f'{OUT_BENCH_DIR}/cfrac 17545186520507317056371138836327483792789528',
    'espresso': f'{OUT_BENCH_DIR}/espresso {BENCH_DIR}/espresso/largest.espresso',
    'barnes': f'{OUT_BENCH_DIR}/barnes < {BENCH_DIR}/barnes/input',
    'leanN': f'../bin/lean --make -j 8',
    'alloc-test1': f'{OUT_BENCH_DIR}/alloc-test 1',
    'alloc-testN': f'{OUT_BENCH_DIR}/alloc-test {min(PROCS, 16)}',
    'larsonN': f'{OUT_BENCH_DIR}/larson 5 8 1000 5000 100 4141 {PROCS}',
    'sh6benchN': f'{OUT_BENCH_DIR}/sh6bench {PROCS * 2}',
    'sh8benchN': f'{OUT_BENCH_DIR}/sh8bench {PROCS * 2}',
    'xmalloc-testN': f'{OUT_BENCH_DIR}/xmalloc-test -w {PROCS} -t 5 -s 64',
    'cache-scratch1': f'{OUT_BENCH_DIR}/cache-scratch 1 1000 1 2000000 {PROCS}',
    'cache-scratchN': f'{OUT_BENCH_DIR}/cache-scratch {PROCS} 1000 1 2000000 {PROCS}',
    'mstressN': f'{OUT_BENCH_DIR}/mstress {PROCS} 50 25',
    'rptestN': f'{OUT_BENCH_DIR}/rptest {16 if 18 > PROCS else PROCS} 0 1 2 500 1000 100 8 16000',
}

def execute(cmd, cwd=None, perf=False, verbose=True):
    if verbose: print('⏳ ' + cmd)
    if perf is not False:
        e = f"-e '{PERF_EVENTS}'" if PERF_EVENTS != '' else '-n'
        pre_report = f'echo === perf stat results: {perf} ==='
        post_report = 'echo === perf stat results end ==='
        cmd = f"perf stat --no-scale --post '{pre_report}' {e} {cmd}; {post_report}"
    if cwd is not None: cmd = f'cd {cwd}; ' + cmd
    return subprocess.check_output(cmd, stderr=subprocess.STDOUT, shell=True).decode("utf-8")

def run_once(test, algorithm, program, debug=False):
    assert test in ALL_BENCHMARKS and algorithm in ALL_ALGORITHMS
    if test == 'leanN': execute(f"make clean-olean", cwd=f'{EXTERN_DIR}/lean/out/release', perf=False)
    env = 'env ' + (MALLOCKIT_ALGORITHM_ENV(algorithm, debug=debug) if algorithm in MALLOCTK_ALGORITHMS else ALGORITHM_ENVS[algorithm])
    out = execute(f"{env} {program}", cwd=TEST_CWD[test] if test in TEST_CWD else None, perf=f'{algorithm}, {test}')
    print(out)
    # Parse perf stat results
    # csv = ''
    # start_read_csv = False
    # for line in out.split('\n'):
    #     line = line.strip()
    #     if line.startswith('Performance counter stats for '): start_read_csv = True
    #     elif line.startswith('=== perf stat results end ==='): start_read_csv = False
    #     elif start_read_csv and line != '':
    #         print(line.split(' '))
    # print(pd.read_csv(StringIO(csv), header=None))

def run(tests=ALL_BENCHMARKS, algorithms=ALL_ALGORITHMS, debug=False):
    os.system(f"cd {MALLOCKIT_DIR} && cargo build {'--release' if not debug else ''}")
    for t in tests:
        for a in algorithms:
            run_once(t, a, TEST_COMMANDS[t], debug)



def main():
    global PERF_EVENTS

    parser = argparse.ArgumentParser(formatter_class=argparse.RawTextHelpFormatter, description=
        f"MallocKit Benchmark Runner\n"
        f" - available algorithms: {' '.join(ALL_ALGORITHMS)}\n"
        f" - available benchmarks: {' '.join(ALL_BENCHMARKS)}\n"
        f" - default perf events: {PERF_EVENTS}\n"
        f"    * use 'perf list' to see all available events\n"
    )
    def algorithm(v):
        if v not in ALL_ALGORITHMS: raise ValueError
        return v
    def benchmark(v):
        if v not in ALL_BENCHMARKS: raise ValueError
        return v
    parser.add_argument('-a', '--algo', nargs='*', type=algorithm, default=ALL_ALGORITHMS, help='malloc algorithms')
    parser.add_argument('-t', '--test', nargs='*', type=benchmark, default=ALL_BENCHMARKS, help='benchmarks')
    parser.add_argument('--perf', default=PERF_EVENTS, help='perf events')
    parser.add_argument('--debug', default=False, help='Use debug build of mallockit algorithms', action='store_true')
    args = parser.parse_args()

    PERF_EVENTS = args.perf.strip()
    run(tests=args.test, algorithms=args.algo, debug=args.debug)

main()