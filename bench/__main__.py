from bench.bench import BenchmarkSuite
from .benchmarks import BENCHMARK_SUITE
import argparse
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description='Benchmark runner.\n  - example: python3 -m bench -a mi hd sys buddy -b cfrac espresso barnes lean larson -i 10 -p duration_time cache-misses cache-references dTLB-load-misses dTLB-loads instructions page-faults --build', formatter_class=argparse.RawTextHelpFormatter)
parser.add_argument('-a', '--algorithms', nargs='*', default=BENCHMARK_SUITE.algorithms)
parser.add_argument('-b', '--benchmarks', nargs='*', default=[b.name for b in BENCHMARK_SUITE.benchmarks])
parser.add_argument('-i', '--invocations', nargs='?', type=int, default=1)
parser.add_argument('-p', '--perf', nargs='*')
parser.add_argument('--build', default=False, action='store_true')
parser.add_argument('--debug', default=False, action='store_true')

args = parser.parse_args()

if len(args.perf) > 0:
    BENCHMARK_SUITE.perf = ','.join(args.perf)

if args.build:
    cwd = Path(__file__).parent.parent.absolute()
    flags = '--release' if not args.debug else ''
    subprocess.check_call(f'cargo build {flags}', shell=True, text=True, cwd=cwd)

if args.debug:
    BenchmarkSuite.debug = True

BENCHMARK_SUITE.run(args.algorithms, args.benchmarks, args.invocations)

