FROM nvidia/cuda:12.8.1-devel-ubuntu24.04 as build-env

RUN apt-get update \
  && DEBIAN_FRONTEND=noninteractive \
  apt-get install --no-install-recommends --assume-yes \
  curl build-essential pkg-config git ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Install rustup and the latest stable toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

ENV PATH="/root/.cargo/bin:${PATH}"

# Install SP1 toolchain (this is done early to benefit from caching)
RUN curl -L https://sp1.succinct.xyz | bash
RUN /root/.sp1/bin/sp1up

# FIXME: cargo isn't installed for sp1 correctly otherwise (maybe 1.85-dev related?)
# RUN rustup update stable

####################################################################################################
## Dependency stage: Cache Cargo dependencies via cargo fetch
####################################################################################################
FROM build-env AS deps

# TODO: consider using https://github.com/LukeMathWalker/cargo-chef

WORKDIR /app

# Copy only the Cargo files that affect dependency resolution.
COPY Cargo.lock Cargo.toml ./
COPY service/Cargo.toml ./service/
COPY zkVM/common/Cargo.toml ./zkVM/common/
COPY zkVM/sp1/Cargo.toml ./zkVM/sp1/
COPY zkVM/sp1/program-chacha/Cargo.toml ./zkVM/sp1/program-chacha/

# Create dummy targets for each workspace member so that cargo fetch can succeed.
RUN mkdir -p service/src && echo 'fn main() {}' > service/src/main.rs && \
  mkdir -p zkVM/common/src && echo 'fn main() {}' > zkVM/common/src/lib.rs && \
  mkdir -p zkVM/sp1/src && echo 'fn main() {}' > zkVM/sp1/src/main.rs && \
  mkdir -p zkVM/sp1/program-chacha/src && echo 'fn main() {}' > zkVM/sp1/program-chacha/src/main.rs

# Run cargo fetch so that dependency downloads are cached in the image.
RUN cargo fetch

####################################################################################################
## Builder stage: Build the application using cached dependencies
####################################################################################################
FROM build-env AS builder

WORKDIR /app

# Import the cached Cargo registry from the deps stage.
COPY --from=deps /root/.cargo /root/.cargo

COPY . .

RUN --mount=type=cache,id=target_cache,target=/app/target \
  /root/.sp1/bin/cargo-prove prove build -p chacha-program

RUN --mount=type=cache,id=target_cache,target=/app/target \
  cargo build --release && \
  cp /app/target/release/pda-proxy /app/pda-proxy

####################################################################################################
## Final stage: Prepare the runtime image
####################################################################################################
FROM nvidia/cuda:12.8.1-base-ubuntu24.04

COPY --from=builder /app/pda-proxy ./

CMD ["/pda-proxy"]
