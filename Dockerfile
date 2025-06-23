# syntax=docker/dockerfile:1.3

####################################################################################################
# Base with Rust, Go, CUDA, SP1, and cargo-chef
####################################################################################################
FROM nvidia/cuda:12.9.1-devel-ubuntu24.04 AS base-dev

# Install system dependencies including docker CLI
RUN apt-get update && DEBIAN_FRONTEND=noninteractive \
  apt-get install --no-install-recommends -y \
    clang libclang-dev docker.io curl tar build-essential pkg-config git ca-certificates gnupg2 \
  && rm -rf /var/lib/apt/lists/*

# Install Go
ENV GO_VERSION=1.24.4 \
    GO_URL="https://go.dev/dl/go${GO_VERSION}.linux-amd64.tar.gz"
RUN curl -L --proto '=https' --tlsv1.2 -sSf ${GO_URL} -o go.tar.gz \
  && mkdir -p /opt/go \
  && tar -C /opt/go --strip-components=1 -xzf go.tar.gz \
  && rm go.tar.gz
ENV PATH="/opt/go/bin:${PATH}"

WORKDIR /app
COPY ./rust-toolchain.toml ./

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install cargo-chef
RUN cargo install cargo-chef

# Install SP1
RUN curl -L https://sp1up.succinct.xyz | bash && \
    /root/.sp1/bin/sp1up

####################################################################################################
# Planner Stage
####################################################################################################
FROM base-dev AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

####################################################################################################
# Builder Stage (with BuildKit Docker socket mount)
####################################################################################################
FROM base-dev AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json ./

# Restore and compile dependencies
RUN --mount=type=cache,id=target_cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

COPY . .

# Build SP1 ELF for proving
# Mounting host Docker socket needed for reproducible builds
RUN --mount=type=cache,id=target_cache,target=/app/target \
    --mount=type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
    RUSTFLAGS="-Copt-level=3 -Clto=fat -Ccodegen-units=1 -Cdebuginfo=1 -Cembed-bitcode=yes" \
    /root/.sp1/bin/cargo-prove prove build --docker -p chacha-program

# Build final binary
RUN --mount=type=cache,id=target_cache,target=/app/target \
    cargo build --release && \
    strip /app/target/release/pda-proxy && \
    cp /app/target/release/pda-proxy /app/pda-proxy

####################################################################################################
# Runtime Stage
####################################################################################################
FROM nvidia/cuda:12.9.1-base-ubuntu24.04 AS runtime

# Copy Docker CLI for SP1 CUDA support
COPY --from=base-dev /usr/bin/docker /usr/bin/docker
# Copy the built proxy binary
COPY --from=builder /app/pda-proxy /usr/local/bin/pda-proxy

ENTRYPOINT ["/usr/local/bin/pda-proxy"]

# Build command:
# DOCKER_BUILDKIT=1 docker build -t pda-proxy .
