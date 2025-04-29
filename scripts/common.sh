#!/usr/bin/env bash

# Update existing VAR or append to .env file.
# Values escaped by sed.
#
# Example:
# update_env_var "TLS_CERTS_PATH" "/etc/letsencrypt/live/my.domain.com/fullchain.pem"
# update_env_var "TLS_CERTS_PATH" "/etc/letsencrypt/live/my.domain.com/fullchain.pem" "../some/path/.env"
update_env_var() {
  local key=$1
  local val=$2
  local env_file="${3:-${ENV_FILE:-../.env}}"

  # Escape value for sed
  val_escaped=$(printf '%s\n' "$val" | sed -e 's/[\/&]/\\&/g')

  if grep -q "^$key=" "$env_file"; then
    sed -i.bak "s/^$key=.*/$key=$val_escaped/" "$env_file"
  else
    echo "$key=$val" >> "$env_file"
  fi
}


# Fail if missing variables in current environment
#
# Example:
# require_dot_env_vars TLS_CERTS_PATH
# require_dot_env_vars REMOTE_IDENTITY TLS_DOMAIN TLS_EMAIL
require_dot_env_vars() {
  for var in "$@"; do
    if [[ -z "${!var:-}" ]]; then
      echo "❌ $var must be set in .env"
      exit 1
    fi
  done
}

