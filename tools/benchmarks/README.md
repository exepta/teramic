# Benchmark Artifacts

Benchmark scripts, measurement reports, summaries and reproducible settings are
part of the repository. Runtime assets under `assets/` and `src-ui/assets/` are
also versioned; the benchmark exclusions do not apply to them.

Generated maps, player profiles, saved worlds, local asset libraries, compressed
logs and Python caches stay local. They may contain player identifiers or world
state and are not required to build or run TERAMIC.

Historical PNG captures in `results/` are intentionally excluded from Git. The
local files are not deleted, and historical report paths still refer to those
local artifacts. The initial publication would otherwise include more than
5 GiB of screenshots. Current regression captures are retained for:

- `results/inventory-fps-2026-10-22/native/`
- `results/status-effects-2026-10-22/native/`

Add similarly narrow exceptions for future reviewed visual evidence rather
than committing complete generated benchmark directories.

Reviewed plain-text logs in those same two reports' `logs/` directories are
also retained. Other logs keep the existing exclusion; do not include private
server logs or copied profiles as benchmark evidence.
