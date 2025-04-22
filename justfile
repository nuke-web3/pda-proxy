default:
    @just --list

alias r := run-debug
alias rr := run-release
alias db := docker-build
alias dr := docker-run
alias pb := podman-build
alias pr := podman-run
alias b := build-debug
alias br := build-release
alias f := fmt
alias c := clean

env-settings := "./.env"

# SP1 just recipies
sp1 *args:
    @just --justfile ./zkVM/sp1/justfile {{ args }}

# Install SP1 tooling & more
initial-config-installs:
    @just sp1 initial-config-installs

_pre-build:
    #!/usr/bin/env bash
    echo TODO

_pre-run:
    #!/usr/bin/env bash
    if ! {{ path_exists(env-settings) }}; then
        echo -e "⛔ Missing required \`{{ env-settings }}\` file.\nCreate one with:\n\n\tcp example.env .env\n\nAnd then edit to adjust settings"
        exit 1
    fi

# Run in release mode, with optimizations AND debug logs
run-release *FLAGS: _pre-build _pre-run
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    RUST_LOG=pda_proxy=debug cargo r -r -- {{ FLAGS }}

# Run in debug mode, with extra pre-checks, no optimizations
run-debug *FLAGS: _pre-build _pre-run
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    # TODO :Check node up with some healthcheck endpoint

    # export CELESTIA_NODE_AUTH_TOKEN=$(celestia light auth admin --p2p.network mocha)
    RUST_LOG=pda_proxy=debug cargo r -- {{ FLAGS }}

# Build docker image & tag `pda-proxy`
docker-build:
    docker build --build-arg BUILDKIT_INLINE_CACHE=1 --tag pda-proxy --progress=plain .

# Run a pre-built docker image
docker-run:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    mkdir -p $PDA_DB_PATH
    # Note socket assumes running "normally" with docker managed by root
    # TODO: support docker rootless!
    docker run --rm -it -v /var/run/docker.sock:/var/run/docker.sock -v $PDA_DB_PATH:$PDA_DB_PATH --env-file {{ env-settings }} --env RUST_LOG=pda_proxy=debug --network=host -p $PDA_PORT:$PDA_PORT pda-proxy

# Build docker image & tag `pda-proxy`
podman-build:
    podman build --layers --tag pda-proxy --log-level=debug .

# Run a pre-built podman image
podman-run:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    mkdir -p $PDA_DB_PATH
    podman run --rm -it -v $PDA_DB_PATH:$PDA_DB_PATH --env-file {{ env-settings }} --env RUST_LOG=pda_proxy=debug --network=host -p $PDA_PORT:$PDA_PORT pda-proxy

# Build in debug mode, no optimizations
build-debug: _pre-build
    cargo b

# Build in release mode, includes optimizations
build-release: _pre-build
    cargo b -r

# Scrub build artifacts
clean:
    #!/usr/bin/env bash
    cargo clean

# Format source code
fmt:
    @cargo fmt
    @just --quiet --unstable --fmt > /dev/null

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

# Test proxy
curl:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    curl -H "Content-Type: application/json" -H "Authorization: Bearer $CELESTIA_NODE_AUTH_TOKEN" -X POST --data '{"id": 1,"jsonrpc": "2.0", "method": "blob.Get", "params": [ 42, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAMJ/xGlNMdE=", "aHlbp+J9yub6hw/uhK6dP8hBLR2mFy78XNRRdLf2794=" ] }' 127.0.0.1:3001
