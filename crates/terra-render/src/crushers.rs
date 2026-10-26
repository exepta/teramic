//! Chunk-batched machine frames and shared articulated rollers; no tooth entities.
use crate::{
    blocks::{BlockAssets, TemplateSet},
    meshing::builder::MeshBatch,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use terra_network::client::ClientSession;
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::ChunkStore,
};

const MAX_DUST_MACHINES: usize = 16;
const DUST_PER_MACHINE: usize = 8;
const CENTER: Vec3 = Vec3::new(0.5, 0.0, 0.5);

pub(crate) fn rotor_material(name: &str) -> Option<u8> {
    if name.starts_with("crusher_roller_left") {
        Some(0)
    } else if name.starts_with("crusher_roller_right") {
        Some(1)
    } else if name.starts_with("crusher_cog") {
        Some(2)
    } else if name.starts_with("crank_rotating") {
        Some(3)
    } else {
        None
    }
}

fn pivot(rotor: u8) -> Vec3 {
    match rotor {
        0 => Vec3::new(0.545, 0.455, 0.515),
        1 => Vec3::new(1.455, 0.455, 0.515),
        2 => Vec3::new(1.96, 0.455, 0.515),
        _ => Vec3::new(0.22, 0.455, 0.515),
    }
}

#[derive(Clone)]
struct Part {
    rotor: u8,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct Machines {
    parts: Vec<Part>,
    placed: HashMap<[i64; 3], (Entity, u8)>,
    next_scan: f32,
    next_dust: f32,
    dust: Option<(Handle<Mesh>, Entity)>,
    sources: HashMap<terra_voxel::SectionPos, u64>,
    scratch_sources: HashMap<terra_voxel::SectionPos, u64>,
    candidates: Vec<([i64; 3], u16, BlockObjectState)>,
    connection: Option<Entity>,
}

#[derive(Component)]
struct Drive {
    position: [i64; 3],
    working: bool,
    manual_active: bool,
    crank: bool,
    speed: f32,
    phase: f32,
}

impl Drive {
    fn advance(&mut self, dt: f32) {
        let target = if self.working || self.manual_active {
            3.4
        } else {
            0.0
        };
        self.speed += (target - self.speed) * (1.0 - (-dt * 9.0).exp());
        if target == 0.0 && self.speed < 0.002 {
            self.speed = 0.0;
        }
        self.phase = (self.phase + self.speed * dt).rem_euclid(std::f32::consts::TAU);
    }
}

#[derive(Component)]
struct Rotor {
    root: Entity,
    index: u8,
}

#[derive(Resource, Default, Debug)]
pub struct CrusherRenderStatus {
    pub machines: usize,
    pub cranks: usize,
    pub working: usize,
    pub cached_meshes: usize,
    pub particles: usize,
    pub source_scans: u64,
    pub source_sections: usize,
}

fn refresh_candidates(state: &mut Machines, journal: &EditJournal, chunks: &ChunkStore) -> bool {
    state.scratch_sources.clear();
    for &section in chunks.loaded.keys() {
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (Some(x), Some(z)) = (section.x.checked_add(dx), section.z.checked_add(dz)) else {
                continue;
            };
            let source = terra_voxel::SectionPos::new(x, section.y, z);
            state
                .scratch_sources
                .insert(source, journal.mesh_epoch(source));
        }
    }
    if state.scratch_sources == state.sources {
        return false;
    }
    std::mem::swap(&mut state.sources, &mut state.scratch_sources);
    state.candidates.clear();
    let mut seen = HashSet::new();
    for &section in state.sources.keys() {
        for (position, material, object) in journal.object_states_in_section(section) {
            if matches!(
                material,
                terra_core::crusher::MATERIAL | terra_core::crusher::HAND_CRANK_MATERIAL
            ) && !object.secondary
                && seen.insert(position)
            {
                state.candidates.push((position, material, object));
            }
        }
    }
    true
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Machines>()
        .init_resource::<CrusherRenderStatus>()
        .add_systems(Update, (synchronize, animate, dust).chain());
}

