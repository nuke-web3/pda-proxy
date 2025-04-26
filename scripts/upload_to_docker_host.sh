#!/usr/bin/env bash
set -euo pipefail

# Config
source ../.env

set -x  # start verbose output

# Batch upload to /app
# Don't archive, we wan't to use new HOST's user
rsync --recursive --links --perms --verbose --compress --progress \
  -e "ssh -i $REMOTE_IDENTITY" \
  ./service/static/ \
  ./scripts/ \
  .env \
  /tmp/pda-proxy-docker.tar.gz \
  "$REMOTE_HOST:/app/"
