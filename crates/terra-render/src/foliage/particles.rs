use super::FoliageStatus;
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    ecs::system::SystemParam,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_player::{PlayerBody, TerraCamera};
use terra_world::streaming::{ChunkStore, WorldTerrain};

const CALM_LEAVES: usize = 48;
const MAX_LEAVES: usize = 144;
const RADIUS: f32 = 18.0;
const STEP: f32 = 1.0 / 30.0;

#[derive(Clone, Debug)]
struct Leaf {
    position: Vec3,
    age: f32,
    phase: f32,
    size: f32,
    color: [f32; 4],
}

#[derive(Resource, Default)]
struct FallingLeaves {
    leaves: Vec<Leaf>,
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    anchor: Option<Vec3>,
    observer: Option<Entity>,
    accumulator: f32,
    spawn_timer: f32,
    sequence: u64,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<FallingLeaves>().add_systems(
        Update,
        update
            .after(terra_player::PlayerSet::Movement)
            .after(crate::weather::WeatherSet::Local),
    );
}

#[derive(SystemParam)]
struct LeafWorld<'w> {
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, terra_world::edits::EditJournal>,
    terrain: Res<'w, WorldTerrain>,
    time: Res<'w, Time>,
    blocks: Res<'w, crate::blocks::BlockAssets>,
    weather: Option<Res<'w, crate::weather::WeatherVisual>>,
    clock: Option<Res<'w, terra_core::WorldClock>>,
}

#[derive(SystemParam)]
struct LeafAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

type LeafObservers<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, Has<PlayerBody>),
    Or<(With<PlayerBody>, With<TerraCamera>)>,
>;