fn shared_parts(
    templates: &TemplateSet,
    assets: &BlockAssets,
    meshes: &mut Assets<Mesh>,
) -> Option<Vec<Part>> {
    let models = [
        terra_core::crusher::MATERIAL,
        terra_core::crusher::HAND_CRANK_MATERIAL,
    ]
    .map(|id| templates.blocks.get(&id))
    .into_iter()
    .flatten();
    for primitive in models.clone().flat_map(|block| &block.model) {
        if templates
            .crusher_moving_materials
            .contains_key(&primitive.material)
            && assets.materials.get(primitive.material).is_none()
        {
            return None;
        }
    }
    let mut parts = Vec::new();
    for primitive in models.flat_map(|block| &block.model) {
        let Some(&rotor) = templates.crusher_moving_materials.get(&primitive.material) else {
            continue;
        };
        let origin = pivot(rotor);
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            primitive
                .positions
                .iter()
                .map(|p| {
                    (Vec3::new(p[0] * if rotor == 3 { 1.0 } else { 2.0 }, p[1], p[2]) - origin)
                        .to_array()
                })
                .collect::<Vec<_>>(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            primitive
                .normals
                .iter()
                .map(|n| {
                    (Vec3::from_array(*n) / Vec3::new(if rotor == 3 { 1.0 } else { 2.0 }, 1.0, 1.0))
                        .normalize_or_zero()
                        .to_array()
                })
                .collect::<Vec<_>>(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, primitive.uvs.clone())
        .with_inserted_indices(Indices::U32(primitive.indices.clone()));
        parts.push(Part {
            rotor,
            mesh: meshes.add(mesh),
            material: assets.materials.get(primitive.material)?.clone(),
        });
    }
    (!parts.is_empty()).then_some(parts)
}

fn loaded(
    chunks: &ChunkStore,
    position: [i64; 3],
    material: u16,
    object: BlockObjectState,
) -> bool {
    chunks.block_at(position) == Some(material)
        || material == terra_core::crusher::MATERIAL
            && object
                .peer
                .is_some_and(|peer| chunks.block_at(peer) == Some(terra_core::crusher::MATERIAL))
}

type Cameras<'w, 's> = Query<'w, 's, &'static Transform, With<terra_player::TerraCamera>>;

#[allow(clippy::too_many_arguments)]
fn synchronize(
    mut commands: Commands,
    time: Res<Time>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    assets: Res<BlockAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    session: Option<Res<ClientSession>>,
    mut state: ResMut<Machines>,
    mut drives: Query<&mut Drive>,
    mut status: ResMut<CrusherRenderStatus>,
) {
    let now = time.elapsed_secs();
    if now < state.next_scan {
        return;
    }
    state.next_scan = now + 0.1;
    let Some(templates) = &assets.templates else {
        return;
    };
    if state.parts.is_empty() {
        let Some(parts) = shared_parts(templates, &assets, &mut meshes) else {
            return;
        };
        status.cached_meshes = parts.len();
        state.parts = parts;
    }
    let connection = session.as_ref().and_then(|session| session.connection);
    if state.connection != connection {
        state.connection = connection;
        state.sources.clear();
        state.candidates.clear();
    }
    if refresh_candidates(&mut state, &journal, &chunks) {
        status.source_scans = status.source_scans.saturating_add(1);
        status.source_sections = state.sources.len();
    }
    let mut live = HashSet::new();
    for index in 0..state.candidates.len() {
        let (position, material, object) = state.candidates[index];
        if !loaded(&chunks, position, material, object) {
            continue;
        }
        live.insert(position);
        let crank = material == terra_core::crusher::HAND_CRANK_MATERIAL;
        let owner = if crank {
            object.peer.unwrap_or(position)
        } else {
            position
        };
        let visual = session.as_ref().and_then(|session| {
            session
                .crushers
                .iter()
                .find(|visual| visual.position == owner)
        });
        let working = visual.is_some_and(|visual| visual.working);
        let manual_active = visual.is_some_and(|visual| visual.manual_active);
        if let Some(&(entity, facing)) = state.placed.get(&position) {
            if facing == object.facing {
                if let Ok(mut drive) = drives.get_mut(entity) {
                    drive.working = working;
                    drive.manual_active = manual_active;
                }
                continue;
            }
            commands.entity(entity).try_despawn();
            state.placed.remove(&position);
        }
        let root = commands
            .spawn((
                Name::new(if crank {
                    "Hand crank moving assembly"
                } else {
                    "Block Crusher moving assembly"
                }),
                Drive {
                    position: owner,
                    working,
                    manual_active,
                    crank,
                    speed: 0.0,
                    phase: 0.0,
                },
                Transform::from_translation(Vec3::from_array(position.map(|v| v as f32)) + CENTER)
                    .with_rotation(Quat::from_rotation_y(
                        f32::from(object.facing) * std::f32::consts::FRAC_PI_2,
                    )),
                Visibility::Inherited,
            ))
            .id();
        for index in if crank { 3..4 } else { 0..3 } {
            let rotor = commands
                .spawn((
                    Name::new(format!(
                        "{} rotor {index}",
                        if crank { "Hand crank" } else { "Crusher" }
                    )),
                    Rotor { root, index },
                    Transform::from_translation(pivot(index) - CENTER),
                    Visibility::Inherited,
                    ChildOf(root),
                ))
                .id();
            for part in state.parts.iter().filter(|part| part.rotor == index) {
                commands.spawn((
                    Mesh3d(part.mesh.clone()),
                    MeshMaterial3d(part.material.clone()),
                    Transform::default(),
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Terrain,
                    ChildOf(rotor),
                ));
            }
        }
        state.placed.insert(position, (root, object.facing));
    }
    state.placed.retain(|position, (entity, _)| {
        if live.contains(position) {
            true
        } else {
            commands.entity(*entity).try_despawn();
            false
        }
    });
    status.cranks = state
        .candidates
        .iter()
        .filter(|(position, material, _)| {
            *material == terra_core::crusher::HAND_CRANK_MATERIAL
                && state.placed.contains_key(position)
        })
        .count();
    status.machines = state.placed.len() - status.cranks;
}

fn animate(
    time: Res<Time>,
    mut drives: Query<&mut Drive>,
    mut rotors: Query<(&Rotor, &mut Transform)>,
    mut status: ResMut<CrusherRenderStatus>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let mut working = 0;
    for mut drive in &mut drives {
        drive.advance(dt);
        working += usize::from(drive.working && !drive.crank);
    }
    for (rotor, mut transform) in &mut rotors {
        let Ok(drive) = drives.get(rotor.root) else {
            continue;
        };
        if drive.speed <= 0.0 {
            continue;
        }
        let angle = drive.phase
            * match rotor.index {
                0 => 1.0,
                1 => -1.0,
                _ => -0.75,
            };
        transform.rotation = Quat::from_rotation_x(angle);
    }
    if status.working != working {
        status.working = working;
    }
}

fn placeholder() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0; 3]; 3])
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 3])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0; 2]; 3])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0; 4]; 3])
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

