#  Base with Rust, CUDA, SP1, and cargo-chef
FROM nvidia/cuda:12.8.1-devel-ubuntu24.04 AS base-dev

RUN apt-get update && DEBIAN_FRONTEND=noninteractive \
  apt-get install --no-install-recommends -y \
  curl build-essential pkg-config git ca-certificates gnupg2 \
  && rm -rf /var/lib/apt/lists/*

# Ensure we cache toolchain & components
WORKDIR /app
COPY ./rust-toolchain.toml ./

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# https://github.com/LukeMathWalker/cargo-chef/
RUN cargo install cargo-chef

# https://docs.succinct.xyz/docs/sp1/getting-started/install
RUN curl -L https://sp1up.succinct.xyz | bash && \
  /root/.sp1/bin/sp1up

####################################################################################################
FROM base-dev AS planner

WORKDIR /app

COPY . .

RUN cargo chef prepare --recipe-path recipe.json

####################################################################################################
FROM base-dev AS builder

WORKDIR /app
COPY --from=planner /app/recipe.json ./

RUN --mount=type=cache,id=target_cache,target=/app/target \
  cargo chef cook --release --recipe-path recipe.json

COPY . .

# Build SP1 ELF to be proven (with optimizations)
RUN --mount=type=cache,id=target_cache,target=/app/target \
  /root/.sp1/bin/cargo-prove prove build -p chacha-program

# Build the final binary
RUN --mount=type=cache,id=target_cache,target=/app/target \
  cargo build --release && \
  strip /app/target/release/pda-proxy && \
  cp target/release/pda-proxy /app/pda-proxy # pop out of cache

####################################################################################################
FROM nvidia/cuda:12.8.1-base-ubuntu24.04 AS runtime

COPY --from=builder /app/pda-proxy /usr/local/bin/pda-proxy

ENTRYPOINT ["/usr/local/bin/pda-proxy"]
