#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root/go"

out_dir="$repo_root/coverage/go"
profile="$out_dir/coverage.out"
summary="$out_dir/summary.txt"

mkdir -p "$out_dir"

GOCACHE="${GOCACHE:-/tmp/go-build}" \
CGO_ENABLED=0 \
go test \
  ./teleport/... \
  ./teleporthttp/... \
  -covermode=atomic \
  -coverpkg=./teleport/...,./teleporthttp/... \
  -coverprofile="$profile"

go tool cover -func="$profile" | tee "$summary"
