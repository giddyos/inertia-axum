#!/usr/bin/env bash
set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$#" -eq 0 ]]; then
  for example in axum-embedded actix-embedded rocket-embedded; do
    "$0" "$example"
  done
  exit 0
fi

if [[ "$#" -ne 1 ]]; then
  echo "usage: $0 [axum-embedded|actix-embedded|rocket-embedded]" >&2
  exit 2
fi

example="$1"
case "$example" in
  axum-embedded | actix-embedded | rocket-embedded) ;;
  *)
    echo "unsupported embedded example: $example" >&2
    exit 2
    ;;
esac

frontend="$repository/examples/$example/frontend"
dist="$frontend/dist"
target="${CARGO_TARGET_DIR:-$repository/target}"
isolated="$(mktemp -d)"
hidden_dist=""
server_pid=""

cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  if [[ -n "$hidden_dist" && -d "$hidden_dist" && ! -e "$dist" ]]; then
    mv "$hidden_dist" "$dist"
  fi
  rm -rf "$isolated"
}
trap cleanup EXIT

pnpm --dir "$frontend" install --frozen-lockfile --prefer-offline
pnpm --dir "$frontend" build
cargo build --locked --release -p "$example"

cp "$target/release/$example" "$isolated/$example"
hidden_dist="$frontend/dist.self-contained-test.$$"
mv "$dist" "$hidden_dist"

(
  cd "$isolated"
  exec env ADDR=127.0.0.1:0 "./$example" >server.log 2>&1
) &
server_pid=$!

address=""
for _ in {1..100}; do
  if [[ -f "$isolated/server.log" ]]; then
    address="$(sed -n 's/^LISTENING //p' "$isolated/server.log" | head -n 1)"
  fi
  [[ -n "$address" ]] && break
  if ! kill -0 "$server_pid" 2>/dev/null; then
    cat "$isolated/server.log" >&2
    exit 1
  fi
  sleep 0.1
done

if [[ -z "$address" ]]; then
  echo "$example did not report a listening address" >&2
  cat "$isolated/server.log" >&2
  exit 1
fi

curl --fail --silent --show-error "http://$address/" >"$isolated/index.html"
grep -q 'rel="stylesheet"' "$isolated/index.html"
grep -q '<script type="module"' "$isolated/index.html"

assets=()
while IFS= read -r asset; do
  assets+=("$asset")
done < <(
  grep -Eo '(href|src)="[^"]+"' "$isolated/index.html" |
    sed -E 's/^[^"]*"([^"]+)"$/\1/'
)
if [[ "${#assets[@]}" -lt 2 ]]; then
  echo "expected embedded script and stylesheet URLs" >&2
  exit 1
fi

for asset in "${assets[@]}"; do
  output="$isolated/$(basename "$asset")"
  headers="$output.headers"
  curl --fail --silent --show-error --dump-header "$headers" \
    "http://$address$asset" >"$output"
  [[ -s "$output" ]]
  if grep -qi '^content-encoding:' "$headers"; then
    echo "$example exposed executable storage as HTTP content encoding" >&2
    exit 1
  fi
  cmp "$output" "$hidden_dist/${asset#/build/}"
done

[[ ! -e "$dist" ]]
echo "$example served its page and ${#assets[@]} assets with the frontend build hidden"
