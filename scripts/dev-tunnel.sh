#!/usr/bin/env bash
set -euo pipefail

if ! command -v cloudflared >/dev/null 2>&1; then
  echo "error: cloudflared is not installed or not on PATH." >&2
  echo "Install it from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/" >&2
  exit 1
fi

# Read BOTIFIL_WEBAPP_BIND from .env when unset in the shell without sourcing secrets.
if [[ -z "${BOTIFIL_WEBAPP_BIND:-}" && -z "${WEBAPP_BIND:-}" && -f .env ]]; then
  dotenv_bind="$(
    grep -E '^[[:space:]]*BOTIFIL_WEBAPP_BIND=' .env \
      | tail -n 1 \
      | cut -d= -f2- \
      | sed -e 's/[[:space:]]*#.*$//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e 's/^["'\'']//' -e 's/["'\'']$//' \
      || true
  )"
  if [[ -n "$dotenv_bind" ]]; then
    BOTIFIL_WEBAPP_BIND="$dotenv_bind"
  fi
fi

bind="${BOTIFIL_WEBAPP_BIND:-${WEBAPP_BIND:-127.0.0.1:8080}}"
log_file="$(mktemp -t botifil-cloudflared.XXXXXX.log)"
cf_pid=""

cleanup() {
  trap - EXIT INT TERM
  if [[ -n "$cf_pid" ]] && kill -0 "$cf_pid" 2>/dev/null; then
    kill "$cf_pid" 2>/dev/null || true
    wait "$cf_pid" 2>/dev/null || true
  fi
  rm -f "$log_file"
}

trap cleanup EXIT
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM

cloudflared tunnel --url "http://$bind" >"$log_file" 2>&1 &
cf_pid=$!

url=""
for _ in $(seq 1 60); do
  url="$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$log_file" | head -n 1 || true)"
  if [[ -n "$url" ]]; then
    break
  fi
  if ! kill -0 "$cf_pid" 2>/dev/null; then
    echo "error: cloudflared exited before publishing a tunnel URL:" >&2
    cat "$log_file" >&2
    exit 1
  fi
  sleep 0.5
done

if [[ -z "$url" ]]; then
  echo "error: timed out waiting for cloudflared tunnel URL:" >&2
  cat "$log_file" >&2
  exit 1
fi

echo "Cloudflare Quick Tunnel active: $url -> http://$bind"
BOTIFIL_WEBAPP_URL="$url" cargo run --locked
