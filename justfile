default:
    @just --list

alias r := run-debug
alias rr := run-release
alias db := docker-build
alias ds := docker-save
alias dl := docker-load
alias dr := docker-run
alias pb := podman-build
alias pr := podman-run
alias b := build-debug
alias br := build-release
alias f := fmt
alias c := clean

env-settings := ".env"
artifacts-tar-gz-file := "/tmp/groth-circuits.tar.gz"
circuit-dir := "$HOME/.sp1/circuits/groth16/$SP1_CIRCUIT_VERSION"

# SP1 just recipies
sp1 *args:
    @just --justfile ./zkVM/sp1/justfile {{ args }}

# Install SP1 tooling & more
initial-config-installs:
    @just sp1 initial-config-installs

_pre-build:
    #!/usr/bin/env bash
    if ! {{ path_exists(env-settings) }}; then
        echo -e "⛔ Missing required \`{{ env-settings }}\` file.\nCreate one with:\n\n\tcp example.env .env\n\nAnd then edit to adjust settings"
        exit 1
    fi
    source {{ env-settings }}
    if [ ! -f "$ZK_PROGRAM_ELF_PATH" ]; then
      echo -e "⛔ Missing required \`$ZK_PROGRAM_ELF_PATH\` file.\nAttempting to build it..."
      just sp1 build-elf
    fi

_pre-run:
    #!/usr/bin/env bash
    echo "just pre-run TODO"

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

    # export CELESTIA_NODE_WRITE_TOKEN=$(celestia light auth admin --p2p.network mocha)
    RUST_LOG=pda_proxy=debug cargo r -- {{ FLAGS }}

# Build docker image & tag `pda-proxy`
docker-build:
    docker build --build-arg BUILDKIT_INLINE_CACHE=1 --tag pda-proxy --progress=plain .

# Save docker image to a tar.gz
docker-save:
    docker save pda-proxy | gzip > pda-proxy-docker.tar.gz

# Load docker image from tar.gz
docker-load:
    gunzip -c pda-proxy-docker.tar.gz | docker load

# Run a pre-built docker image
docker-run:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
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
    cargo fmt # *.rs
    just --quiet --unstable --fmt > /dev/null # justfile
    taplo format # *.toml

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
    curl -H "Content-Type: application/json" -H "Authorization: Bearer $CELESTIA_NODE_WRITE_TOKEN" -X POST --data '{"id": 1,"jsonrpc": "2.0", "method": "blob.Get", "params": [ 42, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAMJ/xGlNMdE=", "aHlbp+J9yub6hw/uhK6dP8hBLR2mFy78XNRRdLf2794=" ] }' 127.0.0.1:3001

# Download required groth16 artifacts
download-groth16-circuit:
    #!/usr/bin/env bash
    set -euo pipefail
    set -x  # start verbose output

    # TODO: don't have to manually upkeep these here if possible!
    # See <https://github.com/succinctlabs/sp1/blob/dev/crates/sdk/src/install.rs>
    # might make a mini utility bin for service to manually execute this instead
    #
    # Required environment variables:
    CIRCUIT_ARTIFACTS_URL_BASE="https://sp1-circuits.s3-us-east-2.amazonaws.com"
    ARTIFACTS_TYPE="groth16"
    SP1_CIRCUIT_VERSION="v4.0.0-rc.3"
    CIRCUIT_DIR="$HOME/.sp1/circuits/$ARTIFACTS_TYPE/$SP1_CIRCUIT_VERSION"

    DOWNLOAD_URL="${CIRCUIT_ARTIFACTS_URL_BASE}/${SP1_CIRCUIT_VERSION}-${ARTIFACTS_TYPE}.tar.gz"

    ARTIFACTS_TAR_GZ_FILE={{ artifacts-tar-gz-file }}

    echo "Downloading SP1 circuit files from $DOWNLOAD_URL"
    curl -# -L "$DOWNLOAD_URL" -o "$ARTIFACTS_TAR_GZ_FILE"

    # Create the build directory
    mkdir -p "$CIRCUIT_DIR"

    # Extract the tarball into the build directory with extraction progress
    echo "Extracting to $CIRCUIT_DIR..."
    tar --checkpoint=100 --checkpoint-action=dot -Pxzf "$ARTIFACTS_TAR_GZ_FILE" -C "$CIRCUIT_DIR"

    # Keep around in case we want to scp this to a docker host
    # NOTE: assumes /tmp is doing what it should and cleared on restart
    # rm -f "$ARTIFACTS_TAR_GZ_FILE"

    echo "Downloaded $DOWNLOAD_URL to $CIRCUIT_DIR"

# Extract required groth16 artifacts
extract-groth16-circuit:
    #!/usr/bin/env bash
    set -euo pipefail
    set -x  # start verbose output

    ARTIFACTS_TYPE="groth16"
    SP1_CIRCUIT_VERSION="v4.0.0-rc.3"
    CIRCUIT_DIR="$HOME/.sp1/circuits/$ARTIFACTS_TYPE/$SP1_CIRCUIT_VERSION"

    ARTIFACTS_TAR_GZ_FILE={{ artifacts-tar-gz-file }}

    # Create the build directory
    mkdir -p "$CIRCUIT_DIR"

    # Extract the tarball into the build directory with extraction progress
    echo "Extracting to $CIRCUIT_DIR..."
    tar --checkpoint=100 --checkpoint-action=dot -Pxzf "$ARTIFACTS_TAR_GZ_FILE" -C "$CIRCUIT_DIR"

    rm -f "$ARTIFACTS_TAR_GZ_FILE"

    echo "Downloaded $DOWNLOAD_URL to $CIRCUIT_DIR"