fn update(
    mut commands: Commands,
    world: LeafWorld,
    observers: LeafObservers,
    mut state: ResMut<FallingLeaves>,
    mut status: ResMut<FoliageStatus>,
    assets: LeafAssets,
) {
    let LeafAssets {
        mut meshes,
        mut materials,
    } = assets;
    let observer = observers
        .iter()
        .find(|(_, _, body)| *body)
        .or_else(|| observers.iter().next());
    let Some((entity, transform, _)) = observer.filter(|_| !world.chunks.loaded.is_empty()) else {
        clear(&mut state, &mut commands, &mut meshes, &mut materials);
        status.falling_leaves = 0;
        status.particle_batches = 0;
        return;
    };
    let center = transform.translation;
    if world.terrain.is_changed()
        || state.observer != Some(entity)
        || state
            .anchor
            .is_some_and(|anchor| anchor.distance_squared(center) > 32.0 * 32.0)
    {
        clear(&mut state, &mut commands, &mut meshes, &mut materials);
        state.observer = Some(entity);
    }
    state.accumulator = (state.accumulator + world.time.delta_secs()).min(STEP * 4.0);
    if state.accumulator < STEP {
        return;
    }
    let dt = state.accumulator;
    state.accumulator = 0.0;
    let wind = world.weather.as_ref().map_or(Vec2::ZERO, |w| w.wind);
    state
        .leaves
        .retain_mut(|leaf| advance_in_wind(leaf, dt, center, &world.chunks, wind));
    let season = world
        .clock
        .as_deref()
        .copied()
        .unwrap_or_default()
        .season(world.terrain.0.config().seasons);
    let (mut interval, maximum) = emission_in_wind(wind.length());
    if season.enabled {
        let shedding = season
            .weights
            .iter()
            .zip([0.25, 0.35, 2.5, 0.08])
            .map(|(w, rate)| w * rate)
            .sum::<f32>();
        interval /= shedding.max(0.05);
    }
    state.spawn_timer += dt;
    // The storm allowance remains one local batch and a fixed source-probe
    // budget. A stalled frame never produces an unbounded catch-up burst.
    let attempts = ((state.spawn_timer / interval) as usize).min(3);
    state.spawn_timer = state.spawn_timer.min(interval * 3.0);
    for _ in 0..attempts {
        if state.leaves.len() >= maximum {
            break;
        }
        state.spawn_timer -= interval;
        let sequence = state.sequence;
        state.sequence = sequence.wrapping_add(1);
        if let Some((position, kind)) = find_source(&world.chunks, center, sequence) {
            let source = (position + Vec3::Y * 0.2).floor().as_i64vec3().to_array();
            let section = terra_voxel::global_to_section(source).0;
            let Ok(origin) = section.origin() else {
                continue;
            };
            let appearance = world.terrain.0.tree_appearance(origin).at(source, kind);
            if world.journal.block(source).is_some()
                || !matches!(
                    appearance,
                    Some(
                        terra_world::generation::TreeAppearance::Oak
                            | terra_world::generation::TreeAppearance::Autumn
                            | terra_world::generation::TreeAppearance::Birch
                    )
                )
                || season.leaf_loss() >= 0.999
            {
                continue;
            }
            let color = world
                .blocks
                .templates
                .as_ref()
                .and_then(|t| t.blocks.get(&kind))
                .and_then(|b| world.blocks.materials.get(b.face_materials[0]))
                .and_then(|h| materials.get(h))
                .map(|m| m.base_color.to_linear().to_f32_array());
            if let Some(mut color) = color {
                for (channel, tint) in color[..3].iter_mut().zip(season.leaf_tint()) {
                    *channel *= tint;
                }
                state.leaves.push(Leaf {
                    position,
                    age: 0.0,
                    phase: random(sequence ^ 0x6d4a) * std::f32::consts::TAU,
                    size: 0.07 + random(sequence ^ 0xa791) * 0.04,
                    color,
                });
                status.spawned_leaves += 1;
            }
        }
    }
    status.falling_leaves = state.leaves.len();
    status.particle_batches = usize::from(!state.leaves.is_empty());
    if state.leaves.is_empty() {
        release_batch(&mut state, &mut commands, &mut meshes, &mut materials);
        return;
    }
    let anchor = *state.anchor.get_or_insert(center);
    let mesh = build_mesh(&state.leaves, anchor);
    if let Some(handle) = &state.mesh {
        if let Err(error) = meshes.insert(handle.id(), mesh) {
            tracing::warn!(%error, "Cannot update falling leaf batch");
        }
    } else {
        let mesh = meshes.add(mesh);
        let material = materials.add(StandardMaterial {
            perceptual_roughness: 0.92,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        state.entity = Some(
            commands
                .spawn((
                    Name::new("Local falling leaves"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(anchor),
                    // Camera movement is bounded to 32m before reset, particles to 18m.
                    Aabb::from_min_max(Vec3::splat(-52.0), Vec3::splat(52.0)),
                    NotShadowCaster,
                ))
                .id(),
        );
        state.mesh = Some(mesh);
        state.material = Some(material);
    }
}

fn emission_in_wind(speed: f32) -> (f32, usize) {
    let storm = ((speed - 2.2) / 4.4).clamp(0.0, 1.0);
    (
        0.32 + (0.055 - 0.32) * storm,
        CALM_LEAVES + ((MAX_LEAVES - CALM_LEAVES) as f32 * storm).round() as usize,
    )
}

fn release_batch(
    state: &mut FallingLeaves,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if let Some(entity) = state.entity.take() {
        commands.entity(entity).despawn();
    }
    if let Some(mesh) = state.mesh.take() {
        meshes.remove(mesh.id());
    }
    if let Some(material) = state.material.take() {
        materials.remove(material.id());
    }
}

fn clear(
    state: &mut FallingLeaves,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    release_batch(state, commands, meshes, materials);
    *state = FallingLeaves::default();
}

fn random(mut seed: u64) -> f32 {
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((seed ^ (seed >> 31)) >> 40) as f32 / 16_777_216.0
}

fn find_source(chunks: &ChunkStore, center: Vec3, sequence: u64) -> Option<(Vec3, u16)> {
    // Fixed budget: eight local columns, at most eighteen loaded-voxel probes
    // each. Never ask the procedural generator to manufacture foliage sources.
    for attempt in 0..8 {
        let seed = sequence.wrapping_mul(17).wrapping_add(attempt);
        let x = (center.x + (random(seed ^ 0xc513) - 0.5) * 24.0).floor() as i64;
        let z = (center.z + (random(seed ^ 0xf723) - 0.5) * 24.0).floor() as i64;
        let bottom = center.y.floor() as i64;
        for y in (bottom + 1..=bottom + 18).rev() {
            if let Some(kind @ 7..=9) = chunks.block_at([x, y, z])
                && chunks.block_at([x, y - 1, z]) == Some(0)
            {
                let position = Vec3::new(
                    x as f32 + 0.25 + random(seed ^ 0x1429) * 0.5,
                    y as f32 - 0.15,
                    z as f32 + 0.25 + random(seed ^ 0xa7de) * 0.5,
                );
                if clear_at(chunks, position, 0.11) {
                    return Some((position, kind));
                }
            }
        }
    }
    None
}

fn clear_at(chunks: &ChunkStore, p: Vec3, radius: f32) -> bool {
    let minimum = (p - Vec3::splat(radius)).floor().as_i64vec3();
    let maximum = (p + Vec3::splat(radius)).floor().as_i64vec3();
    for z in minimum.z..=maximum.z {
        for y in minimum.y..=maximum.y {
            for x in minimum.x..=maximum.x {
                if chunks.block_at([x, y, z]) != Some(0) {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
fn advance(leaf: &mut Leaf, dt: f32, center: Vec3, chunks: &ChunkStore) -> bool {
    advance_in_wind(leaf, dt, center, chunks, Vec2::ZERO)
}

fn advance_in_wind(
    leaf: &mut Leaf,
    dt: f32,
    center: Vec3,
    chunks: &ChunkStore,
    wind: Vec2,
) -> bool {
    leaf.age += dt;
    if leaf.age >= 12.0 || leaf.position.distance_squared(center) > RADIUS * RADIUS {
        return false;
    }
    let phase = leaf.phase + leaf.age * 1.8;
    let next = leaf.position
        + Vec3::new(
            0.18 + phase.sin() * 0.26 + wind.x * 0.25,
            -0.48 - phase.cos().abs() * 0.13,
            phase.cos() * 0.22 + wind.y * 0.25,
        ) * dt;
    // Swept samples include the leaf's extent. A roof or an unloaded section is
    // a stop, not a reason to render through missing world data.
    let steps = ((next - leaf.position).length() / 0.12).ceil().max(1.0) as usize;
    for step in 1..=steps {
        if !clear_at(
            chunks,
            leaf.position.lerp(next, step as f32 / steps as f32),
            leaf.size,
        ) {
            return false;
        }
    }
    leaf.position = next;
    true
}

fn build_mesh(leaves: &[Leaf], anchor: Vec3) -> Mesh {
    let count = leaves.len().min(MAX_LEAVES);
    let mut positions = Vec::with_capacity(count * 7);
    let mut normals = Vec::with_capacity(count * 7);
    let mut colors = Vec::with_capacity(count * 7);
    let mut uvs = Vec::with_capacity(count * 7);
    let mut indices = Vec::with_capacity(count * 18);
    for leaf in leaves.iter().take(MAX_LEAVES) {
        let rotation = Quat::from_euler(
            EulerRot::YXZ,
            leaf.phase + leaf.age * 0.7,
            (leaf.phase + leaf.age * 2.6).sin() * 0.65,
            (leaf.phase - leaf.age * 1.9).cos() * 0.35,
        );
        let normal = (rotation * Vec3::Y).to_array();
        let scale =
            leaf.size * (leaf.age / 0.25).min(1.0) * ((12.0 - leaf.age) / 1.2).clamp(0.0, 1.0);
        let start = positions.len() as u32;
        for p in [
            [0.0, 0.0],
            [-0.42, -0.55],
            [0.0, -1.0],
            [0.42, -0.55],
            [0.42, 0.55],
            [0.0, 1.0],
            [-0.42, 0.55],
        ] {
            let vertex = leaf.position - anchor + rotation * Vec3::new(p[0], 0.0, p[1]) * scale;
            positions.push(vertex.to_array());
            normals.push(normal);
            colors.push(leaf.color);
            uvs.push(p);
        }
        for side in 0..6 {
            indices.extend_from_slice(&[start, start + (side + 1) % 6 + 1, start + side + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, SectionPos};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn storms_shed_more_foliage_without_removing_the_single_batch_budget() {
        assert_eq!(emission_in_wind(0.0), (0.32, CALM_LEAVES));
        let (interval, maximum) = emission_in_wind(7.8);
        assert!(interval < 0.06);
        assert_eq!(maximum, MAX_LEAVES);
        assert_eq!(emission_in_wind(100.0), (interval, maximum));
        assert!(emission_in_wind(4.0).0 > interval);
    }

    fn canopy(kind: u16) -> ChunkStore {
        let mut chunks = ChunkStore::default();
        for x in -1..=0 {
            for z in -1..=0 {
                let p = SectionPos::new(x, 0, z);
                let mut data = BlockSection::empty(p).unwrap();
                for x in 0..16 {
                    for z in 0..16 {
                        data.set_block([x, 10, z], kind).unwrap();
                    }
                }
                chunks.loaded.insert(
                    p,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
        chunks
    }

    #[test]
    fn only_existing_loaded_foliage_emits_leaves_and_all_variants_survive() {
        for kind in 7..=9 {
            let chunks = canopy(kind);
            let source = find_source(&chunks, Vec3::new(0.0, 2.0, 0.0), 0).unwrap();
            assert_eq!(source.1, kind);
            assert_eq!(source.0.y, 9.85);
            assert_eq!(
                find_source(&chunks, Vec3::new(0.0, 2.0, 0.0), 0),
                Some(source)
            );
        }
        assert!(find_source(&canopy(3), Vec3::ZERO, 0).is_none());
        assert!(find_source(&ChunkStore::default(), Vec3::ZERO, 0).is_none());
    }

    #[test]
    fn leaves_stop_at_roofs_ground_and_missing_sections() {
        let chunks = canopy(3);
        let mut leaf = Leaf {
            position: Vec3::new(0.5, 11.12, 0.5),
            age: 0.0,
            phase: 0.0,
            size: 0.09,
            color: [1.0; 4],
        };
        assert!(!advance(&mut leaf, 0.3, Vec3::new(0.0, 11.0, 0.0), &chunks));
        leaf.position = Vec3::new(0.5, 9.5, 0.5);
        assert!(advance(&mut leaf, 0.1, Vec3::new(0.0, 9.0, 0.0), &chunks));
        assert!(!advance(
            &mut leaf,
            0.1,
            Vec3::new(0.0, 9.0, 0.0),
            &ChunkStore::default()
        ));
    }

    #[test]
    fn one_bounded_batch_has_valid_indices_normals_and_winding() {
        let leaf = Leaf {
            position: Vec3::ZERO,
            age: 2.0,
            phase: 0.0,
            size: 0.1,
            color: [1.0; 4],
        };
        let mesh = build_mesh(&vec![leaf; MAX_LEAVES + 4], Vec3::ZERO);
        assert_eq!(mesh.count_vertices(), MAX_LEAVES * 7);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals");
        };
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        assert_eq!(indices.len(), MAX_LEAVES * 18);
        for triangle in indices.chunks_exact(3) {
            let [a, b, c] = std::array::from_fn(|i| Vec3::from_array(positions[triangle[i]]));
            assert!(
                (b - a)
                    .cross(c - a)
                    .dot(Vec3::from_array(normals[triangle[0]]))
                    > 0.0
            );
        }
    }

    #[test]
    fn empty_session_removes_particles_entity_mesh_and_material() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<ChunkStore>()
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<crate::blocks::BlockAssets>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<FoliageStatus>()
            .insert_resource(WorldTerrain(
                terra_world::TerraGenerator::new(Default::default()).unwrap(),
            ))
            .add_systems(Update, update);
        let leaf = Leaf {
            position: Vec3::ZERO,
            age: 2.0,
            phase: 0.0,
            size: 0.1,
            color: [1.0; 4],
        };
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(build_mesh(std::slice::from_ref(&leaf), Vec3::ZERO));
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let entity = app
            .world_mut()
            .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())))
            .id();
        app.insert_resource(FallingLeaves {
            leaves: vec![leaf],
            entity: Some(entity),
            mesh: Some(mesh.clone()),
            material: Some(material.clone()),
            ..default()
        });
        app.update();
        assert!(app.world().get_entity(entity).is_err());
        assert!(!app.world().resource::<Assets<Mesh>>().contains(mesh.id()));
        assert!(
            !app.world()
                .resource::<Assets<StandardMaterial>>()
                .contains(material.id())
        );
        assert!(app.world().resource::<FallingLeaves>().leaves.is_empty());
        assert_eq!(app.world().resource::<FoliageStatus>().particle_batches, 0);
    }
}
