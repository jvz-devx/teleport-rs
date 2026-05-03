#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "cargo-llvm-cov is required. Run this from nix develop with cargo-llvm-cov installed." >&2
  exit 1
fi

if [[ -z "${LLVM_COV:-}" ]]; then
  export LLVM_COV="$(command -v llvm-cov)"
fi

if [[ -z "${LLVM_PROFDATA:-}" ]]; then
  export LLVM_PROFDATA="$(command -v llvm-profdata)"
fi

out_dir="$repo_root/coverage/rust"
main_dir="$out_dir/main"
cli_dir="$out_dir/cli"
ignore_regex='(/tests?/|/examples?/|/target/|/\.cargo/registry/|/rustc/)'

mkdir -p "$main_dir" "$cli_dir"

coverage_attr() {
  local xml_file="$1"
  local attr="$2"

  sed -n "s/.*$attr=\"\\([^\"]*\\)\".*/\\1/p" "$xml_file" | head -n 1
}

write_summary() {
  local label="$1"
  local xml_file="$2"
  local summary_file="$3"
  local lines_covered lines_valid branches_covered branches_valid line_rate branch_rate

  lines_covered="$(coverage_attr "$xml_file" "lines-covered")"
  lines_valid="$(coverage_attr "$xml_file" "lines-valid")"
  branches_covered="$(coverage_attr "$xml_file" "branches-covered")"
  branches_valid="$(coverage_attr "$xml_file" "branches-valid")"
  line_rate="$(coverage_attr "$xml_file" "line-rate")"
  branch_rate="$(coverage_attr "$xml_file" "branch-rate")"

  {
    printf "%s\n" "$label"
    printf "line coverage: %.2f%% (%s/%s)\n" "$(awk "BEGIN { print $line_rate * 100 }")" "$lines_covered" "$lines_valid"
    printf "branch coverage: %.2f%% (%s/%s)\n" "$(awk "BEGIN { print $branch_rate * 100 }")" "$branches_covered" "$branches_valid"
    printf "cobertura xml: %s\n" "$xml_file"
  } | tee "$summary_file"
}

run_report() {
  local _label="$1"
  local xml_file="$2"
  shift 2

  cargo llvm-cov clean --workspace
  cargo llvm-cov "$@" --ignore-filename-regex "$ignore_regex" --cobertura --output-path "$xml_file"
}

main_xml="$main_dir/cobertura.xml"
cli_xml="$cli_dir/cobertura.xml"

run_report \
  "Rust workspace libraries" \
  "$main_xml" \
  --workspace \
  --all-features \
  --exclude teleport-demo \
  --exclude teleport-starter \
  --exclude teleport-cli

write_summary "Rust workspace libraries" "$main_xml" "$main_dir/summary.txt"

run_report \
  "teleport-cli" \
  "$cli_xml" \
  -p teleport-cli \
  --all-features

write_summary "teleport-cli" "$cli_xml" "$cli_dir/summary.txt"
