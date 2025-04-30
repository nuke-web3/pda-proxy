#!/usr/bin/env bash

# Update an existing .env file variable, or append to .env file.
#
# Example:
# update_env_var "TLS_CERTS_PATH" "/etc/letsencrypt/live/my.domain.com/fullchain.pem"
# update_env_var "TLS_CERTS_PATH" "/etc/letsencrypt/live/my.domain.com/fullchain.pem" "../some/path/.env"
update_env_var() {
  local key=$1
  local val=$2
  local env_file="${3:-${ENV_FILE:-$(pwd)/../.env}}"

  if [[ ! -f "$env_file" ]]; then
      echo "❌ Error: Failed to find '$env_file'" >&2
  fi

  # Escape value for sed
  local val_escaped
  val_escaped=$(printf '%s\n' "$val" | sed -e 's/[\/&]/\\&/g')

  if grep -q "^$key=" "$env_file"; then
    sed -i.bak "s/^$key=.*/$key=$val_escaped/" "$env_file"
  else
    echo "$key=$val" >> "$env_file"
  fi
}

# Update a TOML file's `[section] key = value` to be some new value
#
# Example:
# update_toml_from_var "DASer.SampleFrom" "${CELESTIA_TRUSTED_HEIGHT}" ~/.celestia-light-mocha-4/config.toml
# update_toml_from_var "Header.TrustedHash" "${CELESTIA_TRUSTED_HASH}" ~/.celestia-light-mocha-4/config.toml
update_toml_from_var() {
  local path="$1"
  local val="$2"
  local toml_file="${3}"

  # Extract section and key
  local section="${path%%.*}"
  local key="${path#*.}"

  # Escape value and special chars
  local val_escaped
  val_escaped=$(printf '%s' "$val" | sed -e 's/[\/&]/\\&/g')

  # Determine if the value is numeric or boolean (unquoted), or string (quoted)
  if [[ "$val" =~ ^[0-9]+$ || "$val" =~ ^(true|false)$ ]]; then
    local replacement="\1$val_escaped"
  else
    local replacement="\1\"$val_escaped\""
  fi

  # Use awk to limit sed range to section, then replace the key
  sed -i.bak -E "/^\[$section\]/,/^\[.*\]/ s|^(\s*${key}\s*=\s*).*$|$replacement|" "$toml_file"
}

# Read a TOML file section.key=value and export as variable
# Values escaped by sed.
#
# Example:
# read_toml_to_export_var "DASer.SampleFrom" "CELESTIA_TRUSTED_HEIGHT"
# read_toml_to_export_var "Header.TrustedHash" "CELESTIA_TRUSTED_HASH"
# 
# echo "$CELESTIA_TRUSTED_HEIGHT"
read_toml_to_export_var() {
  local path="$1"
  local varname="$2"
  local toml_file="$3"

  local section="${path%%.*}"
  local key="${path#*.}"

  local value
  value=$(awk -v section="$section" -v key="$key" '
    BEGIN { in_section=0; found=0 }
    /^\[[^]]+\]/ {
      s=$0
      gsub(/[ \t\[\]]/, "", s)
      in_section = (s == section)
    }
    in_section {
      match($0, "^[ \t]*" key "[ \t]*=[ \t]*")
      if (RSTART > 0) {
        split($0, parts, "=")
        val = parts[2]
        gsub(/^[ \t]+|[ \t]+$/, "", val)
        gsub(/^"|"$/, "", val)
        print val
        exit
      }
    }
  ' "$toml_file")

  if grep -q "^[[:space:]]*$key[[:space:]]*=" "$toml_file"; then
    export "$varname"="$value"
  else
    echo "Warning: $key not found in [$section] in $toml_file" >&2
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

