#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { isDeepStrictEqual } from "node:util";

const contracts = [
  ["rust", "examples/demo/teleport.contract.json"],
  ["dotnet", "dotnet/examples/Teleport.Net.Demo/teleport.contract.json"],
  ["go", "go/examples/demo/teleport.contract.json"],
];

function readContract(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function sorted(value) {
  if (Array.isArray(value)) {
    return value.map(sorted);
  }

  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, inner]) => [key, sorted(inner)]),
    );
  }

  return value;
}

function stableContract(contract) {
  return sorted({
    version: contract.version,
    procedures: [...contract.procedures].sort((a, b) => a.name.localeCompare(b.name)),
    types: [...contract.types].sort((a, b) => a.name.localeCompare(b.name)),
  });
}

const [baselineName, baselinePath] = contracts[0];
const baseline = stableContract(readContract(baselinePath));

for (const [name, path] of contracts.slice(1)) {
  const candidate = stableContract(readContract(path));
  if (!isDeepStrictEqual(candidate, baseline)) {
    console.error(`${name} contract does not match ${baselineName} contract`);
    console.error(`baseline: ${baselinePath}`);
    console.error(`candidate: ${path}`);
    process.exitCode = 1;
  }
}

if (process.exitCode) {
  process.exit();
}

console.log("Rust, .NET, and Go demo contracts match.");
