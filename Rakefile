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

task :test do
    args = []
    $release && args.push("--release")
    🔵 "cargo build #{args.join(' ')}"
    🔵 "cargo test"
end

task :gdb => :build do
    dylib_env = BenchmarkSuite::get_env(malloc)
    cmd = ARGV[(ARGV.index("--") + 1)..-1].join(" ")
    🔵 "rust-gdb -ex='set confirm on' -ex 'set environment #{dylib_env}' -ex 'run' -ex 'quit' --args #{cmd}"
    exit 0
end

task :lldb => :build do
    dylib_env = BenchmarkSuite::get_env(malloc)
    cmd = ARGV[(ARGV.index("--") + 1)..-1].join(" ")
    🔵 "rust-lldb -b -o 'settings set auto-confirm true' -o 'env #{dylib_env}' -o 'run' -- #{cmd}"
    exit 0
end

task :release do
    $release = true
end

task :bench => [:release, :build] do
    BenchmarkSuite::run
end
