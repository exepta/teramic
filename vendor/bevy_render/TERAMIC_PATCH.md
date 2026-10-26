# Bevy Render 0.19.1 Patch

This is the existing 0.19.1 renderer source, with its original licenses.
No dependency version is upgraded. Teramic uses a local Cargo patch so the
fix is reproducible without modifying a developer's Cargo registry cache.

The two CPU-owned bin-list buffers in `write_binned_instance_buffers` use an
opt-in exact-byte upload cache. Unchanged bytes do not need another staging
allocation and queue write. Culling, transforms, view uniforms, indirect draw
parameters, unpacking metadata and shadow rendering still update every frame.

`RawBufferVec::write_buffer` retains its original unconditional behavior.
The new `write_buffer_if_changed` is only valid for exclusively CPU-written
buffers. Buffer reallocation and regular/ranged writes invalidate its cache.
An individual cache is limited to 4 MiB and follows the owning buffer's lifetime.
The existing bin/view cleanup therefore also releases these CPU copies.

`BinnedInstanceBufferUploadSettings` can disable reuse for native comparisons.
Teramic exposes that reference mode only in smoke tests through
`TERRA_BIN_UPLOAD_CACHE=0`.

The native same-binary night comparison reduces the measured shadow bin-list
writer from 1.731 to 0.199 ms/frame, with unchanged section/prop/light counts.
The tiny physics-adjusted camera-height difference is recorded, not hidden.
Three CPU cache regression tests pass using this crate's own locked test
manifest; the full Teramic workspace and native moving/render tests also pass.
See `tools/benchmarks/results/streaming-regression-2026-10-25/README.md` in the
repository root for commands, full comparisons and remaining frame spikes.
