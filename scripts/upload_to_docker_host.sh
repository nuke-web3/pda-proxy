#!/usr/bin/env bash
set -euo pipefail

source ./common.sh
source ../.env

require_dot_env_vars REMOTE_HOST REMOTE_IDENTITY

set -x  # start verbose output

# Batch upload to /app
# Don't archive, we want HOST user as owner (likely root)
rsync --recursive --links --perms --verbose --compress --progress \
  -e "ssh -i $REMOTE_IDENTITY" \
  ./service/static/ \
  ./scripts/ \
  ../.env \
  /tmp/pda-proxy-docker.tar.gz \
  ../compose.yml \
  "$REMOTE_HOST:/app/"
