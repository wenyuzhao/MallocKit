from .benchmarks import BENCHMARK_SUITE
import argparse

parser = argparse.ArgumentParser()
parser.add_argument('-a', '--algorithms', nargs='*', default=BENCHMARK_SUITE.algorithms)
parser.add_argument('-b', '--benchmarks', nargs='*', default=[b.name for b in BENCHMARK_SUITE.benchmarks])
parser.add_argument('-i', '--invocations', nargs='?', type=int, default=1)
parser.add_argument('-p', '--perf', nargs='?', default=None)

args = parser.parse_args()

if args.perf is not None:
    BENCHMARK_SUITE.perf = args.perf

BENCHMARK_SUITE.run(args.algorithms, args.benchmarks, args.invocations)

