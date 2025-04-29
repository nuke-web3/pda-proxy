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
  ../.env \
  /tmp/pda-proxy-docker.tar.gz \
  ../compose.yml \
  "$REMOTE_HOST:/app/"

rsync --recursive --links --perms --verbose --compress --progress \
  -e "ssh -i $REMOTE_IDENTITY" \
  ../scripts/ \
  "$REMOTE_HOST:/app/scripts"
  
# NOTE: Only for development, allow using dummy TLS
# rsync --recursive --links --perms --verbose --compress --progress \
#   -e "ssh -i $REMOTE_IDENTITY" \
#   ../service/static/ \ # Only for development
#   "$REMOTE_HOST:/app/static"
