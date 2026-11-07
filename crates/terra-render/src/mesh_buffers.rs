//! Opt-in telemetry and smoke-test allocator limits while streaming geometry.
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        mesh::{
            RenderMesh,
            allocator::{
                MeshAllocator, MeshAllocatorSettings, MeshSlabId, allocate_and_free_meshes,
            },
        },
        render_asset::{ExtractedAssets, prepare_assets},
        render_resource::BufferId,
    },
};
use std::collections::HashMap;

pub(crate) fn install(app: &mut App) {
    let smoke = std::env::args().any(|argument| argument.starts_with("--smoke-test"));
    let requested = std::env::var("TERRA_MESH_SLAB_MIB").ok();
    let telemetry = std::env::var("TERRA_CPU_SCOPES").is_ok_and(|value| value == "1");
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        if smoke && requested.is_some() {
            render.init_resource::<MeshAllocatorSettings>();
            let mut settings = render.world_mut().resource_mut::<MeshAllocatorSettings>();
            match configure_slab_cap(&mut settings, smoke, requested.as_deref()) {
                Ok(true) => info!(
                    max_slab_bytes = settings.max_slab_size,
                    min_slab_bytes = settings.min_slab_size,
                    large_threshold_bytes = settings.large_threshold,
                    "Smoke-test mesh allocator cap enabled; normal gameplay is unchanged"
                ),
                Err(error) => warn!(?requested, error, "Ignoring invalid TERRA_MESH_SLAB_MIB"),
                Ok(false) => {}
            }
        }
        if telemetry {
            render.add_systems(
                Render,
                observe
                    .after(allocate_and_free_meshes)
                    .before(prepare_assets::<RenderMesh>)
                    .in_set(RenderSystems::PrepareAssets),
            );
        }
    }
}

fn configure_slab_cap(
    settings: &mut MeshAllocatorSettings,
    smoke: bool,
    requested: Option<&str>,
) -> Result<bool, &'static str> {
    let Some(requested) = requested.filter(|_| smoke) else {
        return Ok(false);
    };
    let mib = requested
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|mib| (1..=512).contains(mib))
        .ok_or("expected an integer from 1 to 512 MiB")?;
    let cap = mib * 1024 * 1024;
    settings.max_slab_size = settings.max_slab_size.min(cap);
    settings.min_slab_size = settings.min_slab_size.min(settings.max_slab_size);
    // Stock Bevy routes allocations at/above this bound into dedicated slabs.
    // Do not reject or truncate a mesh that is larger than the general slab cap.
    settings.large_threshold = settings.large_threshold.min(settings.max_slab_size);
    Ok(true)
}

fn observe(
    allocator: Res<MeshAllocator>,
    extracted: Res<ExtractedAssets<RenderMesh>>,
    mut previous: Local<HashMap<MeshSlabId, (BufferId, u64)>>,
) {
    let summary = summarize_meshes(extracted.extracted.iter().map(|(id, mesh)| (*id, mesh)));
    if summary.heavy(extracted.removed.len()) {
        info!(
            added_meshes = extracted.added.len(),
            modified_meshes = extracted.modified.len(),
            removed_meshes = extracted.removed.len(),
            extracted_meshes = summary.meshes,
            vertices = summary.vertices,
            vertex_bytes = summary.vertex_bytes,
            index_bytes = summary.index_bytes,
            largest_mesh = ?summary.largest.as_ref().map(|mesh| mesh.id),
            largest_mesh_vertices = summary.largest.as_ref().map_or(0, |mesh| mesh.vertices),
            largest_mesh_vertex_bytes = summary.largest.as_ref().map_or(0, |mesh| mesh.vertex_bytes),
            largest_mesh_index_bytes = summary.largest.as_ref().map_or(0, |mesh| mesh.index_bytes),
            largest_mesh_attributes = ?summary.largest.as_ref().map(|mesh| &mesh.attributes),
            "Large extracted mesh transaction after stock allocator (CPU metadata only; not GPU time)"
        );
    }
    let mut copies = 0;
    let mut copied_bytes = 0;
    let mut allocations = 0;
    for (id, slab) in &allocator.slabs {
        let Some(buffer) = slab.buffer() else {
            continue;
        };
        let next = (buffer.id(), buffer.size());
        match previous.insert(*id, next) {
            Some((old_id, old_size)) if old_id != next.0 => {
                copies += 1;
                copied_bytes += old_size;
            }
            None => allocations += 1,
            _ => {}
        }
    }
    previous.retain(|id, _| allocator.slabs.contains_key(id));
    if copies > 0 || allocations > 0 {
        info!(
            copies,
            copied_bytes,
            allocations,
            slabs = allocator.slab_count(),
            reserved_bytes = allocator.slabs_size(),
            "Mesh GPU buffer allocation (buffer capacity, not total VRAM)"
        );
    }
}

