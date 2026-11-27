//! One local luminous batch: world-anchored spirit wisps, spores and blood embers.
//! Cached columns select the palette; only existing loaded air may contain motes.
use super::FoliageStatus;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    ecs::system::SystemParam,
    image::{ImageSampler, ImageSamplerDescriptor},
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use terra_core::{BlockDetailQuality, GameConfig, WorldClock};
use terra_player::{PlayerBody, TerraCamera};
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};

const MAX_MOTES: usize = 112;
const RANGE: f32 = 17.0;
const STEP: f32 = 1.0 / 30.0;

#[derive(Resource, Default)]
struct State {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    image: Option<Handle<Image>>,
    accumulator: f32,
}

#[derive(Clone, Copy, Debug)]
struct Mote {
    position: Vec3,
    size: f32,
    color: [f32; 4],
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Update, update.after(terra_player::PlayerSet::Movement));
}

#[derive(SystemParam)]
struct Environment<'w, 's> {
    chunks: Res<'w, ChunkStore>,
    terrain: Res<'w, WorldTerrain>,
    journal: Res<'w, EditJournal>,
    access: Option<Res<'w, WorldAccess>>,
    config: Res<'w, GameConfig>,
    clock: Option<Res<'w, WorldClock>>,
    time: Res<'w, Time>,
    cameras: Query<'w, 's, &'static Transform, With<TerraCamera>>,
    bodies: Query<'w, 's, &'static Transform, With<PlayerBody>>,
}

#[derive(SystemParam)]
struct Assets<'w> {
    meshes: ResMut<'w, bevy::prelude::Assets<Mesh>>,
    materials: ResMut<'w, bevy::prelude::Assets<StandardMaterial>>,
    images: ResMut<'w, bevy::prelude::Assets<Image>>,
}

