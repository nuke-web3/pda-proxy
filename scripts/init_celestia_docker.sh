#!/bin/bash

set -e

# === CONFIGURATION ===

source .env

# === FUNCTIONS ===

pull_image() {
  echo "🚀 Pulling Celestia Node image..."
  docker pull "$DOCKER_IMAGE"
}

create_data_dir() {
  echo "📁 Ensuring data directory exists at $DATA_DIR..."
  mkdir -p "$DATA_DIR"
}

initialize_node() {
  if [ ! -d "$DATA_DIR/$CHAIN_ID" ]; then
    echo "🔧 Initializing $NODE_TYPE node on $CHAIN_ID..."
    docker run --rm \
      -v "$DATA_DIR:/home/celestia" \
      "$DOCKER_IMAGE" \
      celestia "$NODE_TYPE" init --core.ip "" --p2p.network "$CHAIN_ID"
  else
    echo "✅ Node already initialized at $DATA_DIR/$CHAIN_ID"
  fi
}

set_write_jwt() {
  if [ ! -d "$DATA_DIR/$CHAIN_ID" ]; then
    echo "Exporting 'write' auth token "
    export CELESTIA_NODE_WRITE_TOKEN=$(docker run --rm \
      -v "$DATA_DIR:/home/celestia" \
      "$DOCKER_IMAGE" \
      celestia "$NODE_TYPE" light auth write --core.ip "" --p2p.network "$CHAIN_ID")
    echo "CELESTIA_NODE_WRITE_TOKEN=$CELESTIA_NODE_WRITE_TOKEN"  
    echo "^^^ save this in '.env' to persist it!"
  else
    echo "✅ Node already initialized at $DATA_DIR/$CHAIN_ID"
  fi
}

# Unsued, in MAIN but for reference:
run_node() {
  echo "🏃‍♂️ Starting $NODE_TYPE node for $CHAIN_ID..."
  docker run -d \
    --name "$NODE_NAME" \
    -v "$DATA_DIR:/home/celestia" \
    -p "$P2P_PORT:$P2P_PORT" \
    -p "$RPC_PORT:$RPC_PORT" \
    "$DOCKER_IMAGE" \
    celestia "$NODE_TYPE" start --core.ip ""
}

# === MAIN EXECUTION ===

pull_image
create_data_dir
initialize_node
set_write_jwt

echo "🎉 $NODE_TYPE node for $CHAIN_ID is ready with persistent storage at $DATA_DIR"
echo "Remember to save CELESTIA_NODE_WRITE_TOKEN in '.env' to persist it!"

