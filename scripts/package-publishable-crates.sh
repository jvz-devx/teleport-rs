#!/usr/bin/env bash

set -euo pipefail

# `cargo package` verification rebuilds each crate from its tarball in isolation.
# Before the internal crates are published, that verification can fail because
# path dependencies are rewritten to registry dependencies that do not exist yet.
#
# This script keeps the packaging checks that matter pre-release:
# 1. `cargo package --list` for each publishable crate validates package
#    metadata and the file set that would go into the tarball.
# 2. A temporary workspace is assembled from exactly those packaged files.
# 3. That staged workspace is compiled, so only files that would actually be
#    published are allowed to satisfy the build.

readonly crates=(
  teleport-contract
  teleport-core
  teleport-build
  teleport-macros
  teleport
  teleport-cli
)

crate_dir() {
  case "$1" in
    teleport-contract) printf '%s\n' "crates/teleport-contract" ;;
    teleport-core) printf '%s\n' "crates/teleport-core" ;;
    teleport-build) printf '%s\n' "crates/teleport-build" ;;
    teleport-macros) printf '%s\n' "crates/teleport-macros" ;;
    teleport) printf '%s\n' "crates/teleport" ;;
    teleport-cli) printf '%s\n' "crates/teleport-cli" ;;
    *)
      printf 'unknown crate: %s\n' "$1" >&2
      return 1
      ;;
  esac
}

temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT
stage_dir="$temp_dir/stage"
mkdir -p "$stage_dir"

workspace_package_block() {
  awk '
    /^\[workspace\.package\]/ {
      print
      in_block = 1
      next
    }
    /^\[/ && in_block {
      exit
    }
    in_block {
      print
    }
  ' Cargo.toml
}

workspace_dependencies_block() {
  awk '
    /^\[workspace\.dependencies\]/ {
      print
      in_block = 1
      next
    }
    /^\[/ && in_block {
      exit
    }
    in_block {
      print
    }
  ' Cargo.toml
}

copy_packaged_file() {
  local crate="$1"
  local package_path="$2"
  local source_dir
  local target_path

  source_dir="$(crate_dir "$crate")"
  target_path="$stage_dir/$source_dir/$package_path"
  mkdir -p "$(dirname "$target_path")"

  case "$package_path" in
    .cargo_vcs_info.json)
      printf '{\"git\":{\"sha1\":null},\"path_in_vcs\":\"%s\"}\n' "$source_dir" > "$target_path"
      ;;
    Cargo.toml|Cargo.toml.orig)
      cp "$source_dir/Cargo.toml" "$target_path"
      ;;
    Cargo.lock)
      cp Cargo.lock "$target_path"
      ;;
    README.md)
      cp README.md "$target_path"
      ;;
    *)
      cp "$source_dir/$package_path" "$target_path"
      ;;
  esac
}

for crate in "${crates[@]}"; do
  while IFS= read -r package_path; do
    copy_packaged_file "$crate" "$package_path"
  done < <(cargo package --list "$@" -p "$crate")
done

workspace_toml="$stage_dir/Cargo.toml"

{
  printf '[workspace]\n'
  printf 'members = [\n'
  for crate in "${crates[@]}"; do
    printf '  "%s",\n' "$(crate_dir "$crate")"
  done
  printf ']\n'
  printf 'resolver = "3"\n\n'
  workspace_package_block
  printf '\n'
  workspace_dependencies_block
} > "$workspace_toml"

check_args=(
  check
  --manifest-path "$workspace_toml"
  --all-targets
  --all-features
)

for crate in "${crates[@]}"; do
  check_args+=(-p "$crate")
done

cargo "${check_args[@]}"
