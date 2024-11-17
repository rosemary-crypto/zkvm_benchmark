#!/usr/bin/env bash

#
# ZK Proving Systems Benchmark Suite
# Compares performance metrics across different ZK proving implementations
#

set -euo pipefail
IFS=$'\n\t'

# Constants
readonly LOG_DIR="/zkvm-benchmarking/logs"
readonly RESULTS_DIR="/zkvm-benchmarking/results"
readonly TIMESTAMP=$(date +%Y%m%d_%H%M%S)
readonly LOG_FILE="${LOG_DIR}/benchmark_${TIMESTAMP}.log"

# Default settings (can be overridden via env vars)
: "${BENCHMARK_ITERATIONS:=100}"
: "${BENCHMARK_WARMUP_ITERATIONS:=10}"
: "${COLLECT_MEMORY_STATS:=true}"

# Setup
mkdir -p "$LOG_DIR" "$RESULTS_DIR"

#
# Utility Functions
#
log() {
    local msg="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$msg" | tee -a "$LOG_FILE"
}

fail() {
    log "ERROR: $1"
    exit 1
}

get_memory_usage() {
    ps -o rss= -p "$1" 2>/dev/null || echo 0
}

check_dependencies() {
    local deps=("jq" "bc" "python3")
    for dep in "${deps[@]}"; do
        command -v "$dep" >/dev/null 2>&1 || fail "Required dependency not found: $dep"
    done
}

get_system_info() {
    local info="{"
    
    # CPU Info
    info+="\"cpu\":\"$(cat /proc/cpuinfo | grep "model name" | head -n1 | cut -d: -f2 | xargs)\","
    
    # Memory Info
    info+="\"memory\":\"$(free -h | grep Mem | awk '{print $2}')\","
    
    # OS Info
    info+="\"os\":\"$(uname -s) $(uname -r)\","
    
    # Rust Version
    info+="\"rust\":\"$(rustc --version)\""
    
    info+="}"
    echo "$info"
}

monitor_resources() {
    local pid=$1
    local output_file=$2
    
    while kill -0 $pid 2>/dev/null; do
        local cpu_usage=$(ps -p $pid -o %cpu | tail -n1)
        local mem_usage=$(ps -p $pid -o rss | tail -n1)
        local timestamp=$(date +%s.%N)
        
        echo "{\"timestamp\":$timestamp,\"cpu\":$cpu_usage,\"memory\":$mem_usage}" >> "$output_file"
        sleep 0.1
    done
}

#
# Benchmark Core
#
run_single_benchmark() {
    local system=$1
    local op=$2
    local bench_path="/zkvm-benchmarking/${system}/target/release/bench"
    local result_file="${RESULTS_DIR}/${system}_${op}_${TIMESTAMP}.json"
    
    # Sanity checks
    [[ -x "$bench_path" ]] || fail "Benchmark binary not found or not executable: $bench_path"
    
    log "Starting benchmark: $system ($op)"
    
    # System info
    local sys_info
    sys_info=$(cat /proc/cpuinfo | grep "model name" | head -n1 || echo "Unknown CPU")
    
    # Initialize results file
    cat > "$result_file" << EOF
{
    "system": "$system",
    "operation": "$op",
    "timestamp": "$TIMESTAMP",
    "system_info": "$sys_info",
    "measurements": []
}
EOF
    
    # Warmup phase
    log "Warming up..."
    for i in $(seq "$BENCHMARK_WARMUP_ITERATIONS"); do
        "$bench_path" "$op" >/dev/null 2>&1 || true
    done
    
    # Main benchmark loop
    log "Running measurements..."
    for i in $(seq "$BENCHMARK_ITERATIONS"); do
        local start_time peak_mem output end_time
        
        start_time=$(date +%s.%N)
        peak_mem=0
        
        # Memory monitoring
        if [[ "$COLLECT_MEMORY_STATS" == "true" ]]; then
            while sleep 0.1; do
                local current_mem
                current_mem=$(get_memory_usage "$$")
                (( current_mem > peak_mem )) && peak_mem=$current_mem
            done & local mem_pid=$!
        fi
        
        # Run benchmark
        output=$("$bench_path" "$op" 2>&1)
        end_time=$(date +%s.%N)
        
        # Cleanup memory monitor
        [[ "$COLLECT_MEMORY_STATS" == "true" ]] && kill $mem_pid 2>/dev/null || true
        
        # Parse metrics
        local prove_time verify_time proof_size
        prove_time=$(echo "$output" | grep -oP 'Proving time: \K[\d.]+' || echo 0)
        verify_time=$(echo "$output" | grep -oP 'Verification time: \K[\d.]+' || echo 0)
        proof_size=$(echo "$output" | grep -oP 'Proof size: \K[\d.]+' || echo 0)
        
        # Record results
        local duration
        duration=$(echo "$end_time - $start_time" | bc)
        
        jq --arg pt "$prove_time" \
           --arg vt "$verify_time" \
           --arg ps "$proof_size" \
           --arg pm "$peak_mem" \
           --arg d "$duration" \
           '.measurements += [{
               "iteration": '$i',
               "prove_time": $pt,
               "verify_time": $vt,
               "proof_size": $ps,
               "peak_memory_kb": $pm,
               "duration": $d
           }]' "$result_file" > "${result_file}.tmp" && mv "${result_file}.tmp" "$result_file"
        
        log "Iteration $i/$BENCHMARK_ITERATIONS complete"
    done
    
    # Calculate summary stats
    jq '
        .summary = {
            "avg_prove_time": (.measurements | map(.prove_time | tonumber) | add / length),
            "avg_verify_time": (.measurements | map(.verify_time | tonumber) | add / length),
            "avg_proof_size": (.measurements | map(.proof_size | tonumber) | add / length),
            "peak_memory": (.measurements | map(.peak_memory_kb | tonumber) | max),
            "min_prove_time": (.measurements | map(.prove_time | tonumber) | min),
            "max_prove_time": (.measurements | map(.prove_time | tonumber) | max)
        }
    ' "$result_file" > "${result_file}.tmp" && mv "${result_file}.tmp" "$result_file"
}

#
# Main
#
main() {
    log "Starting ZK Proving Systems Benchmark Suite"
    
    local -a systems=("halo2" "plonky3" "miden" "risc0" "jolt" "nexus" "aleo")
    local -a ops=("ecdsa")
    
    for sys in "${systems[@]}"; do
        for op in "${ops[@]}"; do
            run_single_benchmark "$sys" "$op"
        done
    done
    
    log "Generating report..."
    python3 /zkvm-benchmarking/scripts/generate_report.py \
        --results-dir "$RESULTS_DIR" \
        --timestamp "$TIMESTAMP" \
        --output "${RESULTS_DIR}/report_${TIMESTAMP}.html"
    
    log "Benchmark suite completed"
}

# Execute main if script is run directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi