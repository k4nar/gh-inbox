#!/usr/bin/env bash
# Integration test: verify the release binary serves embedded frontend HTML.
# Run after `cargo build --release`.
set -euo pipefail

BINARY="./target/release/gh-inbox"

if [ ! -f "$BINARY" ]; then
  echo "FAIL: release binary not found. Run 'cargo build --release' first."
  exit 1
fi

PORT=0
# Let the OS pick a port by starting the server and reading its output
OUTPUT=$(mktemp)
GH_INBOX_PORT=0 "$BINARY" > "$OUTPUT" 2>&1 &
PID=$!

# Wait for the server to print its listening address
for i in $(seq 1 30); do
  if grep -q "Listening on" "$OUTPUT" 2>/dev/null; then
    break
  fi
  sleep 0.1
done

PORT=$(grep "Listening on" "$OUTPUT" | grep -oE '[0-9]+$')

cleanup() {
  kill "$PID" 2>/dev/null || true
  wait "$PID" 2>/dev/null || true
  rm -f "$OUTPUT"
}
trap cleanup EXIT

if [ -z "$PORT" ]; then
  echo "FAIL: could not determine server port"
  cat "$OUTPUT"
  exit 1
fi

BODY=$(curl -sf "http://127.0.0.1:$PORT/")

if echo "$BODY" | grep -q '<!doctype html>'; then
  echo "PASS: GET / returns HTML from embedded frontend"
else
  echo "FAIL: GET / did not return expected HTML"
  echo "$BODY"
  exit 1
fi

# Also test SPA fallback: an unknown path should return index.html, not 404
FALLBACK=$(curl -sf "http://127.0.0.1:$PORT/some/unknown/route")

if echo "$FALLBACK" | grep -q '<!doctype html>'; then
  echo "PASS: SPA fallback returns index.html for unknown routes"
else
  echo "FAIL: SPA fallback did not return index.html"
  echo "$FALLBACK"
  exit 1
fi

# Unknown /api/* routes must return 404, not SPA fallback
API_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PORT/api/nonexistent")

if [ "$API_STATUS" = "404" ]; then
  echo "PASS: unknown /api/ route returns 404"
else
  echo "FAIL: unknown /api/ route returned $API_STATUS (expected 404)"
  exit 1
fi

echo "All release embed tests passed."
