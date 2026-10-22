import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const directory = fileURLToPath(new URL("logs/", import.meta.url));
const number = (line, field) => {
  const value = line.match(new RegExp(`\\b${field}=([\\d.e+-]+)`))?.[1];
  return value === undefined ? null : Number(value);
};
for (const file of readdirSync(directory).filter((name) => name.endsWith(".log")).sort()) {
  const lines = readFileSync(`${directory}/${file}`, "utf8").split("\n");
  const result = lines.find((line) => line.includes("Native smoke benchmark ("));
  if (!result) continue;
  const scopes = {};
  const gpu = {};
  for (const line of lines) {
    const scope = line.match(/CPU scope .* scope="([^"]+)"/)?.[1];
    if (scope) {
      scopes[scope] = {
        calls: number(line, "calls"),
        mean_ms: number(line, "total_ms") / number(line, "calls"),
        max_ms: number(line, "max_ms"),
      };
    }
    const pass = line.match(/diagnostic=(\S+\/elapsed_gpu) /)?.[1];
    if (pass) gpu[pass] = number(line, "mean");
  }
  const fields = [
    "average_fps", "p95_frame_ms", "p99_frame_ms", "initial_load_seconds",
    "resident_memory_kib", "loaded_sections", "rendered_sections",
    "mesh_assets", "visible_mesh_entities", "represented_props",
    "prop_batches", "water_tiles",
  ];
  console.log(JSON.stringify({
    file,
    resolution: result.match(/physical_resolution=(Some\(\(\d+, \d+\)\)|None)/)?.[1],
    ...Object.fromEntries(fields.map((field) => [field, number(result, field)])),
    errors: lines.filter((line) => /\bERROR\b|panicked at/.test(line)).length,
    scopes,
    gpu,
  }));
}
