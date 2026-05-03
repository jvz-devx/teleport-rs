#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

out_dir="$repo_root/coverage/dotnet"
results_dir="$out_dir/results"
runsettings="$out_dir/coverage.runsettings"
summary_file="$out_dir/summary.txt"

mkdir -p "$results_dir"

cat >"$runsettings" <<'EOF'
<?xml version="1.0" encoding="utf-8"?>
<RunSettings>
  <DataCollectionRunSettings>
    <DataCollectors>
      <DataCollector friendlyName="XPlat Code Coverage">
        <Configuration>
          <Format>cobertura</Format>
          <Include>[Teleport.Net*]*</Include>
          <Exclude>[Teleport.Net.TestFixtures]*,[*.Tests]*</Exclude>
          <ExcludeByFile>**/tests/**/*.cs</ExcludeByFile>
        </Configuration>
      </DataCollector>
    </DataCollectors>
  </DataCollectionRunSettings>
</RunSettings>
EOF

projects=(
  "dotnet/tests/Teleport.Net.Tests/Teleport.Net.Tests.csproj"
  "dotnet/tests/Teleport.Net.AspNetCore.Tests/Teleport.Net.AspNetCore.Tests.csproj"
)

xml_files=()

for project in "${projects[@]}"; do
  project_name="$(basename "$project" .csproj)"
  project_results_dir="$results_dir/$project_name"
  rm -rf "$project_results_dir"
  mkdir -p "$project_results_dir"

  DOTNET_CLI_HOME=/tmp dotnet test "$project" \
    -c Release \
    -maxcpucount:1 \
    -nodeReuse:false \
    --collect:"XPlat Code Coverage" \
    --settings "$runsettings" \
    --results-directory "$project_results_dir"

  xml_file="$(find "$project_results_dir" -name 'coverage.cobertura.xml' -print -quit)"
  if [[ -z "$xml_file" ]]; then
    echo "missing coverage.cobertura.xml for $project" >&2
    exit 1
  fi

  normalized_xml="$out_dir/$project_name.cobertura.xml"
  cp "$xml_file" "$normalized_xml"
  xml_files+=("$normalized_xml")
done

awk -v repo_root="$repo_root/" '
  function normalize_path(source, file, full_path) {
    full_path = file

    if (full_path !~ /^\// && source != "") {
      full_path = source "/" full_path
    }

    gsub(/\/+/, "/", full_path)

    if (index(full_path, repo_root) == 1) {
      full_path = substr(full_path, length(repo_root) + 1)
    }

    return full_path
  }

  function is_production_file(path) {
    return path ~ /^dotnet\/src\/Teleport\.Net\// || path ~ /^dotnet\/src\/Teleport\.Net\.AspNetCore\//
  }

  function report_name(path, parts, n) {
    n = split(path, parts, "/")
    return parts[n]
  }

  FNR == 1 {
    current_source = ""
    current_file = ""
    current_report = report_name(FILENAME)
  }

  match($0, /<source>([^<]+)<\/source>/, source_match) {
    current_source = source_match[1]
    sub(/\/$/, "", current_source)
  }

  match($0, /<class name="[^"]*" filename="([^"]+)"/, class_match) {
    current_file = normalize_path(current_source, class_match[1])
  }

  match($0, /<line number="([0-9]+)" hits="([0-9]+)"/, line_match) {
    if (!is_production_file(current_file)) {
      next
    }

    report_key = current_report ":" current_file ":" line_match[1]
    combined_key = current_file ":" line_match[1]

    report_valid[report_key] = 1
    combined_valid[combined_key] = 1

    if (line_match[2] + 0 > 0) {
      report_covered[report_key] = 1
      combined_covered[combined_key] = 1
    }
  }

  END {
    report_count = 0

    for (key in report_valid) {
      split(key, parts, ":")
      report_totals[parts[1]]++
    }

    for (key in report_covered) {
      split(key, parts, ":")
      report_hits[parts[1]]++
    }

    for (report in report_totals) {
      reports[++report_count] = report
    }

    n = asort(reports)
    for (i = 1; i <= n; i++) {
      report = reports[i]
      total = report_totals[report]
      hits = report_hits[report] + 0
      rate = total == 0 ? 0 : (hits / total) * 100
      printf "%s production line coverage: %.2f%% (%d/%d)\n", report, rate, hits, total
    }

    for (key in combined_valid) {
      total_lines++
    }

    for (key in combined_covered) {
      covered_lines++
    }

    rate = total_lines == 0 ? 0 : (covered_lines / total_lines) * 100
    printf ".NET combined production line coverage: %.2f%% (%d/%d)\n", rate, covered_lines, total_lines
  }
' "${xml_files[@]}" | tee "$summary_file"

{
  printf "reports:\n"
  for xml_file in "${xml_files[@]}"; do
    printf "%s\n" "$xml_file"
  done
} >>"$summary_file"
