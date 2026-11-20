# Benchmark Artifacts

`bash tools/benchmarks/run-voxel-creatures.sh` inspects the already-built
`target/debug/terra-client` in six isolated creature galleries. It records 43
screenshots covering standard animals, Spirit creatures, Blood Forest creatures
and skeletons, aquatic creatures, horses and representative villagers. The run
checks exact filenames, completion markers, image dimensions, shader/model
errors and an unchanged client checksum. It creates disposable configurations
and saves under `target/voxel-creatures/`; it does not build the client or open
player saves. These are model and animation fixtures, not authoritative AI or
swimming tests. See [voxel creature models](../../docs/voxel-creature-models.md).

`run-frame-spikes.py` runs a finite integrated comparison of clear weather,
rain, a real village, flight into new chunks, the actual inventory and rapid
acknowledged block edits. It freezes configurations, isolates runtime state
and rejects incomplete scenarios. Run `python3 tools/benchmarks/run-frame-spikes.py --help`
for options; the [O07 report](results/o07-frame-spikes-2026-11-05/README.md)
contains measured results, reproduction settings and interpretation limits.

The [O08 inventory report](results/o08-inventory-opening-2026-11-05/README.md)
documents first-open and repeated player/chest transitions. The inventory
benchmark accepts optional `TERRA_BENCH_INVENTORY_ROUNDS=2` and
`TERRA_BENCH_INVENTORY_SURFACE=player|chest`; the report includes isolated
configuration and readiness requirements.

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
- `results/equipment-2026-10-22/native/`
- `results/capes-crusher-2026-10-22/native/`
- `results/manual-crusher-2026-10-22/native/`

Add similarly narrow exceptions for future reviewed visual evidence rather
than committing complete generated benchmark directories.

Reviewed plain-text logs in those same reports' `logs/` directories are
also retained. Other logs keep the existing exclusion; do not include private
server logs or copied profiles as benchmark evidence.