#[derive(Debug)]
struct MeshSize {
    id: AssetId<Mesh>,
    vertices: usize,
    vertex_bytes: u64,
    index_bytes: u64,
    attributes: Vec<(
        &'static str,
        bevy::render::render_resource::VertexFormat,
        usize,
    )>,
}

#[derive(Debug, Default)]
struct MeshSummary {
    meshes: usize,
    vertices: usize,
    vertex_bytes: u64,
    index_bytes: u64,
    largest: Option<MeshSize>,
}

impl MeshSummary {
    fn heavy(&self, removed: usize) -> bool {
        self.vertices >= 128_000 || removed >= 64
    }
}

fn summarize_meshes<'a>(meshes: impl Iterator<Item = (AssetId<Mesh>, &'a Mesh)>) -> MeshSummary {
    let mut summary = MeshSummary::default();
    for (id, mesh) in meshes {
        // These APIs inspect attribute/index lengths, not vertex values.
        let mut size = MeshSize {
            id,
            vertices: mesh.count_vertices(),
            vertex_bytes: mesh.get_vertex_buffer_size() as u64,
            index_bytes: mesh
                .get_index_buffer_bytes()
                .map_or(0, |bytes| bytes.len() as u64),
            attributes: Vec::new(),
        };
        summary.meshes += 1;
        summary.vertices = summary.vertices.saturating_add(size.vertices);
        summary.vertex_bytes = summary.vertex_bytes.saturating_add(size.vertex_bytes);
        summary.index_bytes = summary.index_bytes.saturating_add(size.index_bytes);
        if summary.largest.as_ref().is_none_or(|largest| {
            size.vertex_bytes + size.index_bytes > largest.vertex_bytes + largest.index_bytes
        }) {
            size.attributes = mesh
                .attributes()
                .map(|(attribute, values)| (attribute.name, attribute.format, values.len()))
                .collect();
            summary.largest = Some(size);
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1024 * 1024;

    #[test]
    fn extraction_summary_counts_metadata_and_largest_combined_payload() {
        use bevy::{
            asset::RenderAssetUsages,
            mesh::{Indices, PrimitiveTopology},
        };
        let mut meshes = Assets::<Mesh>::default();
        let first = meshes.add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0; 3]; 3])
            .with_inserted_indices(Indices::U16(vec![0, 1, 2])),
        );
        let second = meshes.add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0; 3]; 2])
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0; 3]; 2])
            .with_inserted_indices(Indices::U32(vec![0, 1, 0])),
        );
        let summary = summarize_meshes(meshes.iter());
        assert_eq!((summary.meshes, summary.vertices), (2, 5));
        assert_eq!((summary.vertex_bytes, summary.index_bytes), (84, 18));
        let largest = summary.largest.unwrap();
        assert_eq!(largest.id, second.id());
        assert_ne!(largest.id, first.id());
        assert_eq!(
            (largest.vertices, largest.vertex_bytes, largest.index_bytes),
            (2, 48, 12)
        );
        let empty = summarize_meshes(std::iter::empty());
        assert_eq!(
            (
                empty.meshes,
                empty.vertices,
                empty.vertex_bytes,
                empty.index_bytes
            ),
            (0, 0, 0, 0)
        );
        assert!(empty.largest.is_none());
    }

    #[test]
    fn extraction_logging_thresholds_do_not_require_allocating_large_meshes() {
        let mut summary = MeshSummary {
            vertices: 127_999,
            ..Default::default()
        };
        assert!(!summary.heavy(63));
        assert!(summary.heavy(64));
        summary.vertices = 128_000;
        assert!(summary.heavy(0));
    }

    fn limits(settings: &MeshAllocatorSettings) -> (u64, u64, u64, f64) {
        (
            settings.min_slab_size,
            settings.max_slab_size,
            settings.large_threshold,
            settings.growth_factor,
        )
    }

    #[test]
    fn defaults_and_normal_gameplay_ignore_the_benchmark_override() {
        let mut settings = MeshAllocatorSettings::default();
        let defaults = limits(&settings);
        assert!(!configure_slab_cap(&mut settings, true, None).unwrap());
        assert!(!configure_slab_cap(&mut settings, false, Some("32")).unwrap());
        assert!(!configure_slab_cap(&mut settings, false, Some("invalid")).unwrap());
        assert_eq!(limits(&settings), defaults);
    }

    #[test]
    fn benchmark_caps_keep_sizes_consistent_without_changing_the_growth_factor() {
        for mib in [32, 64] {
            let mut settings = MeshAllocatorSettings::default();
            let growth = settings.growth_factor;
            assert!(configure_slab_cap(&mut settings, true, Some(&mib.to_string())).unwrap());
            assert_eq!(limits(&settings), (MIB, mib * MIB, mib * MIB, growth));
        }
        let mut settings = MeshAllocatorSettings {
            slab_allocator_settings: bevy::render::slab_allocator::SlabAllocatorSettings {
                min_slab_size: 64 * MIB,
                ..default()
            },
            ..default()
        };
        assert!(configure_slab_cap(&mut settings, true, Some("32")).unwrap());
        assert_eq!(settings.min_slab_size, 32 * MIB);
        assert_eq!(settings.max_slab_size, 32 * MIB);
    }

    #[test]
    fn invalid_and_too_small_limits_leave_existing_settings_untouched() {
        let mut settings = MeshAllocatorSettings::default();
        let original = limits(&settings);
        for requested in [
            "",
            "0",
            "0.5",
            "-1",
            "32MiB",
            "invalid",
            "513",
            "18446744073709551615",
        ] {
            assert!(configure_slab_cap(&mut settings, true, Some(requested)).is_err());
            assert_eq!(limits(&settings), original, "{requested}");
        }
    }

    #[test]
    fn oversized_allocations_keep_the_stock_dedicated_slab_fallback() {
        use bevy::render::{
            render_resource::BufferUsages,
            slab_allocator::{Slab, SlabAllocator, SlabItem, SlabItemLayout},
        };
        use std::borrow::Cow;

        struct TestItem;
        #[derive(Clone, PartialEq, Eq, Hash)]
        struct TestLayout;
        impl SlabItem for TestItem {
            type Key = u32;
            type Layout = TestLayout;
            fn label() -> Cow<'static, str> {
                "mesh cap regression".into()
            }
        }
        impl SlabItemLayout for TestLayout {
            fn size(&self) -> u64 {
                4
            }
            fn elements_per_slot(&self) -> u32 {
                1
            }
            fn buffer_usages(&self) -> BufferUsages {
                BufferUsages::VERTEX
            }
        }
        let mut settings = MeshAllocatorSettings::default();
        configure_slab_cap(&mut settings, true, Some("1")).unwrap();
        let mut allocator = SlabAllocator::<TestItem>::new();
        {
            // Dedicated-slab routing is CPU-only; this does not create GPU buffers.
            let mut stage = allocator.stage_allocation();
            stage.allocate(&7, 2 * MIB, TestLayout, &settings.slab_allocator_settings);
            let slab = stage.allocator.key_to_slab[&7];
            assert!(matches!(stage.allocator.slabs[&slab], Slab::LargeObject(_)));
        }
        assert_eq!(allocator.slab_count(), 1);
    }
}
