#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frontend="$root/examples/axum-svelte/svelte-app"
scope="${1:-all}"

if [[ "$scope" != "all" && "$scope" != "--example-only" ]]; then
  echo "usage: $0 [--example-only]" >&2
  exit 2
fi

node -e '
const major = Number(process.versions.node.split(".")[0])
if (major < 22) {
  throw new Error(`Node 22 or newer is required; found ${process.version}`)
}
'
command -v pnpm >/dev/null || {
  echo "pnpm is required to run live SSR tests" >&2
  exit 1
}

rm -rf \
  "$root/examples/axum-svelte/public/build" \
  "$frontend/dist"

pnpm --dir "$frontend" install --frozen-lockfile --prefer-offline
pnpm --dir "$frontend" build

test -f "$root/examples/axum-svelte/public/build/.vite/manifest.json"
test -f "$frontend/dist/ssr/app.js"

if [[ "$scope" == "all" ]]; then
  cargo test \
    --locked \
    -p inertia-axum \
    --features ssr \
    --test ssr_node_lifecycle \
    -- \
    --ignored \
    --test-threads=1
fi

cargo test \
  --locked \
  -p axum-svelte \
  --test production_ssr \
  -- \
  --ignored \
  --test-threads=1
