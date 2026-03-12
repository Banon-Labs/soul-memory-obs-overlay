#!/usr/bin/env bash
set -euo pipefail

DEFAULT_TIMESTAMP_URL="http://timestamp.digicert.com"

usage() {
  cat <<'EOF'
Usage:
  scripts/setup-codesign-secrets.sh --pfx /path/to/cert.pfx [options]

Options:
  -p, --pfx PATH          Path to .pfx file (WSL Windows paths supported)
  -r, --repo OWNER/REPO   GitHub repository (default: current gh repo)
  -t, --timestamp-url URL Timestamp URL variable value (default: http://timestamp.digicert.com)
      --skip-timestamp    Do not set WINDOWS_CODESIGN_TIMESTAMP_URL
      --check             Show whether required names exist and exit
  -h, --help              Show this help

Secrets/variables configured:
  WINDOWS_CODESIGN_PFX_BASE64      (secret)
  WINDOWS_CODESIGN_PFX_PASSWORD    (secret)
  WINDOWS_CODESIGN_TIMESTAMP_URL   (variable)
EOF
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

resolve_repo() {
  gh repo view --json nameWithOwner --jq .nameWithOwner
}

supports_base64_wrap_flag() {
  base64 --help 2>&1 | grep -q -- "-w"
}

base64_single_line() {
  local path="$1"
  if supports_base64_wrap_flag; then
    base64 -w 0 "$path"
  else
    base64 "$path" | tr -d '\n'
  fi
}

resolve_pfx_path() {
  local input_path="$1"

  if [[ -f "$input_path" ]]; then
    printf '%s\n' "$input_path"
    return
  fi

  if command -v wslpath >/dev/null 2>&1 && [[ "$input_path" =~ ^[A-Za-z]:\\ ]]; then
    local unix_path
    unix_path="$(wslpath -u "$input_path")"
    if [[ -f "$unix_path" ]]; then
      printf '%s\n' "$unix_path"
      return
    fi
  fi

  printf '%s\n' "$input_path"
}

show_status() {
  local repo="$1"
  local secret_list variable_list

  secret_list="$(gh secret list -R "$repo")"
  variable_list="$(gh variable list -R "$repo")"

  if grep -q "^WINDOWS_CODESIGN_PFX_BASE64" <<<"$secret_list"; then
    echo "OK secret WINDOWS_CODESIGN_PFX_BASE64"
  else
    echo "MISSING secret WINDOWS_CODESIGN_PFX_BASE64"
  fi

  if grep -q "^WINDOWS_CODESIGN_PFX_PASSWORD" <<<"$secret_list"; then
    echo "OK secret WINDOWS_CODESIGN_PFX_PASSWORD"
  else
    echo "MISSING secret WINDOWS_CODESIGN_PFX_PASSWORD"
  fi

  if grep -q "^WINDOWS_CODESIGN_TIMESTAMP_URL" <<<"$variable_list"; then
    echo "OK variable WINDOWS_CODESIGN_TIMESTAMP_URL"
  else
    echo "MISSING variable WINDOWS_CODESIGN_TIMESTAMP_URL"
  fi
}

prompt_password() {
  local password=""

  while [[ -z "$password" ]]; do
    read -r -s -p "Enter PFX password: " password
    echo
    if [[ -z "$password" ]]; then
      echo "Password cannot be empty." >&2
    fi
  done

  printf '%s' "$password"
}

main() {
  local pfx_path=""
  local repo=""
  local timestamp_url="$DEFAULT_TIMESTAMP_URL"
  local skip_timestamp="false"
  local check_only="false"

  while (($# > 0)); do
    case "$1" in
      -p|--pfx)
        pfx_path="${2:-}"
        shift 2
        ;;
      -r|--repo)
        repo="${2:-}"
        shift 2
        ;;
      -t|--timestamp-url)
        timestamp_url="${2:-}"
        shift 2
        ;;
      --skip-timestamp)
        skip_timestamp="true"
        shift
        ;;
      --check)
        check_only="true"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        usage
        exit 1
        ;;
    esac
  done

  require_command gh
  require_command base64

  gh auth status >/dev/null

  if [[ -z "$repo" ]]; then
    repo="$(resolve_repo)"
  fi

  echo "Target repository: $repo"

  if [[ "$check_only" == "true" ]]; then
    show_status "$repo"
    exit 0
  fi

  if [[ -z "$pfx_path" ]]; then
    echo "Missing required --pfx path." >&2
    usage
    exit 1
  fi

  pfx_path="$(resolve_pfx_path "$pfx_path")"
  if [[ ! -f "$pfx_path" ]]; then
    echo "PFX file not found: $pfx_path" >&2
    exit 1
  fi

  echo "Setting WINDOWS_CODESIGN_PFX_BASE64 from $pfx_path"
  base64_single_line "$pfx_path" | gh secret set WINDOWS_CODESIGN_PFX_BASE64 -R "$repo"

  local password="${WINDOWS_CODESIGN_PFX_PASSWORD:-}"
  if [[ -z "$password" ]]; then
    password="$(prompt_password)"
  fi

  echo "Setting WINDOWS_CODESIGN_PFX_PASSWORD"
  printf '%s' "$password" | gh secret set WINDOWS_CODESIGN_PFX_PASSWORD -R "$repo"
  unset password

  if [[ "$skip_timestamp" == "false" ]]; then
    echo "Setting WINDOWS_CODESIGN_TIMESTAMP_URL to $timestamp_url"
    gh variable set WINDOWS_CODESIGN_TIMESTAMP_URL -R "$repo" --body "$timestamp_url"
  fi

  echo
  echo "Configuration status:"
  show_status "$repo"
}

main "$@"
