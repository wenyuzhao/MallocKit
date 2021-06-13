# Rakefile
task default: []

at_exit { sleep 1 }

def 🔵(command, cwd: '.', log: true)
    log && puts("🔵 #{command}")
    system("cd #{cwd} && #{command}") || raise('❌')
end



$release = ENV["profile"] == "release"
malloc = ENV["malloc"] || "bump"
stat = ENV["stat"] || false
perf_events = 'page-faults,instructions,dTLB-loads,dTLB-load-misses,cache-misses,cache-references'
test_program = ENV["program"] || "cargo"
benchmark = ENV["bench"] || "alloc-test1"



import "./bench/bench.rb"

task :build do
    features = []
    stat && features.push("stat")
    args = []
    $release && args.push("--release")
    features.length() != 0 && args.push("--features=" + features.join(','))
    🔵 "cargo build #{args.join(' ')}"
    target_dir = "./target/" + ($release ? "release" : "debug")
	`llvm-objdump -d -S #{target_dir}/lib#{malloc}.a > #{target_dir}/lib#{malloc}.s 2>/dev/null`
end

task :test => :build do
    if test_program
        dylib = BenchmarkSuite::get_malloc_dylib(malloc)
        if (/darwin/ =~ RUBY_PLATFORM) != nil
            execute test_program, perf:false, env:{'DYLD_INSERT_LIBRARIES' => dylib}
        else
            execute test_program, perf:perf_events, env:{'LD_PRELOAD' => dylib}
        end
    else
        bench = BenchmarkSuite::get(benchmark)
        bench.run malloc, perf:perf_events
    end
end

task :lldb => :build do
    dylib = BenchmarkSuite::get_malloc_dylib(malloc)
    cmd = ARGV[(ARGV.index("--") + 1)..-1].join(" ")
    🔵 "rust-lldb -b -o 'settings set auto-confirm true' -o 'env LD_PRELOAD=#{dylib}' -o 'r' -- '#{cmd}' "
    exit 0
end

task :release do
    $release = true
end

task :bench => [:release, :build] do
    BenchmarkSuite::run
end
