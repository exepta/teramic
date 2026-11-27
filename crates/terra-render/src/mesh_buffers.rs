//! Mesh allocator settings and opt-in telemetry while streaming geometry.
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        mesh::{
            RenderMesh,
            allocator::{
                ElementClass, MeshAllocator, MeshAllocatorSettings, MeshSlabId,
                allocate_and_free_meshes,
            },
        },
        render_asset::{ExtractedAssets, prepare_assets},
        render_resource::BufferId,
        renderer::RenderDevice,
        slab_allocator::{Slab, SlabItemLayout},
    },
};
use std::collections::HashMap;

const MIB: u64 = 1024 * 1024;
const STREAMING_SLAB_BYTES: u64 = 512 * MIB;

pub(crate) fn install(app: &mut App) {
    let smoke = std::env::args().any(|argument| argument.starts_with("--smoke-test"));
    let requested = std::env::var("TERRA_MESH_SLAB_MIB").ok();
    let telemetry = std::env::var("TERRA_CPU_SCOPES").is_ok_and(|value| value == "1");
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.init_resource::<MeshAllocatorSettings>();
        let mut settings = render.world_mut().resource_mut::<MeshAllocatorSettings>();
        let legacy = smoke && std::env::var("TERRA_MESH_SLAB_LEGACY_GROWTH").as_deref() == Ok("1");
        let override_cap = configure_slab_cap(&mut settings, smoke, requested.as_deref())
            .unwrap_or_else(|error| {
                warn!(?requested, error, "Ignoring invalid TERRA_MESH_SLAB_MIB");
                false
            });
        if !legacy {
            settings.growth_factor = 2.0;
            if !override_cap {
                limit_slab_size(&mut settings, STREAMING_SLAB_BYTES);
            }
        }
        let (growth_factor, max_slab_bytes) = (settings.growth_factor, settings.max_slab_size);
        if !legacy && !override_cap {
            render.add_systems(
                Render,
                size_streaming_pages
                    .before(allocate_and_free_meshes)
                    .in_set(RenderSystems::PrepareAssets),
            );
        }
        if telemetry {
            info!(growth_factor, max_slab_bytes, "Mesh GPU slab growth");
            render.add_systems(
                Render,
                observe
                    .after(allocate_and_free_meshes)
                    .before(prepare_assets::<RenderMesh>)
                    .in_set(RenderSystems::PrepareAssets),
            );
        }
        render.add_systems(RenderStartup, limit_slabs_to_device);
    }
}

/// Keep the allocator's small initial buffers for infrequent vertex formats.
/// Once a format fills a slab, subsequent slabs of known formats can take
/// chunk uploads in their free ranges without relocating all resident meshes.
fn size_streaming_pages(
    allocator: Res<MeshAllocator>,
    extracted: Res<ExtractedAssets<RenderMesh>>,
    mut settings: ResMut<MeshAllocatorSettings>,
    mut layouts: Local<Vec<(ElementClass, u64)>>,
) {
    if extracted.extracted.is_empty() {
        return;
    }
    layouts.clear();
    let mut has_full_page = false;
    for slab in allocator.slabs.values() {
        // A dedicated allocation cannot accept a second mesh of that layout.
        if !matches!(slab, Slab::General(_)) {
            continue;
        }
        let layout = slab.element_layout();
        let slot_size = layout.size() * u64::from(layout.elements_per_slot());
        has_full_page |= slab
            .buffer()
            .is_some_and(|buffer| buffer.size() >= settings.max_slab_size / slot_size * slot_size);
        let key = (slab.element_class(), layout.size());
        if !layouts.contains(&key) {
            layouts.push(key);
        }
    }
    let streaming = has_full_page
        && extracted
            .extracted
            .iter()
            .all(|(_, mesh)| resident_layouts(mesh, &layouts));
    let minimum = if streaming {
        settings.max_slab_size
    } else {
        MIB.min(settings.max_slab_size)
    };
    if minimum != settings.min_slab_size {
        settings.min_slab_size = minimum;
    }
}

fn resident_layouts(mesh: &Mesh, layouts: &[(ElementClass, u64)]) -> bool {
    layouts.contains(&(ElementClass::Vertex, mesh.get_vertex_size()))
        && mesh.indices().is_none_or(|indices| {
            let size = match indices {
                bevy::mesh::Indices::U16(_) => 2,
                bevy::mesh::Indices::U32(_) => 4,
            };
            layouts.contains(&(ElementClass::Index, size))
        })
        && ((mesh.final_aabb.is_none() && mesh.final_uv_ranges.iter().all(Option::is_none))
            || layouts
                .iter()
                .any(|(class, _)| *class == ElementClass::Metadata))
        && (mesh.get_morph_targets().is_none()
            || layouts
                .iter()
                .any(|(class, _)| *class == ElementClass::MorphTarget))
}

