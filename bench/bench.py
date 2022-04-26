import subprocess
from typing import List, Optional
from os import path
import pandas as pd
from datetime import datetime
import socket

BENCH_DIR = path.dirname(path.abspath(__file__))
DEFAULT_BENCH_CWD = f'{BENCH_DIR}/mimalloc-bench/out/bench'
MIMALLOC_BENCH_SRC_DIR = f'{BENCH_DIR}/mimalloc-bench/bench'
MIMALLOC_BENCH_EXTERN_DIR = f'{BENCH_DIR}/mimalloc-bench/extern'
PROJECT_DIR = path.dirname(BENCH_DIR)

RUNID = socket.gethostname() + '-' + datetime.now().strftime("%Y-%m-%d-%H%M%S")
BENCH_LOGS_DIR = f'{BENCH_DIR}/results/{RUNID}'
TEMP_REPORT_FILE = f'{BENCH_LOGS_DIR}/.temp.csv'
RESULTS_FILE = f'{BENCH_LOGS_DIR}/results.csv'


class Benchmark:
    name = None
    record = False
    test = False
    lldb = False

    def __init__(self):
        self.current_invocation = None
        self.current_malloc = None

    def set_invocation_info(self, malloc: str, invocation: int, perf: Optional[str]):
        self.current_invocation = invocation
        self.current_malloc = malloc
        self.perf = perf

    def clear_invocation_info(self):
        self.current_invocation = None
        self.current_malloc = None
        self.perf = None

    def prologue(self):
        ...

    def run(self, env: List[str]):
        raise NotImplementedError()

    def epilogue(self):
        ...

    def exec(self, cmd: str, cwd: Optional[str] = None):
        return subprocess.check_call(cmd, shell=True, text=True, cwd=cwd)

    def __measure_record(self, cmd: str, env: List[str] = [], cwd: str = DEFAULT_BENCH_CWD, infile: Optional[str] = None):
        assert Benchmark.record
        # Prepare commands
        perf_wrapper = f'perf record -o {PROJECT_DIR}/perf.data'
        if self.perf is not None:
            perf_wrapper += f' -e {self.perf}'
        perf_wrapper += ' --'
        env_wrapper = 'env'
        for e in env:
            env_wrapper += f' {e}'
        command = f'{perf_wrapper} {env_wrapper} {cmd}'
        # Run
        print(f'🚀 [{self.name}] #{self.current_invocation} {self.current_malloc}')
        print(f'>  {command}')
        if infile is not None:
            with open(infile, 'r') as infile:
                subprocess.check_call(
                    command, shell=True, text=True, cwd=cwd, stdin=infile)
        else:
            subprocess.check_call(command, shell=True, text=True, cwd=cwd)
        print(f'Please run `perf report`.')

    def __measure_test(self, cmd: str, env: List[str] = [], cwd: str = DEFAULT_BENCH_CWD, infile: Optional[str] = None):
        assert Benchmark.test or Benchmark.lldb
        # Prepare commands
        env_wrapper = 'env'
        for e in env:
            env_wrapper += f' {e}'
        command = f'rust-lldb -o "{env_wrapper}" -- {cmd}' if Benchmark.lldb else f'{env_wrapper} {cmd}'
        # Run
        print(f'🚀 [{self.name}] #{self.current_invocation} {self.current_malloc}')
        print(f'>  {command}')
        if infile is not None:
            with open(infile, 'r') as infile:
                subprocess.check_call(
                    command, shell=True, text=True, cwd=cwd, stdin=infile)
        else:
            subprocess.check_call(command, shell=True, text=True, cwd=cwd)

    def measure(self, cmd: str, env: List[str] = [], cwd: str = DEFAULT_BENCH_CWD, infile: Optional[str] = None):
        if Benchmark.record:
            return self.__measure_record(cmd, env, cwd, infile)
        if Benchmark.test or Benchmark.lldb:
            return self.__measure_test(cmd, env, cwd, infile)
        # Prepare commands
        perf_wrapper = f'perf stat --no-scale -o {TEMP_REPORT_FILE} -x ,'
        if self.perf is not None:
            perf_wrapper += f' -e {self.perf}'
        perf_wrapper += ' --'
        env_wrapper = 'env'
        wrapper = PROJECT_DIR + '/bench/wrapper.sh'
        for e in env:
            env_wrapper += f' {e}'
        command = f'{perf_wrapper} {wrapper} {env_wrapper} {cmd}'
        # Run
        print(f'🚀 [{self.name}] #{self.current_invocation} {self.current_malloc}')
        success = True
        with open(f'{BENCH_LOGS_DIR}/{self.name}-{self.current_malloc}.log', 'a') as file:
            file.write(
                f'---------- Invocation #{self.current_invocation} ----------\n\n')
            file.write(f'> {command}\n\n')
            file.flush()
            try:
                if infile is not None:
                    with open(infile, 'r') as infile:
                        subprocess.check_call(
                            command, shell=True, text=True, cwd=cwd, stdout=file, stderr=file, stdin=infile)
                else:
                    subprocess.check_call(
                        command, shell=True, text=True, cwd=cwd, stdout=file, stderr=file)
            except Exception as e:
                file.write(f'\n\nERROR\n\n')
                print(f'❌ {self.name} did not complete successfully.')
                success = False
            file.flush()
            with open(TEMP_REPORT_FILE, 'r') as csv:
                file.write(f'\n> results\n\n{csv.read()}\n\n\n\n')
                file.flush()
        if success:
            # Parse report
            self.exec(f"sed -i '1,2d' {TEMP_REPORT_FILE}")
            df = pd.read_csv(TEMP_REPORT_FILE, header=None)
            df = df.iloc[:, [0, 2]].T
            df.iloc[[0, 1], :] = df.iloc[[1, 0], :]
            df.insert(loc=0, column='malloc', value=[
                      'malloc', self.current_malloc])
            df.insert(loc=0, column='bench', value=['bench', self.name])
            df.insert(loc=0, column='invocation', value=[
                      'invocation', self.current_invocation])
            if not path.isfile(RESULTS_FILE):
                df.loc[[0]].to_csv(RESULTS_FILE, header=False, index=False)
            df.loc[[2]].to_csv(RESULTS_FILE, header=False,
                               index=False, mode='a')
            print(df.to_string(header=False, index=False))


