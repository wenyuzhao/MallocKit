

def execute(command, perf: true, env: {})
    perf_prefix = ""
    if perf != false
        events_arg = perf == "" || perf == true ? "" : "-e #{perf}"
        perf_prefix = "perf stat --no-scale #{events_arg}".strip
    end
    env_prefix = ""
    if env.length > 0
        env_prefix = "env"
        env.each {|k, v| env_prefix += " #{k}=#{v}" }
    end
    🔵 "#{env_prefix} #{perf_prefix} #{command}"
end

class Benchmark
    MIMALLOC_BENCH_OUT_BENCH_DIR = "./bench/mimalloc-bench/out/bench"

    attr_reader :name

    def initialize(name, bin, args)
        @name = name
        @bin = "#{MIMALLOC_BENCH_OUT_BENCH_DIR}/#{bin}"
        @args = args
    end

    def command
        "#{@bin} #{@args}"
    end

    def run(malloc, perf: true)
        dylib = BenchmarkSuite::get_malloc_dylib(malloc)
        execute "env LD_PRELOAD=#{dylib} #{command}", perf:perf
    end
end

class BenchmarkSuite
    private
    PROCS = `nproc`.to_i
    BENCH_DIR = "./bench/mimalloc-bench/bench"
    EXTERN_DIR = "./bench/mimalloc-bench/extern"
    if (/darwin/ =~ RUBY_PLATFORM) != nil
        DYLIB = "dylib"
    else
        DYLIB = "so"
    end

    public
    BENCHMARKS = [
        Benchmark.new("alloc-test1", "alloc-test", "1"),
        Benchmark.new("alloc-testN", "alloc-test", "#{[PROCS, 16].min}"),
        Benchmark.new("barnes", "barnes", "< #{BENCH_DIR}/barnes/input"),
        Benchmark.new("cache-scratch1", "cache-scratch", "1 1000 1 2000000 #{PROCS}"),
        Benchmark.new("cache-scratchN", "cache-scratch", "#{PROCS} 1000 1 2000000 #{PROCS}"),
        Benchmark.new("cfrac", "cfrac", "17545186520507317056371138836327483792789528"),
        Benchmark.new("espresso", "espresso", "#{BENCH_DIR}/espresso/largest.espresso"),
        Benchmark.new("larsonN", "larson", "5 8 1000 5000 100 4141 #{PROCS}"),
        # Benchmark.new("leanN", "barnes", "< #{BENCH_DIR}/barnes/input"),
        Benchmark.new("mstressN", "mstress", "#{PROCS} 50 25"),
        Benchmark.new("rptestN", "rptest", "#{18 > PROCS ? 16 : PROCS} 0 1 2 500 1000 100 8 16000"),
        Benchmark.new("sh6benchN", "sh6bench", "#{PROCS * 2}"),
        # Benchmark.new("sh8benchN", "sh8bench", "#{PROCS * 2}"), # Returns non-zero value on success
        Benchmark.new("xmalloc-testN", "xmalloc-test", "-w #{PROCS} -t 5 -s 64"),
    ]
    CONTROL_ALGORITHMS = {
        "sys" => "",
        "mi" => "#{EXTERN_DIR}/mimalloc/out/release/libmimalloc.#{DYLIB}",
        "smi" => "#{EXTERN_DIR}/mimalloc/out/secure/libmimalloc-secure.#{DYLIB}",
        "tc" => "#{EXTERN_DIR}/gperftools/.libs/libtcmalloc_minimal.#{DYLIB}",
        "je" => "#{EXTERN_DIR}/jemalloc/lib/libjemalloc.#{DYLIB}",
        "sn" => "#{EXTERN_DIR}/snmalloc/release/libsnmallocshim.#{DYLIB}",
        "hd" => "#{EXTERN_DIR}/Hoard/src/libhoard.#{DYLIB}",
    }

    def BenchmarkSuite.get_malloc_dylib(malloc)
        BenchmarkSuite::CONTROL_ALGORITHMS[malloc] || "./target/#{$release ? "release" : "debug"}/lib#{malloc}.#{DYLIB}"
    end

    def BenchmarkSuite.get(benchmark)
        BenchmarkSuite::BENCHMARKS.find {|bm| bm.name == benchmark}
    end

    def BenchmarkSuite.run(invocations: 10)
        mallockit_algorithms = Dir["./target/#{$release ? "release" : "debug"}/lib*.so"].map { |x| x.match("lib(?<name>\\w+)\\.#{DYLIB}")[:name] }
        algorithms = mallockit_algorithms + CONTROL_ALGORITHMS.keys
        for bm in BenchmarkSuite::BENCHMARKS do
            for i in 1..invocations do
                for a in algorithms do
                    bm.run a
                end
            end
        end
    end
end

