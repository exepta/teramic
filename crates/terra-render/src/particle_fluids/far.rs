//! Reservoirs reuse worker-built shorelines and depth/current optics. Finite
//! liquids reconstruct their volumes; both are owned by this bounded uploader.
use super::*;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    platform::collections::HashMap,
    prelude::*,
};
use terra_voxel::SectionPos;

#[derive(Default, Resource)]
struct Cache(HashMap<SectionPos, Tile>);
type Part = (Entity, Handle<Mesh>, usize);
struct Tile {
    epoch: u64,
    water: Option<Part>,
    reconstructed: Vec<Part>,
}
impl Tile {
    fn parts(&self) -> impl Iterator<Item = &Part> {
        self.water.iter().chain(&self.reconstructed)
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Cache>().add_systems(
        Update,
        upload.after(terra_world::streaming::StreamingSet::Receive),
    );
}
#[derive(bevy::ecs::system::SystemParam)]
struct View<'w> {
    source: Res<'w, FarFluidSections>,
    materials: Option<Res<'w, super::surface::ParkedMaterials>>,
    focus: Option<Res<'w, terra_world::streaming::ChunkFocus>>,
    status: ResMut<'w, crate::water::WaterStatus>,
    budget: Option<ResMut<'w, crate::MeshUploadStatus>>,
    water: Option<Res<'w, crate::water::material::WaterAssets>>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    time: Option<Res<'w, Time>>,
}
fn retire(tile: Tile, commands: &mut Commands, meshes: &mut Assets<Mesh>) {
    for (entity, mesh, _) in tile.water.into_iter().chain(tile.reconstructed) {
        commands.entity(entity).despawn();
        meshes.remove(mesh.id());
    }
}
fn upload(
    mut commands: Commands,
    mut view: View,
    mut cache: ResMut<Cache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut pending_uploads: Local<bool>,
    mut motion: Option<ResMut<crate::liquid_motion::History>>,
) {
    let Some(materials) = view.materials.as_ref() else {
        return;
    };
    if !view.source.is_changed() && !*pending_uploads {
        return;
    }
    let removed: Vec<_> = cache
        .0
        .keys()
        .filter(|p| !view.source.sections.contains_key(*p))
        .copied()
        .collect();
    for p in removed {
        if let Some(history) = motion.as_deref_mut() {
            let key = crate::liquid_motion::Key(p, crate::liquid_motion::Kind::Water);
            let now = view.time.as_ref().map_or(0., |t| t.elapsed_secs());
            if let Some((mesh, duration, _)) = history.drain(key, now)
                && let Some(water) = &view.water
                && let Ok(origin) = p.origin()
            {
                let handle = meshes.add(mesh);
                commands.spawn((
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(water.0.clone()),
                    Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                    crate::liquid_motion::retiring(key, now + duration, handle),
                    crate::miniature::layers(),
                    bevy::light::NotShadowCaster,
                    Name::new("Draining confirmed water"),
                ));
            }
        }
        if let Some(tile) = cache.0.remove(&p) {
            retire(tile, &mut commands, &mut meshes);
        }
    }
    let focus = view.focus.as_ref().map_or([0.; 3], |f| f.0);
    let mut pending: Vec<_> = view
        .source
        .sections
        .iter()
        .filter(|(p, s)| cache.0.get(*p).is_none_or(|t| t.epoch != s.epoch))
        .map(|(&p, _)| p)
        .collect();
    pending.sort_by_key(|p| crate::meshing::lod::distance_squared(*p, focus) as u64);
    for position in pending.into_iter().take(4) {
        let section = &view.source.sections[&position];
        let Ok(origin) = position.origin() else {
            continue;
        };
        let geometry: [_; 4] = std::array::from_fn(|kind| {
            let mut geometry = reconstruct(&section.fields[kind]);
            for film in section.films.iter().filter(|f| f.kind as usize == kind) {
                geometry.film(film.minimum, film.maximum);
            }
            geometry
        });
        if section.published_water.is_some() && view.water.is_none() {
            continue;
        }
        let vertices = geometry.iter().map(|g| g.positions.len()).sum::<usize>()
            + section
                .published_water
                .as_ref()
                .map_or(0, |p| p.positions.len());
        if view
            .budget
            .as_deref_mut()
            .is_some_and(|b| !b.reserve(crate::upload_budget::UploadKind::Water, vertices))
        {
            break;
        }
        let mut water_part = None;
        let mut reconstructed = Vec::new();
        if let (Some(patch), Some(water)) = (&section.published_water, &view.water)
            && !patch.indices.is_empty()
        {
            let triangles = patch.indices.len() / 3;
            let mut geometry =
                published_water_mesh(patch, origin, view.terrain.as_deref().map(|t| &t.0));
            if view
                .journal
                .as_ref()
                .is_some_and(|j| j.water_mesh_epoch(position) > 0)
                && let Some(history) = motion.as_deref_mut()
            {
                history.prepare(
                    crate::liquid_motion::Key(position, crate::liquid_motion::Kind::Water),
                    &mut geometry,
                    view.time.as_ref().map_or(0., |t| t.elapsed_secs()),
                    cache.0.contains_key(&position),
                    0,
                );
            }
            let bounds = crate::liquid_motion::bounds(&geometry);
            // Retain the entity, draw order and asset ID throughout the pour.
            // The render world can continue drawing the last prepared GPU mesh
            // while its replacement uploads under the same handle.
            let previous = cache
                .0
                .get_mut(&position)
                .and_then(|tile| tile.water.take());
            let (entity, mesh) = if let Some((entity, mesh, _)) = previous {
                if let Err(error) = meshes.insert(mesh.id(), geometry) {
                    tracing::warn!(%error, "Stale water mesh handle; rebuilding on the next upload");
                    commands.entity(entity).despawn();
                    continue;
                }
                (entity, mesh)
            } else {
                let mesh = meshes.add(geometry);
                let entity = commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(water.0.clone()),
                        Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                        crate::miniature::layers(),
                        bevy::light::NotShadowCaster,
                        terra_core::miniature::MiniatureMesh::Water,
                        Name::new(format!(
                            "Natural water body {},{},{}",
                            position.x, position.y, position.z
                        )),
                    ))
                    .id();
                (entity, mesh)
            };
            if let Some(mut bounds) = bounds {
                bounds.half_extents.y += 0.24;
                commands
                    .entity(entity)
                    .insert((bounds, bevy::camera::visibility::NoAutoAabb));
            }
            water_part = Some((entity, mesh, triangles));
        }
        for (kind, geometry) in geometry.into_iter().enumerate() {
            if geometry.indices.is_empty() {
                continue;
            }
            let triangles = geometry.indices.len() / 3;
            let mesh = meshes.add(geometry.mesh());
            let entity = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(materials.0[kind].clone()),
                    Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                    crate::miniature::layers(),
                    bevy::light::NotShadowCaster,
                    Name::new(format!(
                        "Reconstructed fluid {kind} {},{},{}",
                        position.x, position.y, position.z
                    )),
                ))
                .id();
            reconstructed.push((entity, mesh, triangles));
        }
        if let Some(old) = cache.0.insert(
            position,
            Tile {
                epoch: section.epoch,
                water: water_part,
                reconstructed,
            },
        ) {
            retire(old, &mut commands, &mut meshes);
        }
    }
    view.status.ready = view.source.revision > 0
        && view.source.sections.iter().all(|(position, section)| {
            cache
                .0
                .get(position)
                .is_some_and(|tile| tile.epoch == section.epoch)
        });
    *pending_uploads = !view.status.ready;
    view.status.initial_ready =
        view.status.ready || view.source.sections.keys().all(|p| cache.0.contains_key(p));
    view.status.visible_tiles = cache
        .0
        .values()
        .filter(|t| t.parts().next().is_some())
        .count();
    view.status.triangles = cache
        .0
        .values()
        .flat_map(Tile::parts)
        .map(|(_, _, n)| *n)
        .sum();
    view.status.spray_particles = 0;
    view.status.mist_volumes = 0;
}

