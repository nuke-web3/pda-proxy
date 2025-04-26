#!/usr/bin/env bash

set -euo pipefail

# TODO: don't have to manually upkeep these here if possible!
# See <https://github.com/succinctlabs/sp1/blob/dev/crates/sdk/src/install.rs>
# might make a mini utility bin for service to manually execute this instead
#
# Required environment variables:
ARTIFACTS_TYPE="groth16"
SP1_CIRCUIT_VERSION="v4.0.0-rc.3"
CIRCUIT_DIR="$HOME/.sp1/circuits/$ARTIFACTS_TYPE/$SP1_CIRCUIT_VERSION"

CIRCUIT_ARTIFACTS_URL_BASE="https://sp1-circuits.s3-us-east-2.amazonaws.com"
DOWNLOAD_URL="${CIRCUIT_ARTIFACTS_URL_BASE}/${SP1_CIRCUIT_VERSION}-${ARTIFACTS_TYPE}.tar.gz"

ARTIFACTS_TAR_GZ_FILE="/tmp/groth-circuits.tar.gz"

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
