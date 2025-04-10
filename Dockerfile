# First stage: Build environment (based on Rust)
FROM rust:latest AS build-env

RUN apt-get update \
  && DEBIAN_FRONTEND=noninteractive \
  apt-get install --no-install-recommends --assume-yes \
  protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

# Install SP1 toolchain (this is done early to benefit from caching)
RUN curl -L https://sp1.succinct.xyz | bash
RUN /root/.sp1/bin/sp1up

# FIXME: cargo isn't installed for sp1 correctly otherwise (maybe 1.85-dev related?)
RUN rustup update stable

####################################################################################################
## Dependency stage: Cache Cargo dependencies via cargo fetch
####################################################################################################
FROM build-env AS deps

WORKDIR /app

# Copy only the Cargo files that affect dependency resolution.
COPY Cargo.lock Cargo.toml ./
COPY service/Cargo.toml ./service/
COPY lib/Cargo.toml ./lib/
COPY runner-keccak-inclusion/Cargo.toml ./runner-keccak-inclusion/
COPY blob-tool/Cargo.toml ./blob-tool/
COPY program-keccak-inclusion/Cargo.toml ./program-keccak-inclusion/

# Create dummy targets for each workspace member so that cargo fetch can succeed.
RUN mkdir -p service/src && echo 'fn main() {}' > service/src/main.rs && \
  mkdir -p common/src && echo 'fn main() {}' > common/src/lib.rs && \
  mkdir -p sdk/src && echo 'fn main() {}' > sdk/src/lib.rs && \
  mkdir -p runner-keccak-inclusion/src && echo 'fn main() {}' > runner-keccak-inclusion/src/main.rs && \
  mkdir -p blob-tool/src && echo 'fn main() {}' > blob-tool/src/main.rs && \
  mkdir -p program-keccak-inclusion/src && echo 'fn main() {}' > program-keccak-inclusion/src/main.rs

# Run cargo fetch so that dependency downloads are cached in the image.
RUN cargo fetch

####################################################################################################
## Builder stage: Build the application using cached dependencies
####################################################################################################
FROM build-env AS builder

WORKDIR /app

# Import the cached Cargo registry from the deps stage.
COPY --from=deps /usr/local/cargo /usr/local/cargo

# Now copy the rest of your source code.
COPY . .

# Build ZK Program ELF using SP1 toolchain.
# Use BuildKit 
RUN --mount=type=cache,id=target_cache,target=/app/target \
  /root/.sp1/bin/cargo-prove prove build -p eq-program-keccak-inclusion

# Finally, compile the project in release mode.
RUN --mount=type=cache,id=target_cache,target=/app/target \
  cargo build --release && \
  cp /app/target/release/eq-service /app/eq-service

####################################################################################################
## Final stage: Prepare the runtime image
####################################################################################################
FROM debian:bookworm-slim
# We use bookwork as we need glibc
# We must have libcurl for the SP1 client to function!
# FIXME: we don't need to do this, so long as the image we use has updated ca-certs
# <https://github.com/succinctlabs/sp1/issues/2075#issuecomment-2704953661>
# RUN apt update && apt install -y libcurl4 && rm -rf /var/lib/apt/lists/*

# Copy system CA certs
# Without them, prover network requests will fail: <https://github.com/succinctlabs/sp1/issues/2075>
COPY --from=build-env /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

COPY --from=builder /app/eq-service ./

CMD ["/eq-service"]
