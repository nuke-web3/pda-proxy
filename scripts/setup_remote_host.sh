#!/usr/bin/env bash
set -euo pipefail

# Setup a remote machine to be ready to run

source ./common.sh
source ../.env

if [ ! -f ../.env ]; then
  echo "Creating .env from example.env"
  cp ../example.env ../.env
fi

require_dot_env_vars ENCRYPTION_KEY

./upload_to_docker_host.sh
./config_lets_encrypt.sh
./init_celestia_docker.sh

# Docker compose startup, run detached so you can close the local term,
# then immediate starts following logs.
ssh -i "$REMOTE_IDENTITY" "$REMOTE_HOST" \
  'cd /app && docker load < /tmp/pda-proxy-docker.tar.gz && docker compose up -d && docker compose logs -f'

