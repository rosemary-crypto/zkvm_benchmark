# Specific Rust version as the base image with buildx for multi-platform support
FROM rust:1.75-slim-bullseye

# Build arguments
ARG TARGETPLATFORM
ARG BUILDPLATFORM

# Install platform-specific dependencies
RUN apt-get update && apt-get install -y \
    git \
    cmake \
    build-essential \
    pkg-config \
    libssl-dev \
    python3 \
    python3-pip \
    valgrind \
    && if [ "$TARGETPLATFORM" = "linux/amd64" ] || [ "$TARGETPLATFORM" = "linux/arm64" ]; then \
        apt-get install -y linux-tools-generic linux-tools-common \
        && kernel_version=$(uname -r) \
        && apt-get install -y linux-tools-${kernel_version} linux-headers-${kernel_version} || true; \
        echo "Warning: performance tools installation failed, continuing without them"; \
    fi \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* \
    && rm -rf /tmp/* /var/tmp/*

# Create and set working directory for external repositories
WORKDIR /zk-repos

# Clone repositories with specific hashes
RUN git clone https://github.com/privacy-scaling-explorations/halo2.git && \
    cd halo2 && \
    git checkout 0661f467cbf6655e8419876a7c743d4ce89e19e1 && \
    cd .. && \
    rm -rf halo2/.git && \
    \
    git clone https://github.com/Plonky3/Plonky3.git && \
    cd Plonky3 && \
    git checkout 02f9f5306761b638008e2aace86ee67695fa2062 && \
    cd .. && \
    rm -rf Plonky3/.git && \
    \
    git clone https://github.com/0xPolygonMiden/miden-vm.git && \
    cd miden-vm && \
    git checkout b13ed4d67c13d99d750d7b91293a91b0185cb84d && \
    cd .. && \
    rm -rf miden-vm/.git && \
    \
    git clone https://github.com/risc0/risc0.git && \
    cd risc0 && \
    git checkout f1ecad14b84d92e989103aa3cc61108257703ad5 && \
    cd .. && \
    rm -rf risc0/.git && \
    \
    git clone https://github.com/nexus-xyz/nexus-zkvm.git && \
    cd nexus-zkvm && \
    git checkout f37401c477b680ce5334b2ca523ded8a7273d8c8 && \
    cd .. && \
    rm -rf nexus-zkvm/.git && \
    \
    git clone https://github.com/AleoHQ/snarkOS.git && \
    cd snarkOS && \
    git checkout 01b5d0fbeec83d3ac3e64d0196c6e8bd80c1c93c && \
    cd .. && \
    rm -rf snarkOS/.git && \
    \
    git clone https://github.com/AleoHQ/snarkVM.git && \
    cd snarkVM && \
    git checkout dea322b3d374ac0fa7dae26cf006bf67acd52e9e && \
    cd .. && \
    rm -rf snarkVM/.git

# Switch to benchmarking workspace
WORKDIR /zkvm-benchmarking

# Install Python dependencies for benchmarking
COPY requirements.txt .
RUN pip3 install -r requirements.txt && \
    rm -rf ~/.cache/pip/*

# Copy benchmarking scripts
COPY scripts/ /zkvm-benchmarking/scripts/
RUN chmod +x /zkvm-benchmarking/scripts/*.sh

# Environment variables for controlling benchmarks
ENV BENCHMARK_ITERATIONS=100
ENV BENCHMARK_WARMUP_ITERATIONS=10
ENV COLLECT_MEMORY_STATS=true
ENV COLLECT_PROOF_SIZES=true
ENV RUST_LOG=info
ENV RUSTFLAGS="-C target-cpu=native"

# Create a healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python3 -c "import os; exit(0 if os.path.exists('/zkvm-benchmarking') else 1)"

# Set the default command to run benchmarks
CMD ["./scripts/run_all_benchmarks.sh"]