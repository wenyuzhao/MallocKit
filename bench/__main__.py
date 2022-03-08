from .benchmarks import BENCHMARK_SUITE
import argparse

parser = argparse.ArgumentParser(description='Benchmark runner.\n  - example: python3 -m bench -a mi hd sys buddy -b cfrac espresso barnes lean larson -i 10 -p duration_time cache-misses cache-references dTLB-load-misses dTLB-loads instructions page-faults', formatter_class=argparse.RawTextHelpFormatter)
parser.add_argument('-a', '--algorithms', nargs='*', default=BENCHMARK_SUITE.algorithms)
parser.add_argument('-b', '--benchmarks', nargs='*', default=[b.name for b in BENCHMARK_SUITE.benchmarks])
parser.add_argument('-i', '--invocations', nargs='?', type=int, default=1)
parser.add_argument('-p', '--perf', nargs='*')

args = parser.parse_args()

if len(args.perf) > 0:
    BENCHMARK_SUITE.perf = ','.join(args.perf)

BENCHMARK_SUITE.run(args.algorithms, args.benchmarks, args.invocations)