/// Preserve the worker's exact shoreline, bed depth, current and waterfall
/// attachment geometry. No density grid or camera-relative remeshing is needed.
fn published_water_mesh(
    patch: &terra_world::water::WaterPatch,
    origin: [i64; 3],
    terrain: Option<&terra_world::TerraGenerator>,
) -> Mesh {
    use crate::water::{
        biome::{ATTRIBUTE_BIOME_TINT, ATTRIBUTE_BLOOD_WEIGHT, WaterTint},
        material::ATTRIBUTE_WAVE_WEIGHT,
    };
    let count = patch.positions.len();
    let normals = if patch.normals.len() == count {
        patch.normals.clone()
    } else {
        vec![[0., 1., 0.]; count]
    };
    let flow = if patch.flow.len() == count {
        patch.flow.clone()
    } else {
        vec![[0.; 2]; count]
    };
    let depth = if patch.depth_shore.len() == count {
        patch.depth_shore.clone()
    } else {
        vec![[1., 0.]; count]
    };
    let mut effects = if patch.surface_effects.len() == count {
        patch.surface_effects.clone()
    } else {
        vec![[0.; 4]; count]
    };
    if let Some(terrain) = terrain {
        for ((effect, p), normal) in effects.iter_mut().zip(&patch.positions).zip(&normals) {
            if effect[3] < 0.5
                && normal[1] > 0.95
                && (origin[1] as f32 + p[1] - terrain.config().sea_level as f32).abs() < 0.05
            {
                effect[2] = 1.;
            }
        }
    }
    let tint = terrain.map(|t| WaterTint::new(t, origin));
    let colors: Vec<_> = patch
        .positions
        .iter()
        .map(|&p| {
            tint.as_ref()
                .map_or([0.025, 0.20, 0.16, 0.], |t| t.payload(p))
        })
        .collect();
    let blood: Vec<_> = patch
        .positions
        .iter()
        .map(|&p| tint.as_ref().map_or(0., |t| t.blood_at(p)))
        .collect();
    let waves = if patch.wave_weights.len() == count {
        patch.wave_weights.clone()
    } else {
        vec![0.; count]
    };
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, patch.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, depth)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, flow)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, effects)
    .with_inserted_attribute(ATTRIBUTE_BIOME_TINT, colors)
    .with_inserted_attribute(ATTRIBUTE_BLOOD_WEIGHT, blood)
    .with_inserted_attribute(ATTRIBUTE_WAVE_WEIGHT, waves)
    .with_inserted_indices(Indices::U32(patch.indices.clone()))
}

