#!/usr/bin/env bash
set -euo pipefail

ARTIFACTS_TYPE="groth16"
SP1_CIRCUIT_VERSION="v4.0.0-rc.3"
CIRCUIT_DIR="$HOME/.sp1/circuits/$ARTIFACTS_TYPE/$SP1_CIRCUIT_VERSION"
ARTIFACTS_TAR_GZ_FILE="/tmp/groth-circuits.tar.gz"

# Create the build directory
mkdir -p "$CIRCUIT_DIR"

# Extract the tarball into the build directory with extraction progress
echo "Extracting to $CIRCUIT_DIR..."
tar --checkpoint=100 --checkpoint-action=dot -Pxzf "$ARTIFACTS_TAR_GZ_FILE" -C "$CIRCUIT_DIR"

rm -f "$ARTIFACTS_TAR_GZ_FILE"

echo "Downloaded $DOWNLOAD_URL to $CIRCUIT_DIR"
