default:
    @just --list

alias r := run-debug
alias rr := run-release
alias db := docker-build
alias ds := docker-save
alias dl := docker-load
alias dr := docker-run
alias b := build-debug
alias br := build-release
alias f := fmt
alias c := clean

# Bash, fail with undefined vars

set shell := ["bash", "-cu"]
set quiet := true

env-settings := ".env"

# SP1 just recipies
sp1 *args:
    @just --justfile ./zkVM/sp1/justfile {{ args }}

# Install SP1 tooling & more
initial-config-installs:
    @just sp1 initial-config-installs

_load-env:
    if ! {{ path_exists(env-settings) }}; then \
      echo -e "⛔ Missing required \`{{ env-settings }}\` file.\nCreate one with:\n\n\tcp example.env .env\n\nAnd then edit to adjust settings"; \
      exit 1; \
    fi; \
    set -a; \
    source {{ env-settings }};

_pre-build: _load-env
    if [ ! -f "$ZK_PROGRAM_ELF_PATH" ]; then \
      echo -e "⛔ Missing required \`$ZK_PROGRAM_ELF_PATH\` file.\nAttempting to build it..."; \
      just sp1 build-elf; \
    fi

_pre-run:
    echo "just pre-run TODO"

# Run in release mode, with optimizations AND debug logs
run-release *FLAGS: _pre-build _pre-run
    RUST_LOG=pda_proxy=debug cargo r -r -- {{ FLAGS }}

# Run in debug mode, with extra pre-checks, no optimizations
run-debug *FLAGS: _pre-build _pre-run
    # TODO :Check DA node up with some healthcheck endpoint
    RUST_LOG=pda_proxy=debug cargo r -- {{ FLAGS }}

# Build docker image & tag `pda-proxy`
docker-build:
    docker build --build-arg BUILDKIT_INLINE_CACHE=1 --tag pda-proxy --progress=plain .

# Save docker image to a tar.gz
docker-save:
    docker save pda-proxy | gzip > /tmp/pda-proxy-docker.tar.gz

# Load docker image from tar.gz
docker-load:
    gunzip -c /tmp/pda-proxy-docker.tar.gz | docker load

# Run a pre-built docker image
docker-run: _load-env
    mkdir -p $PDA_DB_PATH
    # Note socket assumes running "normally" with docker managed by root
    # TODO: support docker rootless!
    # FIXME: better way to share files accross .env file!
    docker run --rm -it \
      -v /var/run/docker.sock:/var/run/docker.sock \
      -v ./service/static:/app/static \
      -v $PDA_DB_PATH:$PDA_DB_PATH \
      --env-file {{ env-settings }} \
      --env TLS_CERTS_PATH=/app/static/sample.pem --env TLS_KEY_PATH=/app/static/sample.rsa \
      --env RUST_LOG=pda_proxy=debug \
      --network=host -p $PDA_PORT:$PDA_PORT \
      pda-proxy

# Build in debug mode, no optimizations
build-debug: _pre-build
    cargo b

# Build in release mode, includes optimizations
build-release: _pre-build
    cargo b -r

# Scrub build artifacts
clean:
    cargo clean

# Format source code (Rust, Justfile, and TOMLs)
fmt:
    cargo fmt # *.rs
    just --quiet --unstable --fmt > /dev/null # justfile
    taplo format > /dev/null 2>&1 # *.toml

# Build & open Rustdocs for the workspace
doc:
    RUSTDOCFLAGS="--enable-index-page -Zunstable-options" cargo +nightly doc --no-deps --workspace
    firefox {{ justfile_directory() }}/target/doc/index.html
    # TODO fix snadbox issues with flatpak
    # xdg-open {{ justfile_directory() }}/target/doc/index.html

# Launch a local Celestia testnet: Mocha
mocha:
    # Assumes you already did init for this & configured
    # If not, see https://docs.celestia.org/tutorials/node-tutorial#setting-up-dependencies
    celestia light start --core.ip rpc-mocha.pops.one --p2p.network mocha

# Setup and print to stdout, needs to be set in env to be picked up by pda-proxy
mocha-local-auth:
    celestia light auth admin --p2p.network mocha

# Test blob.Get
curl-blob-get: _load-env
    curl -H "Content-Type: application/json" -H "Authorization: Bearer $CELESTIA_NODE_WRITE_TOKEN" -X POST --data '{"id": 1,"jsonrpc": "2.0", "method": "blob.Get", "params": [ 42, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAMJ/xGlNMdE=", "aHlbp+J9yub6hw/uhK6dP8hBLR2mFy78XNRRdLf2794=" ] }' 127.0.0.1:3001

# Test blob.Submit
curl-blob-submit: _load-env
    curl -H "Content-Type: application/json" -H "Authorization: Bearer $CELESTIA_NODE_WRITE_TOKEN" -X POST \
         --data '{ "id": 1, "jsonrpc": "2.0", "method": "blob.Submit", "params": [ [ { "namespace": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAMJ/xGlNMdE=", "data": "DEADB33F", "share_version": 0, "commitment": "aHlbp+J9yub6hw/uhK6dP8hBLR2mFy78XNRRdLf2794=", "index": -1 } ], { } ] }' \
         https://127.0.0.1:26657 \
         --insecure -v