#[derive(Default)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}
impl Geometry {
    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_indices(Indices::U32(self.indices))
    }
    fn triangle(&mut self, mut p: [Vec3; 3], mut n: [Vec3; 3]) {
        let cross = (p[1] - p[0]).cross(p[2] - p[0]);
        if cross.length_squared() < 1e-12 {
            return;
        }
        let outward = (n[0] + n[1] + n[2]).try_normalize().unwrap_or(Vec3::Y);
        // Front faces are culled for both the raymarch proxy and these meshes.
        if cross.dot(outward) > 0. {
            p.swap(1, 2);
            n.swap(1, 2);
        }
        let start = self.positions.len() as u32;
        self.positions.extend(p.map(|v| v.to_array()));
        self.normals
            .extend(n.map(|v| v.try_normalize().unwrap_or(outward).to_array()));
        self.indices.extend([start, start + 1, start + 2]);
    }
    fn film(&mut self, lo: [f32; 3], hi: [f32; 3]) {
        if (0..3).any(|i| hi[i] <= lo[i]) {
            return;
        }
        let p = std::array::from_fn::<_, 8, _>(|i| {
            Vec3::new(
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            )
        });
        for (ids, normal) in [
            ([0, 4, 6, 2], -Vec3::X),
            ([1, 3, 7, 5], Vec3::X),
            ([0, 1, 5, 4], -Vec3::Y),
            ([2, 6, 7, 3], Vec3::Y),
            ([0, 2, 3, 1], -Vec3::Z),
            ([4, 5, 7, 6], Vec3::Z),
        ] {
            self.triangle([p[ids[0]], p[ids[1]], p[ids[2]]], [normal; 3]);
            self.triangle([p[ids[0]], p[ids[2]], p[ids[3]]], [normal; 3]);
        }
    }
}
const N: usize = 17;
fn index(x: usize, y: usize, z: usize) -> usize {
    x + N * (y + N * z)
}
fn gradient(field: &[f32], p: [usize; 3]) -> Vec3 {
    Vec3::from_array(std::array::from_fn(|axis| {
        let mut a = p;
        let mut b = p;
        a[axis] = p[axis].saturating_sub(1);
        b[axis] = (p[axis] + 1).min(16);
        -(field[index(b[0], b[1], b[2])] - field[index(a[0], a[1], a[2])])
            / (b[axis] - a[axis]) as f32
    }))
}
fn reconstruct(field: &[f32]) -> Geometry {
    let mut result = Geometry::default();
    if field.len() != N * N * N {
        return result;
    }
    const CORNERS: [[usize; 3]; 8] = [
        [0, 0, 0],
        [1, 0, 0],
        [1, 1, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 0, 1],
        [1, 1, 1],
        [0, 1, 1],
    ];
    const TETS: [[usize; 4]; 6] = [
        [0, 5, 1, 6],
        [0, 1, 2, 6],
        [0, 2, 3, 6],
        [0, 3, 7, 6],
        [0, 7, 4, 6],
        [0, 4, 5, 6],
    ];
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                let grid = CORNERS.map(|c| [x + c[0], y + c[1], z + c[2]]);
                let values = grid.map(|p| field[index(p[0], p[1], p[2])]);
                if values.iter().all(|&v| v >= 0.) || values.iter().all(|&v| v < 0.) {
                    continue;
                }
                let positions = grid.map(|p| Vec3::from_array(p.map(|v| v as f32)));
                let normals = grid.map(|p| gradient(field, p));
                for tet in TETS {
                    let mut polygon: Vec<(Vec3, Vec3)> = Vec::with_capacity(4);
                    for (a, b) in [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)] {
                        let a = tet[a];
                        let b = tet[b];
                        if (values[a] >= 0.) == (values[b] >= 0.) {
                            continue;
                        }
                        let t = (values[a] / (values[a] - values[b])).clamp(0., 1.);
                        let p = positions[a].lerp(positions[b], t);
                        if polygon.iter().any(|(q, _)| q.distance_squared(p) < 1e-12) {
                            continue;
                        }
                        polygon.push((p, normals[a].lerp(normals[b], t)));
                    }
                    if polygon.len() < 3 {
                        continue;
                    }
                    let center =
                        polygon.iter().map(|(p, _)| *p).sum::<Vec3>() / polygon.len() as f32;
                    let normal = polygon
                        .iter()
                        .map(|(_, n)| *n)
                        .sum::<Vec3>()
                        .try_normalize()
                        .unwrap_or(Vec3::Y);
                    let tangent = normal.any_orthonormal_vector();
                    let bitangent = normal.cross(tangent);
                    polygon.sort_by(|(a, _), (b, _)| {
                        let a = *a - center;
                        let b = *b - center;
                        a.dot(bitangent)
                            .atan2(a.dot(tangent))
                            .total_cmp(&b.dot(bitangent).atan2(b.dot(tangent)))
                    });
                    for i in 1..polygon.len() - 1 {
                        result.triangle(
                            [polygon[0].0, polygon[i].0, polygon[i + 1].0],
                            [polygon[0].1, polygon[i].1, polygon[i + 1].1],
                        );
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_dry_upper_owner_interpolates_a_flow_from_full_cells_below_its_boundary() {
        use bevy::mesh::VertexAttributeValues;
        use terra_world::{
            edits::EditJournal,
            water::{FluidCell, WaterPatch},
        };
        let upper = SectionPos::new(0, 1, 0);
        let mut journal = EditJournal::default();
        journal
            .set_fluid(
                [1, 15, 1],
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0; 2],
                },
            )
            .unwrap();
        assert_eq!(
            journal.fluid_epoch(upper),
            0,
            "the surface owner has no water volume"
        );
        assert!(
            journal.water_mesh_epoch(upper) > 0,
            "the boundary halo owns its surface"
        );
        let patch = |right: f32| WaterPatch {
            positions: vec![[1., 0., 1.], [1., 0., 2.], [right, 0., 2.], [right, 0., 1.]],
            normals: vec![[0., 1., 0.]; 4],
            flow: vec![[1., 0.]; 4],
            depth_shore: vec![[1., 0.]; 4],
            surface_effects: vec![[0.; 4]; 4],
            wave_weights: vec![0.; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let mut source = FarFluidSections {
            revision: 1,
            ..default()
        };
        source.sections.insert(
            upper,
            FarFluidSection {
                epoch: journal.water_mesh_epoch(upper),
                fields: std::array::from_fn(|_| vec![]),
                films: vec![],
                published_water: Some(patch(2.)),
            },
        );
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Cache>()
            .init_resource::<crate::water::WaterStatus>()
            .init_resource::<crate::liquid_motion::History>()
            .init_resource::<Time>()
            .insert_resource(journal)
            .insert_resource(source)
            .insert_resource(super::super::surface::ParkedMaterials(std::array::from_fn(
                |_| Handle::default(),
            )))
            .insert_resource(crate::water::material::WaterAssets(Handle::default()))
            .add_systems(Update, upload);
        app.update();
        let original = app.world().resource::<Cache>().0[&upper]
            .water
            .as_ref()
            .unwrap()
            .1
            .clone();
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&original)
            .unwrap();
        assert!(mesh.attribute(crate::liquid_motion::FROM).is_some());
        assert!(mesh.attribute(crate::liquid_motion::DATA).is_some());
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(
                [2, 15, 1],
                FluidCell {
                    amount: 8,
                    source: false,
                    flow: [1, 0],
                },
            )
            .unwrap();
        assert_eq!(app.world().resource::<EditJournal>().fluid_epoch(upper), 0);
        {
            let mut source = app.world_mut().resource_mut::<FarFluidSections>();
            source.revision += 1;
            let surface = source.sections.get_mut(&upper).unwrap();
            surface.epoch += 1;
            surface.published_water = Some(patch(3.));
        }
        app.update();
        let uploaded = app.world().resource::<Cache>().0[&upper]
            .water
            .as_ref()
            .unwrap()
            .1
            .clone();
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&uploaded)
            .unwrap();
        let Some(VertexAttributeValues::Float32x3(target)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("published positions missing")
        };
        let Some(VertexAttributeValues::Float32x4(from)) =
            mesh.attribute(crate::liquid_motion::FROM)
        else {
            panic!("upper-owner flow has no interpolation")
        };
        let Some(VertexAttributeValues::Float32x4(data)) =
            mesh.attribute(crate::liquid_motion::DATA)
        else {
            panic!("upper-owner flow has no motion periods")
        };
        assert_eq!(target, &patch(3.).positions);
        assert!(target.iter().zip(from).any(|(to, start)| {
            Vec3::from_array(*to).distance(Vec3::new(start[0], start[1], start[2])) > 0.1
        }));
        assert!(data.iter().all(|motion| motion[0] > 0.));
        assert_eq!(
            uploaded, original,
            "animated water keeps the same GPU asset ID"
        );
    }
    #[test]
    fn prepared_body_keeps_exact_topology_depth_current_and_mesh_when_focus_moves() {
        use bevy::mesh::VertexAttributeValues;
        use terra_world::water::WaterPatch;
        let position = SectionPos::new(-2, 0, 1);
        let patch = WaterPatch {
            positions: vec![
                [0., 7.25, 0.],
                [0., 7.25, 16.],
                [16., 7.25, 16.],
                [16., 7.25, 0.],
            ],
            normals: vec![[0., 1., 0.]; 4],
            flow: vec![[0.3, -0.2]; 4],
            depth_shore: vec![[6., 0.25]; 4],
            surface_effects: vec![[0.; 4]; 4],
            wave_weights: vec![0.5; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let mesh = published_water_mesh(&patch, [-32, 0, 16], None);
        assert_eq!(
            mesh.count_vertices(),
            4,
            "Use the worker geometry, not a dense volume remesh"
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            Some(&VertexAttributeValues::Float32x3(patch.positions.clone()))
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
            Some(&VertexAttributeValues::Float32x2(patch.depth_shore.clone()))
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_1),
            Some(&VertexAttributeValues::Float32x2(patch.flow.clone()))
        );
        let mut source = FarFluidSections {
            revision: 1,
            ..Default::default()
        };
        source.sections.insert(
            position,
            FarFluidSection {
                epoch: 7,
                fields: std::array::from_fn(|_| Vec::new()),
                films: Vec::new(),
                published_water: Some(patch),
            },
        );
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Cache>()
            .init_resource::<crate::water::WaterStatus>()
            .insert_resource(source)
            .insert_resource(super::super::surface::ParkedMaterials(std::array::from_fn(
                |_| Handle::default(),
            )))
            .insert_resource(crate::water::material::WaterAssets(Handle::default()))
            .insert_resource(terra_world::streaming::ChunkFocus([0.; 3]))
            .add_systems(Update, upload);
        app.update();
        let entity = app.world().resource::<Cache>().0[&position]
            .water
            .as_ref()
            .unwrap()
            .0;
        assert!(
            app.world()
                .get::<MeshMaterial3d<crate::water::material::WaterMaterial>>(entity)
                .is_some()
        );
        for i in 0..32 {
            app.world_mut()
                .resource_mut::<terra_world::streaming::ChunkFocus>()
                .0 = [i as f64 * 4., 48., -32.];
            app.update();
            assert_eq!(
                app.world().resource::<Cache>().0[&position]
                    .water
                    .as_ref()
                    .unwrap()
                    .0,
                entity
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
            assert_eq!(
                app.world()
                    .resource::<crate::water::WaterStatus>()
                    .triangles,
                2
            );
        }
    }

    #[test]
    fn unchanged_epochs_keep_entities_and_meshes_and_retired_sections_release_both() {
        let position = SectionPos { x: 0, y: 0, z: 0 };
        let mut field = vec![-1.; N * N * N];
        for z in 0..N {
            for y in 0..N {
                for x in 0..N {
                    field[index(x, y, z)] = (7.375 - y as f32).clamp(-1., 1.);
                }
            }
        }
        let mut source = FarFluidSections {
            revision: 1,
            ..default()
        };
        source.sections.insert(
            position,
            FarFluidSection {
                epoch: 1,
                fields: [field, vec![], vec![], vec![]],
                films: vec![],
                published_water: None,
            },
        );
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Cache>()
            .init_resource::<crate::water::WaterStatus>()
            .insert_resource(source)
            .insert_resource(super::super::surface::ParkedMaterials(std::array::from_fn(
                |_| Handle::default(),
            )))
            .add_systems(Update, upload);
        app.update();
        let original = app.world().resource::<Cache>().0[&position].reconstructed[0].0;
        let handle = app.world().resource::<Cache>().0[&position].reconstructed[0]
            .1
            .clone();
        for _ in 0..32 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Cache>().0[&position].reconstructed[0].0,
            original
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        assert!(app.world().resource::<crate::water::WaterStatus>().ready);
        app.world_mut()
            .resource_mut::<FarFluidSections>()
            .sections
            .get_mut(&position)
            .unwrap()
            .epoch += 1;
        app.update();
        assert_ne!(
            app.world().resource::<Cache>().0[&position].reconstructed[0].0,
            original
        );
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&handle)
                .is_none()
        );
        assert!(app.world().get_entity(original).is_err());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        app.world_mut()
            .resource_mut::<FarFluidSections>()
            .sections
            .clear();
        app.update();
        assert!(app.world().resource::<Cache>().0.is_empty());
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
        assert_eq!(
            app.world()
                .resource::<crate::water::WaterStatus>()
                .visible_tiles,
            0
        );
    }
    #[test]
    fn calm_volume_reconstructs_one_still_level_with_outward_normals_and_inward_winding() {
        let mut field = vec![0.; N * N * N];
        for z in 0..N {
            for y in 0..N {
                for x in 0..N {
                    field[index(x, y, z)] = 7.375 - y as f32;
                }
            }
        }
        let mesh = reconstruct(&field);
        assert!(!mesh.indices.is_empty());
        assert!(mesh.positions.iter().all(|p| (p[1] - 7.375).abs() < 1e-5));
        assert!(mesh.normals.iter().all(|n| n[1] > 0.99));
        for ids in mesh.indices.chunks_exact(3) {
            let a = Vec3::from_array(mesh.positions[ids[0] as usize]);
            let b = Vec3::from_array(mesh.positions[ids[1] as usize]);
            let c = Vec3::from_array(mesh.positions[ids[2] as usize]);
            assert!((b - a).cross(c - a).y < 0.);
        }
    }
    #[test]
    fn adjacent_owners_share_the_same_zero_crossing_at_negative_world_boundaries() {
        let make = |offset: f32| {
            let mut f = vec![0.; N * N * N];
            for z in 0..N {
                for y in 0..N {
                    for x in 0..N {
                        f[index(x, y, z)] = 4.3 + 0.2 * (x as f32 + offset) - y as f32;
                    }
                }
            }
            reconstruct(&f)
        };
        let a = make(-16.);
        let b = make(0.);
        let mut left: Vec<_> = a
            .positions
            .iter()
            .filter(|p| p[0] == 16.)
            .map(|p| (p[1] * 10000.).round() as i32)
            .collect();
        let mut right: Vec<_> = b
            .positions
            .iter()
            .filter(|p| p[0] == 0.)
            .map(|p| (p[1] * 10000.).round() as i32)
            .collect();
        left.sort_unstable();
        left.dedup();
        right.sort_unstable();
        right.dedup();
        assert!(!left.is_empty());
        assert_eq!(left, right);
    }
    #[test]
    fn a_thin_tar_film_is_preserved_without_a_full_cube_or_duplicate_wave_geometry() {
        let mut mesh = Geometry::default();
        mesh.film([1., 2.9375, 3.], [2., 2.95, 4.]);
        assert_eq!(mesh.indices.len(), 36);
        assert!(
            mesh.positions
                .iter()
                .all(|p| (2.9375..=2.95).contains(&p[1]))
        );
        assert!(reconstruct(&vec![-1.; N * N * N]).indices.is_empty());
    }
}
