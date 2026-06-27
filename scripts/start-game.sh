#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RELEASE=0
if [[ "${1:-}" == "--release" ]]; then
  RELEASE=1
fi

run_cargo() {
  if [[ "$RELEASE" -eq 1 ]]; then
    cargo run --release -p "$1"
  else
    cargo run -p "$1"
  fi
}

SERVER_PID=""
cleanup() {
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "Stopping server (pid $SERVER_PID)..."
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

echo "Starting OpenMMO server..."
run_cargo openmmo-server &
SERVER_PID=$!

echo "Waiting for server at http://127.0.0.1:8080/health ..."
ready=0
for _ in $(seq 1 60); do
  if curl -sf http://127.0.0.1:8080/health >/dev/null 2>&1; then
    ready=1
    break
  fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "Server exited unexpectedly." >&2
    wait "$SERVER_PID" || true
    exit 1
  fi
  sleep 0.5
done

if [[ "$ready" -ne 1 ]]; then
  echo "Timed out waiting for server health check." >&2
  exit 1
fi

echo "Server ready. Starting client..."
run_cargo openmmo-client