class BenchmarkSuite:
    debug = False
    sys_malloc = 'sys'
    # Installed allocators: dh, ff, hd, hm, iso, je, mi, mng, rp, scudo, sm, sn, tbb, tc
    non_mallockit_algorithms = {
        'je': f'{MIMALLOC_BENCH_EXTERN_DIR}/jemalloc/lib/libjemalloc.so',
        'tc': f'{MIMALLOC_BENCH_EXTERN_DIR}/gperftools/.libs/libtcmalloc_minimal.so',
        'sn': f'{MIMALLOC_BENCH_EXTERN_DIR}/snmalloc/release/libsnmallocshim.so',
        'mi': f'{MIMALLOC_BENCH_EXTERN_DIR}/mimalloc/out/release/libmimalloc.so',
        # 'tbb': f'{BENCH_EXTERN_DIR}/',
        'hd': f'{MIMALLOC_BENCH_EXTERN_DIR}/Hoard/src/libhoard.so',
        'sm': f'{MIMALLOC_BENCH_EXTERN_DIR}/SuperMalloc/release/lib/libsupermalloc',
    }

    def __init__(self, benchmarks: List[Benchmark]):
        self.perf = None
        self.benchmarks = benchmarks
        self.algorithms = ['sys'] + \
            [x for x in self.non_mallockit_algorithms.keys()]

    def run(self, algorithms: List[str] = ['sys']):
        for bm in self.benchmarks:
            bm.run('')

    def __get_dylib(self, malloc: str) -> str:
        assert malloc != self.sys_malloc
        if malloc in self.non_mallockit_algorithms:
            return self.non_mallockit_algorithms[malloc]
        else:
            profile = 'debug' if BenchmarkSuite.debug else 'release'
            return f'{PROJECT_DIR}/target/{profile}/lib{malloc}.so'

    def __run_bm(self, bm: Benchmark, algorithms: List[str], invocation: int):
        for a in algorithms:
            env = f'LD_PRELOAD={self.__get_dylib(a)}' if a != self.sys_malloc else 'SYSMALLOC=1'
            bm.set_invocation_info(a, invocation, self.perf)
            bm.run([env])
            bm.clear_invocation_info()

    def run(self, algorithms: List[str] = ['sys'], benchmarks=['cfrac'], invocations: int = 1):
        if not (Benchmark.test or Benchmark.lldb or Benchmark.record):
            subprocess.check_call(f'mkdir -p {BENCH_LOGS_DIR}', shell=True)
            subprocess.check_call(
                f'rm {BENCH_DIR}/results/latest && ln -s {BENCH_LOGS_DIR} {BENCH_DIR}/results/latest', shell=True)
            subprocess.check_call(
                f'echo {RUNID} > {BENCH_LOGS_DIR}/runid', shell=True)
            print(f'🟢 [{RUNID}] Benchmark Start 🟢')
        for bm in self.benchmarks:
            if bm.name not in benchmarks:
                continue
            for i in range(invocations):
                self.__run_bm(bm, algorithms, i)
        print(f'🟢 [{RUNID}] Benchmark Finished 🟢')