fn update(
    mut commands: Commands,
    world: Environment,
    mut assets: Assets,
    mut state: ResMut<State>,
    mut status: ResMut<FoliageStatus>,
) {
    let camera = world.cameras.iter().next();
    let center = world.bodies.iter().next().or(camera).map(|t| t.translation);
    let ready = !world.chunks.loaded.is_empty()
        && world.access.as_deref().is_none_or(|a| a.ready())
        && center.is_some_and(Vec3::is_finite)
        && camera.is_some();
    if !ready {
        clear(&mut state, &mut commands, &mut assets);
        status.magical_motes = 0;
        status.magical_batches = 0;
        return;
    }
    state.accumulator = (state.accumulator + world.time.delta_secs()).min(STEP * 3.0);
    if state.accumulator < STEP {
        return;
    }
    state.accumulator = 0.0;
    let camera = camera.unwrap();
    let center = center.unwrap();
    let budget = budget(world.config.client.blocks.leaf_geometry_quality);
    let phase = world.clock.as_deref().map_or(0.5, |c| c.phase as f32);
    let air = Air {
        chunks: &world.chunks,
        terrain: &world.terrain.0,
        journal: &world.journal,
    };
    let motes = gather(&air, center, world.time.elapsed_secs(), phase, budget);
    status.magical_motes = motes.len();
    status.magical_batches = usize::from(!motes.is_empty());
    if motes.is_empty() {
        clear(&mut state, &mut commands, &mut assets);
        return;
    }
    let mesh = build_mesh(
        &motes,
        center,
        camera.rotation * Vec3::X,
        camera.rotation * Vec3::Y,
    );
    if let Some(handle) = &state.mesh {
        if let Err(error) = assets.meshes.insert(handle.id(), mesh) {
            tracing::warn!(%error, "Cannot update biome mote batch");
        }
        if let Some(entity) = state.entity {
            commands
                .entity(entity)
                .insert(Transform::from_translation(center));
        }
        return;
    }
    let image = assets.images.add(glow_texture());
    let material = assets.materials.add(StandardMaterial {
        base_color: Color::linear_rgba(2.2, 2.2, 2.2, 1.0),
        base_color_texture: Some(image.clone()),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        fog_enabled: true,
        ..default()
    });
    let mesh = assets.meshes.add(mesh);
    state.entity = Some(
        commands
            .spawn((
                Name::new("Local biome wisps and spores"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(center),
                NoFrustumCulling,
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .id(),
    );
    state.mesh = Some(mesh);
    state.material = Some(material);
    state.image = Some(image);
}

fn budget(quality: BlockDetailQuality) -> usize {
    match quality {
        BlockDetailQuality::Low => 48,
        BlockDetailQuality::Medium => 80,
        BlockDetailQuality::High => MAX_MOTES,
    }
}

fn clear(state: &mut State, commands: &mut Commands, assets: &mut Assets) {
    if let Some(entity) = state.entity.take() {
        commands.entity(entity).despawn();
    }
    if let Some(mesh) = state.mesh.take() {
        assets.meshes.remove(mesh.id());
    }
    if let Some(material) = state.material.take() {
        assets.materials.remove(material.id());
    }
    if let Some(image) = state.image.take() {
        assets.images.remove(image.id());
    }
}

fn random(mut seed: u64) -> f32 {
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((seed ^ (seed >> 31)) >> 40) as f32 / 16_777_216.0
}

/// Weighted color and density avoid a dominant-biome switch at a region border.
fn palette(weights: [f32; 4], phase: f32, variation: f32) -> ([f32; 3], f32) {
    let night = (0.5 + (phase * std::f32::consts::TAU).cos() * 1.2).clamp(0.0, 1.0);
    let density = weights[0] * 0.95
        + weights[1] * (0.48 + night * 0.40)
        + weights[2] * (0.24 + 0.16 * night)
        + weights[3] * 0.28;
    let palettes = [
        Vec3::new(0.16, 0.91, 1.0).lerp(Vec3::new(0.84, 0.69, 1.0), variation),
        Vec3::new(0.80, 0.60, 0.98).lerp(Vec3::new(1.0, 0.78, 0.34), variation),
        Vec3::new(0.93, 0.07, 0.13).lerp(Vec3::new(1.0, 0.35, 0.18), variation),
        Vec3::new(1.0, 0.53, 0.76).lerp(Vec3::new(1.0, 0.85, 0.42), variation),
    ];
    let sum = weights.iter().sum::<f32>().max(0.001);
    let color = palettes
        .into_iter()
        .zip(weights)
        .map(|(color, weight)| color * weight)
        .sum::<Vec3>()
        / sum;
    (color.to_array(), density.min(1.0))
}

struct Air<'a> {
    chunks: &'a ChunkStore,
    terrain: &'a TerraGenerator,
    journal: &'a EditJournal,
}

fn gather(air: &Air, center: Vec3, time: f32, phase: f32, limit: usize) -> Vec<Mote> {
    let chunks = air.chunks;
    let mut motes = Vec::with_capacity(limit);
    let Some(column) = chunks.surface_column_at(center.x.floor() as i64, center.z.floor() as i64)
    else {
        return motes;
    };
    if center.y < column.height as f32 - 1.0 || !clear_air(air, center, 0.1) {
        return motes;
    }
    let tile = (center / 5.0).floor().as_i64vec3();
    for z in -3..=3 {
        for x in -3..=3 {
            let gx = tile.x + x;
            let gz = tile.z + z;
            let seed = (gx as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ (gz as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            let px = gx as f32 * 5.0 + 0.8 + random(seed ^ 0x761d) * 3.4;
            let pz = gz as f32 * 5.0 + 0.8 + random(seed ^ 0x2c91) * 3.4;
            let Some(c) = chunks.surface_column_at(px.floor() as i64, pz.floor() as i64) else {
                continue;
            };
            let ground = (c.height as f32 - 0.5).floor();
            let pool = c.water_level.filter(|y| *y as f32 > ground + 0.5);
            if pool.is_some() && c.biome.weights.spirit_blossom < 0.55 {
                continue;
            }
            // Some wisps skim open pools. Anchor them to the actual water plane.
            let ground = pool.map_or(ground, |level| level.ceil() as f32);
            if c.cave.open_to_sky() || (ground - center.y).abs() > 12.0 {
                continue;
            }
            // Existing roofs and missing chunks stop the local emitter. Foliage
            // is allowed above wisps but never a solid building ceiling.
            if !(1..=6).all(|y| {
                chunks
                    .block_at([px.floor() as i64, ground as i64 + y, pz.floor() as i64])
                    .is_some_and(|m| m == 0 || terra_voxel::MaterialId::FOLIAGE.contains(&m))
            }) {
                continue;
            }
            let w = c.biome.weights;
            let (color, strength) = palette(
                [
                    w.spirit_blossom as f32,
                    w.mushroom_park as f32,
                    w.blood_forest as f32,
                    w.cherry_forest as f32,
                ],
                phase,
                random(seed ^ 0x1fc9),
            );
            if strength <= 0.01 {
                continue;
            }
            for copy in 0..3 {
                let n = seed ^ (copy as u64 + 1).wrapping_mul(0x94d0_49bb_1331_11eb);
                let age = (time * (0.055 + random(n) * 0.025) + random(n ^ 0x67d1)).fract();
                let angle = random(n ^ 0x2b83) * std::f32::consts::TAU + age * 5.4;
                let radius = 0.40 + random(n ^ 0xaa93) * 0.65;
                let position = Vec3::new(
                    px + angle.cos() * radius,
                    ground
                        + 1.10
                        + age * (if pool.is_some() { 1.2 } else { 3.2 })
                        + angle.sin() * 0.20,
                    pz + angle.sin() * radius,
                );
                let fade = edge_fade(position.distance(center))
                    * (age / 0.16).min(1.0)
                    * ((1.0 - age) / 0.24).min(1.0);
                let pulse = 0.65 + 0.35 * (time * 1.3 + random(n ^ 0x437b) * 6.3).sin().powi(2);
                let alpha = fade * strength * pulse * 0.72;
                if alpha < 0.003 || !clear_air(air, position, 0.22) {
                    continue;
                }
                let size = 0.14 + random(n ^ 0x179f) * 0.10;
                motes.push(Mote {
                    position,
                    size,
                    color: [color[0], color[1], color[2], alpha],
                });
                // Widely spaced luminous heads leave two small fading sparks
                // along their path, using the same billboard and quality budget.
                if copy == 0 && w.spirit_blossom > 0.45 {
                    let backward = Vec3::new(angle.sin() * radius, -0.5, -angle.cos() * radius);
                    for (lag, scale) in [(0.20, 0.57), (0.36, 0.32)] {
                        let tail = position + lag * backward;
                        if clear_air(air, tail, size * scale) {
                            motes.push(Mote {
                                position: tail,
                                size: scale * size,
                                color: [
                                    color[0],
                                    color[1],
                                    color[2],
                                    alpha * scale * w.spirit_blossom as f32,
                                ],
                            });
                        }
                    }
                }
            }
        }
    }
    // Distance-first truncation keeps dense regions symmetric around the viewer.
    motes.sort_by(|a, b| {
        a.position
            .distance_squared(center)
            .total_cmp(&b.position.distance_squared(center))
    });
    motes.truncate(limit.min(MAX_MOTES));
    motes
}

fn edge_fade(distance: f32) -> f32 {
    let t = ((RANGE - distance) / 5.0).clamp(0.0, 1.0);
    let near = ((distance - 0.6) / 1.2).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t) * near
}

fn clear_air(air: &Air, position: Vec3, radius: f32) -> bool {
    let min = (position - Vec3::splat(radius)).floor().as_i64vec3();
    let max = (position + Vec3::splat(radius)).floor().as_i64vec3();
    for z in min.z..=max.z {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let cell = [x, y, z];
                if air.chunks.block_at(cell) != Some(0) {
                    return false;
                }
                // Live liquid can occupy a solid-air voxel. Never fall back to
                // procedural water/terrain noise when a prepared column is absent.
                if air.terrain.config().water {
                    if air.journal.fluid(cell).is_none()
                        && air.chunks.surface_column_at(x, z).is_none()
                    {
                        return false;
                    }
                    if air
                        .chunks
                        .fluid_cell_at(air.terrain, air.journal, cell, 0)
                        .amount
                        > 0
                    {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn glow_texture() -> Image {
    let mut pixels = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32 {
        for x in 0..32 {
            let r2 = (Vec2::new(x as f32, y as f32) / 15.5 - Vec2::ONE).length_squared();
            let halo = (1.0 - r2).max(0.0).powi(3) * 0.25;
            let core = (-r2 * 48.0).exp() * 0.75;
            pixels.extend([255, 255, 255, ((halo + core) * 255.0) as u8]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

fn build_mesh(motes: &[Mote], anchor: Vec3, right: Vec3, up: Vec3) -> Mesh {
    let count = motes.len().min(MAX_MOTES);
    let mut positions = Vec::with_capacity(count * 4);
    let mut normals = Vec::with_capacity(count * 4);
    let mut colors = Vec::with_capacity(count * 4);
    let mut uvs = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for mote in motes.iter().take(count) {
        let start = positions.len() as u32;
        for uv in [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y] {
            let local = uv * 2.0 - Vec2::ONE;
            let p = mote.position - anchor + (right * local.x + up * local.y) * mote.size;
            positions.push(p.to_array());
            normals.push(right.cross(up).to_array());
            colors.push(mote.color);
            uvs.push(uv.to_array());
        }
        indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
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
    fn fantasy_palettes_blend_continuously_and_ordinary_regions_stay_clear() {
        assert_eq!(palette([0.0; 4], 0.5, 0.5).1, 0.0);
        let spirit = palette([1.0, 0.0, 0.0, 0.0], 0.5, 0.0);
        let blood = palette([0.0, 0.0, 1.0, 0.0], 0.5, 0.0);
        assert!(spirit.0[2] > spirit.0[0]);
        assert!(blood.0[0] > blood.0[1] * 3.0);
        let mut previous = Vec3::from_array(spirit.0);
        for step in 1..=100 {
            let t = step as f32 / 100.0;
            let color = Vec3::from_array(palette([1.0 - t, 0.0, t, 0.0], 0.5, 0.0).0);
            assert!(color.distance(previous) < 0.02);
            previous = color;
        }
        assert!(
            palette([0.0, 1.0, 0.0, 0.0], 0.0, 0.0).1 > palette([0.0, 1.0, 0.0, 0.0], 0.5, 0.0).1
        );
    }

    #[test]
    fn solid_roofs_water_and_missing_chunks_block_motes() {
        let mut chunks = ChunkStore::default();
        let position = SectionPos::new(0, 0, 0);
        chunks.loaded.insert(
            position,
            LoadedSection {
                data: BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let p = Vec3::splat(2.5);
        let dry = TerraGenerator::new(terra_core::WorldConfig {
            water: false,
            ..default()
        })
        .unwrap();
        let mut journal = EditJournal::default();
        let clear = |chunks: &ChunkStore, terrain: &TerraGenerator, journal: &EditJournal, p| {
            clear_air(
                &Air {
                    chunks,
                    terrain,
                    journal,
                },
                p,
                0.2,
            )
        };
        assert!(clear(&chunks, &dry, &journal, p));
        for material in [3, 6] {
            chunks
                .loaded
                .get_mut(&position)
                .unwrap()
                .data
                .set_block([2, 2, 2], material)
                .unwrap();
            assert!(!clear(&chunks, &dry, &journal, p));
        }
        assert!(!clear(&chunks, &dry, &journal, Vec3::new(16.0, 2.5, 2.5)));
        chunks
            .loaded
            .get_mut(&position)
            .unwrap()
            .data
            .set_block([2, 2, 2], 0)
            .unwrap();
        let wet = TerraGenerator::new(Default::default()).unwrap();
        // Unprepared water data is unknown, so the visual probe must stop
        // instead of sampling the generator on the frame thread.
        assert!(!clear(&chunks, &wet, &journal, p));
        for amount in [1, 8] {
            journal
                .set_fluid(
                    [2, 2, 2],
                    terra_world::water::FluidCell {
                        amount,
                        source: false,
                        flow: [0, 0],
                    },
                )
                .unwrap();
            assert_eq!(chunks.block_at([2, 2, 2]), Some(0));
            assert!(
                !clear(&chunks, &wet, &journal, p),
                "live fluid amount {amount}"
            );
        }
        // An explicit air edit retains a dry tombstone; clearing a transient
        // fluid alone restores unknown natural water when no column is loaded.
        journal.record([2, 2, 2], 0).unwrap();
        journal.set_fluid([2, 2, 2], Default::default()).unwrap();
        assert!(
            clear(&chunks, &wet, &journal, p),
            "confirmed drained air is clear"
        );
        assert_eq!(edge_fade(RANGE), 0.0);
        assert_eq!(edge_fade(0.3), 0.0);
        assert_eq!(edge_fade(5.0), 1.0);
    }

    #[test]
    fn all_quality_budgets_share_a_single_valid_bounded_billboard_mesh() {
        assert!(budget(BlockDetailQuality::Low) < budget(BlockDetailQuality::High));
        let mote = Mote {
            position: Vec3::ONE,
            size: 0.2,
            color: [1.0; 4],
        };
        let mesh = build_mesh(&vec![mote; MAX_MOTES + 7], Vec3::ZERO, Vec3::X, Vec3::Y);
        assert_eq!(mesh.count_vertices(), MAX_MOTES * 4);
        assert_eq!(mesh.indices().unwrap().len(), MAX_MOTES * 6);
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .all(|index| index < mesh.count_vertices())
        );
        let image = glow_texture();
        let pixels = image.data.unwrap();
        assert_eq!(pixels.len(), 32 * 32 * 4);
        assert_eq!(pixels[3], 0);
        assert!(pixels[(15 * 32 + 15) * 4 + 3] > 220);
    }

    #[test]
    fn leaving_world_releases_batch_and_all_owned_gpu_assets() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .insert_resource(WorldTerrain(
                TerraGenerator::new(Default::default()).unwrap(),
            ))
            .init_resource::<bevy::prelude::Assets<Mesh>>()
            .init_resource::<bevy::prelude::Assets<StandardMaterial>>()
            .init_resource::<bevy::prelude::Assets<Image>>()
            .init_resource::<GameConfig>()
            .init_resource::<FoliageStatus>()
            .add_systems(Update, update);
        let mesh = app
            .world_mut()
            .resource_mut::<bevy::prelude::Assets<Mesh>>()
            .add(build_mesh(&[], Vec3::ZERO, Vec3::X, Vec3::Y));
        let material = app
            .world_mut()
            .resource_mut::<bevy::prelude::Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let image = app
            .world_mut()
            .resource_mut::<bevy::prelude::Assets<Image>>()
            .add(glow_texture());
        let entity = app.world_mut().spawn_empty().id();
        app.insert_resource(State {
            entity: Some(entity),
            mesh: Some(mesh.clone()),
            material: Some(material.clone()),
            image: Some(image.clone()),
            accumulator: 0.0,
        });
        app.update();
        assert!(app.world().get_entity(entity).is_err());
        assert!(
            !app.world()
                .resource::<bevy::prelude::Assets<Mesh>>()
                .contains(mesh.id())
        );
        assert!(
            !app.world()
                .resource::<bevy::prelude::Assets<StandardMaterial>>()
                .contains(material.id())
        );
        assert!(
            !app.world()
                .resource::<bevy::prelude::Assets<Image>>()
                .contains(image.id())
        );
        assert_eq!(app.world().resource::<FoliageStatus>().magical_batches, 0);
    }
}
