//! Amortize engine-owned preprocessing output buffers without changing their lengths.
use bevy::{
    pbr::{MeshInputUniform, MeshPipeline, MeshUniform},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        batching::gpu_preprocessing::{BatchedInstanceBuffers, write_batched_instance_buffers},
        renderer::RenderDevice,
    },
};

const MIN_GRAIN_BYTES: usize = 64 * 1024;
const MAX_PADDING_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn install(app: &mut App) {
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            Render,
            reserve_output_buffers
                .in_set(RenderSystems::PrepareResourcesFlush)
                .after(RenderSystems::PrepareResourcesCollectPhaseBuffers)
                .before(write_batched_instance_buffers::<MeshPipeline>)
                .run_if(resource_exists::<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>),
        );
    }
}

fn reserve_output_buffers(
    device: Res<RenderDevice>,
    mut buffers: ResMut<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("render.preprocess_buffers.reserve");
    static LOG_GROWTH: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var("TERRA_CPU_SCOPES").is_ok_and(|value| value == "1")
    });
    let limits = device.limits();
    let max_bytes = limits
        .max_buffer_size
        .min(limits.max_storage_buffer_binding_size);
    let stride = size_of::<MeshUniform>();
    for (phase, phase_buffers) in &mut buffers.phase_instance_buffers {
        let output = &mut phase_buffers.data_buffer;
        let len = output.len();
        let old_capacity = output.capacity();
        let Some(capacity) = planned_capacity(len, old_capacity, stride, max_bytes) else {
            continue;
        };
        // The stock write/compute steps still populate this frame's real len.
        // Public reserve changes capacity only; no padding becomes a live item.
        output.reserve(capacity, &device);
        debug_assert_eq!(output.len(), len);
        if *LOG_GROWTH {
            info!(
                ?phase,
                logical_len = len,
                old_capacity_bytes = old_capacity.saturating_mul(stride),
                new_capacity_bytes = output.capacity().saturating_mul(stride),
                "Reserved engine preprocessing output buffer capacity"
            );
        }
    }
}

fn planned_capacity(len: usize, capacity: usize, stride: usize, max_bytes: u64) -> Option<usize> {
    if len == 0 || len <= capacity || stride == 0 {
        return None;
    }
    let max_elements = usize::try_from(max_bytes).unwrap_or(usize::MAX) / stride;
    if len > max_elements {
        // Do not add headroom to a request the adapter cannot legally bind.
        // The stock path retains responsibility for its original allocation.
        return None;
    }
    let grain = MIN_GRAIN_BYTES.div_ceil(stride);
    let padded_limit = len.saturating_add(MAX_PADDING_BYTES / stride);
    Some(
        len.checked_next_power_of_two()
            .unwrap_or(max_elements)
            .max(grain)
            .min(padded_limit)
            .min(max_elements),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::{BufferUsages, UninitBufferVec};

    #[test]
    fn incremental_growth_uses_few_reservations_without_shrinking_or_live_padding() {
        let mut capacity = 0;
        let mut reservations = 0;
        for len in 1..=65_536 {
            if let Some(next) = planned_capacity(len, capacity, 128, 128 * 1024 * 1024) {
                assert!(next > capacity);
                assert!(next >= len);
                assert!((next - len) * 128 <= MAX_PADDING_BYTES);
                capacity = next;
                reservations += 1;
            }
            assert!(capacity >= len);
        }
        assert!(reservations <= 9, "reservations={reservations}");
        assert_eq!(capacity, 65_536);
        assert_eq!(planned_capacity(7, capacity, 128, u64::MAX), None);
    }

    #[test]
    fn padding_and_adapter_limits_bound_every_new_capacity() {
        let stride = 128;
        let len = 4 * MAX_PADDING_BYTES / stride + 3;
        let padded = planned_capacity(len, 0, stride, u64::MAX).unwrap();
        assert!(padded > len);
        assert_eq!((padded - len) * stride, MAX_PADDING_BYTES);

        let limit = 1000 * stride;
        let limited = planned_capacity(900, 0, stride, limit as u64).unwrap();
        assert_eq!(limited, 1000);
        assert!(limited * stride <= limit);
        assert_eq!(planned_capacity(1001, 0, stride, limit as u64), None);
        assert_eq!(planned_capacity(1, 0, stride, 0), None);
        assert_eq!(planned_capacity(0, 0, stride, 1024), None);
        assert_eq!(planned_capacity(1, 0, 0, 1024), None);
    }

    #[test]
    fn power_of_two_rounding_overflow_cannot_exceed_the_device_limit() {
        assert_eq!(planned_capacity(usize::MAX, 0, 128, u64::MAX), None);
        if usize::BITS == 64 {
            let len = usize::MAX - 1;
            assert_eq!(planned_capacity(len, 0, 1, u64::MAX), Some(usize::MAX));
        }
    }

    #[test]
    fn planning_keeps_real_lengths_offsets_and_empty_buffers_unchanged() {
        let mut output = UninitBufferVec::<MeshUniform>::new(BufferUsages::STORAGE);
        assert_eq!(
            planned_capacity(
                output.len(),
                output.capacity(),
                size_of::<MeshUniform>(),
                u64::MAX
            ),
            None
        );
        assert_eq!(output.add_multiple(120), 0);
        assert_eq!(output.add_multiple(3), 120);
        let capacity = planned_capacity(
            output.len(),
            output.capacity(),
            size_of::<MeshUniform>(),
            128 * 1024 * 1024,
        )
        .unwrap();
        assert!(capacity >= 123);
        assert_eq!(output.len(), 123);
        assert_eq!(
            output.capacity(),
            0,
            "planning never allocates or changes logical data"
        );
        assert_eq!(output.add_multiple(2), 123);
        assert_eq!(output.len(), 125);
    }
}