fn limit_slabs_to_device(device: Res<RenderDevice>, mut settings: ResMut<MeshAllocatorSettings>) {
    let limits = device.limits();
    limit_device_pages(
        &mut settings,
        limits.max_buffer_size,
        limits.max_vertex_buffer_array_stride,
    );
}

fn limit_device_pages(settings: &mut MeshAllocatorSettings, buffer_limit: u64, vertex_stride: u32) {
    // Bevy packs up to four vertices per copy-aligned slot. A new page rounds
    // its minimum UP to a slot boundary, even when minimum equals maximum.
    let largest_slot = (4 * u64::from(vertex_stride))
        .max(size_of::<bevy::render::mesh::MeshMetadata>() as u64)
        .max(size_of::<bevy::mesh::morph::MorphAttributes>() as u64)
        .max(4);
    let half = buffer_limit.div_ceil(2);
    // At half the limit, any individually supported slot fits after rounding:
    // larger slots need only one slot, smaller ones fit in the remaining half.
    let cap = buffer_limit.saturating_sub(largest_slot - 1).max(half);
    limit_slab_size(settings, cap);
    // The offset allocator can round the first object's slot count up to a
    // size class (at most the next power of two). Route very large first
    // objects directly to an exactly-sized dedicated buffer.
    settings.large_threshold = settings.large_threshold.min(half);
}

fn limit_slab_size(settings: &mut MeshAllocatorSettings, cap: u64) {
    settings.max_slab_size = settings.max_slab_size.min(cap);
    settings.min_slab_size = settings.min_slab_size.min(settings.max_slab_size);
    // Bevy routes individually large meshes to dedicated slabs.
    settings.large_threshold = settings.large_threshold.min(settings.max_slab_size);
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
    limit_slab_size(settings, mib * 1024 * 1024);
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

    #[test]
    fn streaming_pages_require_each_class_and_stride_in_the_incoming_mesh() {
        use bevy::{
            asset::RenderAssetUsages,
            mesh::{Indices, PrimitiveTopology},
        };
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.; 3]; 3]);
        let mut layouts = vec![(ElementClass::Vertex, 12)];
        assert!(resident_layouts(&mesh, &layouts));
        mesh.insert_indices(Indices::U16(vec![0, 1, 2]));
        layouts.push((ElementClass::Index, 4));
        assert!(!resident_layouts(&mesh, &layouts));
        layouts.push((ElementClass::Index, 2));
        assert!(resident_layouts(&mesh, &layouts));
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.; 3]; 3]);
        assert!(!resident_layouts(&mesh, &layouts));
        layouts.push((ElementClass::Vertex, 24));
        assert!(resident_layouts(&mesh, &layouts));
        mesh.final_aabb = Some(bevy::shape::Aabb3d::new(Vec3::ZERO, Vec3::ONE));
        assert!(!resident_layouts(&mesh, &layouts));
        layouts.push((ElementClass::Metadata, 32));
        assert!(resident_layouts(&mesh, &layouts));
    }

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
    fn device_limit_preserves_smaller_smoke_caps_and_dedicated_allocations() {
        let mut settings = MeshAllocatorSettings::default();
        settings.growth_factor = 2.0;
        limit_slab_size(&mut settings, 128 * MIB);
        assert_eq!(limits(&settings), (MIB, 128 * MIB, 128 * MIB, 2.0));
        configure_slab_cap(&mut settings, true, Some("32")).unwrap();
        limit_slab_size(&mut settings, 256 * MIB);
        assert_eq!(limits(&settings), (MIB, 32 * MIB, 32 * MIB, 2.0));
    }

    #[test]
    fn reserved_pages_fit_device_limits_after_slot_rounding() {
        for limit in [1, 4, 35, 64, 127, 128, 257, 997, MIB, 256 * MIB, 512 * MIB] {
            let mut settings = MeshAllocatorSettings::default();
            limit_device_pages(&mut settings, limit, 2048);
            // The largest supported vertex slot is bounded by four strides.
            // Include non-power-of-two layouts such as 48-byte vertices.
            for slot in (4..=limit.min(4 * 2048)).step_by(4) {
                assert!(settings.max_slab_size.next_multiple_of(slot) <= limit);
            }
            assert!(settings.min_slab_size <= settings.max_slab_size);
            assert!(settings.large_threshold <= limit.div_ceil(2));
        }
        let mut settings = MeshAllocatorSettings::default();
        let defaults = limits(&settings);
        limit_device_pages(&mut settings, 2 * 1024 * MIB, 2048);
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
