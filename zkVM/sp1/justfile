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

zkvm-elf-path := "./target/elf-compilation/riscv32im-succinct-zkvm-elf/release/chacha-program"
env-settings := "./.env"
sp1up-path := shell("which sp1up")
cargo-prove-path := shell("which cargo-prove")

# Install SP1 tooling & more
initial-config-installs:
    #!/usr/bin/env bash
    if ! {{ path_exists(sp1up-path) }}; then
        curl -L https://sp1.succinct.xyz | bash
    fi
    echo "✅ sp1up installed"

    if ! {{ path_exists(cargo-prove-path) }}; then
        {{ sp1up-path }}
    else
        echo -e "✅ cargo-prove installed\n     ⚠️👀NOTE: Check you have the correct version needed for this project!"
    fi

_pre-build:
    #!/usr/bin/env bash
    if ! {{ path_exists(cargo-prove-path) }}; then
        echo -e "⛔ Missing zkVM Compiler.\nRun `just initial-config-installs` to prepare your environment"
        exit 1
    fi
    if ! {{ path_exists(zkvm-elf-path) }}; then
        cargo prove build -p chacha-program
    fi

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
    docker build -t pda-proxy .

# Run a pre-built docker image
docker-run:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    mkdir -p $PDA_DB_PATH
    docker run --rm -it -v $PDA_DB_PATH:$PDA_DB_PATH --env-file {{ env-settings }} --env RUST_LOG=pda_proxy=debug --network=host -p $PDA_PORT:$PDA_PORT pda-proxy

# Build docker image & tag `pda-proxy`
podman-build:
    podman build -t pda-proxy .

# Run a pre-built podman image
podman-run:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source .env
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
    xdg-open {{ justfile_directory() }}/target/doc/index.html

# Launch a local Celestia testnet: Mocha
mocha:
    # Assumes you already did init for this & configured
    # If not, see https://docs.celestia.org/tutorials/node-tutorial#setting-up-dependencies
    celestia light start --core.ip rpc-mocha.pops.one --p2p.network mocha

# Setup and print to stdout, needs to be set in env to be picked up by pda-proxy
mocha-local-auth:
    celestia light auth admin --p2p.network mocha
