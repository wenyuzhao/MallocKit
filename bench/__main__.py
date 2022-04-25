from bench.bench import Benchmark, BenchmarkSuite
from .benchmarks import BENCHMARK_SUITE
import argparse
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description='Benchmark runner.\n  - example: python3 -m bench -a mi hd sys buddy -b cfrac espresso barnes lean larson -i 10 -e duration_time cache-misses cache-references dTLB-load-misses dTLB-loads instructions page-faults --build', formatter_class=argparse.RawTextHelpFormatter)
parser.add_argument('-a', '--algorithms', nargs='*', default=BENCHMARK_SUITE.algorithms)
parser.add_argument('-b', '--benchmarks', nargs='*', default=[b.name for b in BENCHMARK_SUITE.benchmarks])
parser.add_argument('-i', '--invocations', nargs='?', type=int, default=1)
parser.add_argument('-e', '--events', '--perf-events', nargs='*')
parser.add_argument('--build', default=False, action='store_true')
parser.add_argument('--features', nargs='*')
parser.add_argument('--debug', default=False, action='store_true')
parser.add_argument('--test', default=False, action='store_true')
parser.add_argument('--record', default=False, action='store_true')

args = parser.parse_args()

if args.build:
    cwd = Path(__file__).parent.parent.absolute()
    flags = '--release' if not args.debug else ''
    features = ''
    if args.features is not None:
        features = f'--features {" ".join(args.features)}'
    subprocess.check_call(f'cargo build {flags} {features}', shell=True, text=True, cwd=cwd)

if args.debug:
    BenchmarkSuite.debug = True

if args.record:
    assert len(args.algorithms) == 1, 'Only one malloc algorithm is allowed when specifying --record'
    assert len(args.benchmarks) == 1, 'Only one benchmark is allowed when specifying --record'
    assert args.invocations == 1, 'Only one invocation is allowed when specifying --record'
    assert args.events is None or len(args.events) <= 1, 'Only one perf-event is allowed when specifying --record'
    Benchmark.record = True

if args.test:
    Benchmark.test = True

if args.events is not None and len(args.events) > 0:
    BENCHMARK_SUITE.perf = ','.join(args.events)

BENCHMARK_SUITE.run(args.algorithms, args.benchmarks, args.invocations)

