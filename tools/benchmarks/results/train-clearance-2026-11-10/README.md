# Train attachment, clearance and run-over regression checks

Date: 2026-11-10. Checks use isolated in-memory worlds; no graphical client was
launched and existing player worlds were not modified.

## Verified behavior

- The authoritative Survival placement path accepts the rail builder on all four
  horizontal frame faces, orients it outward and consumes exactly one item.
- Re-recording the same train block retains its facing. Section export/import
  preserves that state. Unrelated pairing metadata and invalid facing are rejected.
- The attachment's overhanging geometry participates in targeting, collision and
  neighboring placement checks, including across a section boundary and after reload.
- Forward clearance removes exactly five by nine blocks on cardinal routes,
  preserving the floor, roof and side boundaries. Existing rails remain intact.
- Ordinary blocks retain wood, tint, slab and tree metadata in drops and surviving
  geometry. Protected obstacles, stale plans and exhausted drop IDs do not publish
  partial excavation.
- Clearing and extending a free rail end form one transaction with cargo payment.
  Missing materials leave terrain, cargo, rail revisions and drops unchanged.
- A pitched front sweep cannot delete the extension's ballast or duplicate its
  floor drops. Survival drops are compacted into normal-sized stacks; Creative
  creates neither resource costs nor excavation drops.
- Committed forward and reverse train sweeps hit targets between sampled poses.
  Standing/rejected movement causes no impact. Player immunity, death notification,
  cooldown, animal/mob drops and mounted-horse death synchronization are covered.

## Results

- Workspace: **4564 passed, 0 failed, 10 existing ignored tests**.
- One test was explicitly filtered because the original
  `FIVERR/VoxelGrassBlock glb file.glb` is already absent from this checkout:
  `original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`.
- Strict workspace/all-target Clippy: passed with warnings denied.
- Client/server binaries: built successfully; formatting and whitespace checks passed.
- Native train asset lint: passed for all 86 catalog entries, model data,
  collision shapes, projected icon coverage and embedded recipes.
- Additional focused world rerun: all 10 builder/attachment-state cases passed.

Commands:

```sh
cargo test --locked --workspace --lib --bins --no-fail-fast --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --bins --jobs 2
cargo fmt --all -- --check
python tools/assets/lint_train_parts.py --log target/phase229-assets.log
```

Local logs: `target/phase229-tests-final.log`, `target/phase229-clippy-final.log`,
`target/phase229-build.log`, `target/phase229-assets.log`. Client and server use protocol **131** and must be
restarted together.
