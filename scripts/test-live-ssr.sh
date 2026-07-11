#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
examples=(axum-svelte axum-react axum-vue)

if (( $# == 0 )); then
  selected=("${examples[@]}")
  run_lifecycle=true
elif (( $# == 2 )) && [[ "$1" == "--example-only" ]]; then
  case "$2" in
    axum-svelte|axum-react|axum-vue) selected=("$2") ;;
    *)
      echo "unknown example: $2" >&2
      echo "usage: $0 [--example-only axum-svelte|axum-react|axum-vue]" >&2
      exit 2
      ;;
  esac
  run_lifecycle=false
else
  echo "usage: $0 [--example-only axum-svelte|axum-react|axum-vue]" >&2
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

if [[ "$run_lifecycle" == "true" ]]; then
  cargo test \
    --locked \
    -p inertia-axum \
    --features ssr \
    --test ssr_node_lifecycle \
    -- \
    --ignored \
    --test-threads=1
fi

for example in "${selected[@]}"; do
  case "$example" in
    axum-svelte) app=svelte-app ;;
    axum-react) app=react-app ;;
    axum-vue) app=vue-app ;;
  esac
  frontend="$root/examples/$example/$app"

  rm -rf \
    "$root/examples/$example/public/build" \
    "$frontend/dist"

  pnpm --dir "$frontend" install --frozen-lockfile --prefer-offline
  pnpm --dir "$frontend" build

  test -f "$root/examples/$example/public/build/.vite/manifest.json"
  test -f "$frontend/dist/ssr/app.js"

  cargo test \
    --locked \
    -p "$example" \
    --test production_ssr \
    -- \
    --ignored \
    --test-threads=1
done
