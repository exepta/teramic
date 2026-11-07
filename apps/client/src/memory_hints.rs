use bevy::{
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        renderer::{RenderAdapterInfo, RenderDevice},
        settings::MemoryHints,
    },
};

const MIB: u64 = 1024 * 1024;

pub(crate) fn options(
    smoke_test: bool,
    requested: Option<&str>,
) -> Result<MemoryHints, &'static str> {
    let Some(requested) = requested.filter(|_| smoke_test) else {
        return Ok(MemoryHints::default());
    };
    let mib = requested
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|mib| (64..=512).contains(mib))
        .ok_or("TERRA_WGPU_MEMORY_BLOCK_MIB expects an integer from 64 to 512 in smoke tests")?;
    Ok(MemoryHints::Manual {
        suballocated_device_memory_block_size: mib * MIB..2 * mib * MIB,
    })
}

#[derive(Resource)]
struct Probe(std::ops::Range<u64>);

pub(crate) fn install_report(app: &mut App, hints: &MemoryHints) {
    let MemoryHints::Manual {
        suballocated_device_memory_block_size,
    } = hints
    else {
        return;
    };
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(Probe(suballocated_device_memory_block_size.clone()))
            .add_systems(RenderStartup, report);
    }
}

fn report(probe: Res<Probe>, adapter: Res<RenderAdapterInfo>, device: Res<RenderDevice>) {
    let limits = device.limits();
    let allocator = device.wgpu_device().generate_allocator_report();
    info!(
        backend = ?adapter.backend,
        requested_device_block_min_bytes = probe.0.start,
        requested_device_block_max_bytes = probe.0.end,
        max_buffer_bytes = limits.max_buffer_size,
        max_storage_binding_bytes = limits.max_storage_buffer_binding_size,
        startup_allocator_allocated_bytes = allocator.as_ref().map(|report| report.total_allocated_bytes),
        startup_allocator_reserved_bytes = allocator.as_ref().map(|report| report.total_reserved_bytes),
        "Smoke-test WGPU memory block hints; backend may clamp, not a total VRAM or resource-size budget"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_ignores_even_invalid_smoke_hints() {
        for requested in [None, Some("256"), Some("0"), Some("invalid"), Some("")] {
            assert!(matches!(
                options(false, requested).unwrap(),
                MemoryHints::Performance
            ));
        }
    }

    #[test]
    fn smoke_without_an_override_keeps_the_performance_default() {
        assert!(matches!(
            options(true, None).unwrap(),
            MemoryHints::Performance
        ));
    }

    #[test]
    fn smoke_hints_bound_the_requested_allocation_block_range() {
        for (requested, mib) in [("64", 64), ("256", 256), ("512", 512), (" 128 ", 128)] {
            let MemoryHints::Manual {
                suballocated_device_memory_block_size,
            } = options(true, Some(requested)).unwrap()
            else {
                panic!("Smoke override must select manual memory hints");
            };
            assert_eq!(
                suballocated_device_memory_block_size,
                mib * MIB..2 * mib * MIB
            );
        }
    }

    #[test]
    fn malformed_or_out_of_bounds_smoke_hints_are_rejected() {
        for requested in [
            "0",
            "63",
            "513",
            "-256",
            "256.0",
            "invalid",
            "",
            " ",
            "184467440737095516160",
        ] {
            assert!(options(true, Some(requested)).is_err(), "{requested:?}");
        }
    }
}
