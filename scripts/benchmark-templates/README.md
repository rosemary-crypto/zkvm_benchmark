# ZK Benchmark Templates

This directory contains benchmark templates for various ZK proving systems. These templates provide a standardized way to measure:
- Proving time
- Verification time
- Proof size
- Memory usage
- CPU usage

## Directory Structure
```
benchmarks/
├── halo2/
│   ├── ecdsa_benchmark.rs
│   ├── sha256_benchmark.rs
│   └── fibonacci_benchmark.rs
├── plonky3/
│   └── ...
└── ...
```

## How to Use

1. Implement your ZK circuit in the respective system
2. Copy the appropriate benchmark template
3. Fill in the template with calls to your implementation
4. Run the benchmark using the provided infrastructure

## Metrics Collection
Each benchmark template will collect:
- Execution times
- Memory usage
- Proof sizes
- System resource utilization

## Output Format
All benchmarks output JSON in this format:
```json
{
    "operation": "operation_name",
    "system": "zk_system_name",
    "circuit_size": "size_category",
    "timestamp": "ISO8601_timestamp",
    "measurements": {
        "proving_time_ms": 0,
        "verification_time_ms": 0,
        "proof_size_bytes": 0,
        "memory_usage_kb": 0
    }
}
```