fn particle_geometry(now: f32, working: &[([i64; 3], Quat)]) -> Result<MeshBatch, String> {
    let mut batches = BTreeMap::new();
    for (position, rotation) in working.iter().take(MAX_DUST_MACHINES) {
        let seed = (position[0].wrapping_mul(73) ^ position[2].wrapping_mul(137)) as u32;
        for i in 0..DUST_PER_MACHINE {
            let phase = (now * 1.7 + i as f32 * 0.137 + (seed % 101) as f32 * 0.01).fract();
            let random = ((seed.wrapping_add(i as u32 * 37) % 97) as f32 / 96.0 - 0.5) * 0.36;
            let x = if i % 2 == 0 { 0.545 } else { 1.455 };
            let local = Vec3::new(
                x + random,
                0.53 - phase * 0.18 - phase * phase * 0.13,
                0.66 + phase * 0.23,
            ) - CENTER;
            let center = Vec3::from_array(position.map(|v| v as f32)) + CENTER + *rotation * local;
            let fade = (phase * 12.0).min(1.0) * ((1.0 - phase) * 8.0).min(1.0);
            let size = (0.020 + ((i + seed as usize) % 4) as f32 * 0.008) * fade.max(0.025);
            let start = batches
                .get(&0)
                .map_or(0, |batch: &MeshBatch| batch.positions.len());
            crate::props::add_box(
                &mut batches,
                0,
                center,
                Vec3::splat(size),
                *rotation * Quat::from_euler(EulerRot::XYZ, phase * 4.0, i as f32, phase * 2.0),
            )?;
            let batch = batches
                .get_mut(&0)
                .ok_or("Crusher dust geometry unavailable")?;
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            let tint = Color::srgb(
                0.67 + random * 0.20,
                0.52 + random * 0.13,
                0.35 + random * 0.10,
            )
            .to_linear()
            .to_f32_array();
            batch.colors[start..].fill(tint);
        }
    }
    Ok(batches.remove(&0).unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
fn dust(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<Machines>,
    drives: Query<(&Drive, &Transform)>,
    cameras: Cameras,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut status: ResMut<CrusherRenderStatus>,
) {
    let now = time.elapsed_secs();
    if now < state.next_dust {
        return;
    }
    state.next_dust = now + 0.05;
    let camera = cameras
        .iter()
        .next()
        .map_or(Vec3::ZERO, |camera| camera.translation);
    let mut working: Vec<_> = drives
        .iter()
        .filter(|(drive, pose)| {
            drive.working
                && !drive.crank
                && pose.translation.distance_squared(camera) < 32.0_f32.powi(2)
        })
        .map(|(drive, pose)| (drive.position, pose.rotation))
        .collect();
    working.sort_unstable_by(|(a, _), (b, _)| {
        Vec3::from_array(a.map(|v| v as f32))
            .distance_squared(camera)
            .total_cmp(&Vec3::from_array(b.map(|v| v as f32)).distance_squared(camera))
            .then_with(|| a.cmp(b))
    });
    working.truncate(MAX_DUST_MACHINES);
    let Some((mesh, entity)) = state.dust.clone().or_else(|| {
        if working.is_empty() {
            return None;
        }
        let mesh = meshes.add(placeholder());
        let entity = commands
            .spawn((
                Name::new("Batched crusher chips"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.94,
                    ..default()
                })),
                Transform::default(),
                Visibility::Hidden,
                NoFrustumCulling,
                NotShadowCaster,
                crate::miniature::layers(),
            ))
            .id();
        state.dust = Some((mesh.clone(), entity));
        Some((mesh, entity))
    }) else {
        status.particles = 0;
        return;
    };
    if working.is_empty() {
        commands.entity(entity).insert(Visibility::Hidden);
        status.particles = 0;
        return;
    }
    match particle_geometry(now, &working) {
        Ok(batch) => {
            if let Some(mut mesh) = meshes.get_mut(&mesh) {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals);
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs);
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors);
                mesh.insert_indices(Indices::U32(batch.indices));
                commands.entity(entity).insert(Visibility::Inherited);
                status.particles = working.len() * DUST_PER_MACHINE;
            }
        }
        Err(error) => tracing::warn!(%error,"Cannot compose crusher particles"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roller_drive_is_smooth_and_stops_without_an_unbounded_phase() {
        let mut drive = Drive {
            position: [0; 3],
            working: true,
            manual_active: false,
            crank: false,
            speed: 0.0,
            phase: 0.0,
        };
        drive.advance(1.0 / 60.0);
        assert!(drive.speed > 0.0 && drive.speed < 3.4);
        for _ in 0..600 {
            drive.advance(1.0 / 60.0);
        }
        assert!((drive.speed - 3.4).abs() < 0.01);
        assert!((0.0..std::f32::consts::TAU).contains(&drive.phase));
        drive.working = false;
        for _ in 0..120 {
            drive.advance(1.0 / 60.0);
        }
        assert_eq!(drive.speed, 0.0);
        let phase = drive.phase;
        drive.advance(1.0 / 60.0);
        assert_eq!(phase, drive.phase);
        drive.manual_active = true;
        drive.advance(1.0 / 60.0);
        assert!(
            drive.speed > 0.0,
            "an empty attached crank still turns its gear"
        );
        assert!(!drive.working, "empty cranking must not emit output chips");
    }
    #[test]
    fn crusher_particles_are_bounded_valid_geometry_without_particle_entities() {
        let machines = vec![([0, 0, 0], Quat::IDENTITY); MAX_DUST_MACHINES * 4];
        let batch = particle_geometry(27.5, &machines).unwrap();
        assert_eq!(
            batch.positions.len(),
            MAX_DUST_MACHINES * DUST_PER_MACHINE * 24
        );
        assert!(batch.positions.iter().flatten().all(|v| v.is_finite()));
        assert!(
            batch
                .indices
                .iter()
                .all(|i| (*i as usize) < batch.positions.len())
        );
        assert_eq!(batch.positions.len(), batch.normals.len());
        assert_eq!(batch.positions.len(), batch.colors.len());
        assert_eq!(rotor_material("crusher_roller_left_edge"), Some(0));
        assert_eq!(rotor_material("crusher_roller_right"), Some(1));
        assert_eq!(rotor_material("crusher_cog_edge"), Some(2));
        assert_eq!(rotor_material("crank_rotating_grip"), Some(3));
        assert_eq!(rotor_material("iron"), None);
    }

    #[test]
    fn output_chips_fall_from_rolls_into_the_tray_instead_of_rising_or_crossing_ground() {
        let machine = [([0; 3], Quat::IDENTITY)];
        let early = particle_geometry(0.1 / 1.7, &machine).unwrap();
        let late = particle_geometry(0.8 / 1.7, &machine).unwrap();
        let center = |batch: &MeshBatch| {
            batch.positions[..24]
                .iter()
                .map(|v| Vec3::from_array(*v))
                .sum::<Vec3>()
                / 24.0
        };
        let early = center(&early);
        let late = center(&late);
        assert!(late.y < early.y && late.z > early.z);
        for time in [0.0, 0.2, 0.6, 1.4, 7.3] {
            let batch = particle_geometry(time, &machine).unwrap();
            assert!(batch.positions.iter().all(|v| v[1] > 0.19 && v[1] < 0.58));
        }
    }

    #[test]
    fn repeated_machine_reload_reuses_parts_and_removes_rotors_with_the_owner() {
        use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
        use std::{sync::Arc, time::Duration};
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let owner = [15, 2, 3];
        let peer = [16, 2, 3];
        let crank = [17, 2, 3];
        let mut journal = EditJournal::default();
        journal
            .record(owner, terra_core::crusher::MATERIAL)
            .unwrap();
        journal
            .set_object_state(
                owner,
                Some(BlockObjectState {
                    peer: Some(peer),
                    ..default()
                }),
            )
            .unwrap();
        journal
            .record(crank, terra_core::crusher::HAND_CRANK_MATERIAL)
            .unwrap();
        journal
            .set_object_state(
                crank,
                Some(BlockObjectState {
                    peer: Some(owner),
                    ..default()
                }),
            )
            .unwrap();
        let mut templates = TemplateSet::default();
        for index in 0..6 {
            templates
                .crusher_moving_materials
                .insert(index, (index / 2) as u8);
        }
        templates.blocks.insert(
            terra_core::crusher::MATERIAL,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: (0..6)
                    .map(|material| PrimitiveTemplate {
                        positions: vec![[0.2, 0.3, 0.4], [0.3, 0.3, 0.4], [0.2, 0.4, 0.4]],
                        normals: vec![[0.0, 0.0, 1.0]; 3],
                        uvs: vec![[0.0; 2]; 3],
                        indices: vec![0, 1, 2],
                        material,
                    })
                    .collect(),
            },
        );
        let mut crank_template = templates.blocks[&terra_core::crusher::MATERIAL].clone();
        crank_template.model.truncate(3);
        for (index, primitive) in crank_template.model.iter_mut().enumerate() {
            primitive.material = index + 6;
            templates.crusher_moving_materials.insert(index + 6, 3);
        }
        templates
            .blocks
            .insert(terra_core::crusher::HAND_CRANK_MATERIAL, crank_template);
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = BlockAssets::default();
        assets.materials = (0..9)
            .map(|_| materials.add(StandardMaterial::default()))
            .collect();
        assets.templates = Some(Arc::new(templates));
        let section = SectionPos::new(1, 0, 0);
        let loaded = || {
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block([0, 2, 3], terra_core::crusher::MATERIAL)
                .unwrap();
            data.set_block([1, 2, 3], terra_core::crusher::HAND_CRANK_MATERIAL)
                .unwrap();
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            }
        };
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(section, loaded());
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(assets)
            .insert_resource(materials)
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(journal)
            .insert_resource(chunks);
        install(&mut app);
        app.update();
        assert_eq!(app.world().resource::<CrusherRenderStatus>().machines, 1);
        assert_eq!(app.world().resource::<CrusherRenderStatus>().cranks, 1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 9);
        assert_eq!(
            app.world_mut().query::<&Rotor>().iter(app.world()).count(),
            4
        );
        let scans = app.world().resource::<CrusherRenderStatus>().source_scans;
        for _ in 0..8 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(110));
            app.update();
        }
        assert_eq!(
            app.world().resource::<CrusherRenderStatus>().source_scans,
            scans
        );
        for _ in 0..8 {
            app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(110));
            app.update();
            assert_eq!(
                app.world_mut().query::<&Rotor>().iter(app.world()).count(),
                0
            );
            assert_eq!(app.world().resource::<CrusherRenderStatus>().machines, 0);
            assert_eq!(app.world().resource::<CrusherRenderStatus>().cranks, 0);
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .insert(section, loaded());
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(110));
            app.update();
            assert_eq!(
                app.world_mut().query::<&Rotor>().iter(app.world()).count(),
                4
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 9);
        }
    }

    #[test]
    fn public_loaded_lookup_keeps_negative_boundary_owner_and_ignores_far_history() {
        use terra_voxel::{BlockSection, SectionPos, global_to_section};
        use terra_world::streaming::LoadedSection;
        let owner = [-1, 2, -1];
        let peer = [0, 2, -1];
        let mut host = EditJournal::default();
        host.record(owner, terra_core::crusher::MATERIAL).unwrap();
        host.set_object_state(
            owner,
            Some(BlockObjectState {
                peer: Some(peer),
                ..default()
            }),
        )
        .unwrap();
        let section = global_to_section(owner).0;
        let mut replica = EditJournal::default();
        replica.import_delta(host.section_delta(section)).unwrap();
        assert_eq!(replica.crushers_in_section(section).count(), 0);
        let loaded = SectionPos::new(0, 0, -1);
        let mut data = BlockSection::empty(loaded).unwrap();
        data.set_block([0, 2, 15], terra_core::crusher::MATERIAL)
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            loaded,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut state = Machines::default();
        assert!(refresh_candidates(&mut state, &replica, &chunks));
        assert_eq!(state.sources.len(), 5);
        assert_eq!(state.candidates.len(), 1);
        assert_eq!(state.candidates[0].0, owner);
        assert!(super::loaded(
            &chunks,
            owner,
            state.candidates[0].1,
            state.candidates[0].2
        ));
        for i in 0..256 {
            let position = [512 + i * 32, 2, 3];
            replica
                .record(position, terra_core::crusher::MATERIAL)
                .unwrap();
            replica
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some([position[0] + 1, 2, 3]),
                        ..default()
                    }),
                )
                .unwrap();
        }
        assert!(!refresh_candidates(&mut state, &replica, &chunks));
        assert_eq!(state.candidates.len(), 1);
        replica.record(owner, 0).unwrap();
        assert!(refresh_candidates(&mut state, &replica, &chunks));
        assert!(state.candidates.is_empty());
    }
}
