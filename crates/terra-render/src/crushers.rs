//! Chunk-batched machine frames and shared articulated rollers; no tooth entities.
use crate::{
    blocks::{BlockAssets, TemplateSet},
    meshing::builder::MeshBatch,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::{NoAutoAabb, NoFrustumCulling},
    },
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
mod belts;
mod casings;
mod ribbon_tint;
mod wheel_water;

pub(crate) fn conveyor_preview_geometry(span: terra_world::conveyors::Span) -> MeshBatch {
    let mut batch = belts::geometry(span.length, belts::step_length(span));
    let rotation = belts::pose(span).rotation;
    let translation = Vec3::from_array(std::array::from_fn(|axis| {
        (span.start[axis] - span.controller[axis]) as f32 + 0.5
    }));
    for point in &mut batch.positions {
        *point = (rotation * Vec3::from_array(*point) + translation).to_array();
    }
    for normal in &mut batch.normals {
        *normal = (rotation * Vec3::from_array(*normal)).to_array();
    }
    batch
}

const MAX_DUST_MACHINES: usize = 16;
const DUST_PER_MACHINE: usize = 8;
const CENTER: Vec3 = Vec3::new(0.5, 0.0, 0.5);
const ROTARY_CENTER: Vec3 = Vec3::splat(0.5);
const BELT_RADIUS: f32 = 0.1;

pub(crate) fn articulated_material(material: u16) -> bool {
    terra_core::kinetic::is_part(material)
}

pub(crate) fn wood_material(name: &str) -> bool {
    matches!(
        name,
        "kinetic_gear_wooden"
            | "kinetic_gear_large_wooden"
            | "kinetic_shaft_wooden"
            | "port_support_wood"
    )
}

pub(crate) fn rotor_material(name: &str) -> Option<u8> {
    if name == "hydraulic_drive" {
        Some(16)
    } else if name.starts_with("kinetic_box_nx") {
        Some(11)
    } else if name.starts_with("kinetic_box_nz") {
        Some(12)
    } else if name.starts_with("kinetic_box_ny") {
        Some(13)
    } else if name.starts_with("kinetic_box_x") {
        Some(8)
    } else if name.starts_with("kinetic_box_z") {
        Some(9)
    } else if name.starts_with("kinetic_box_y") {
        Some(10)
    } else if name.starts_with("mixer_paddle") {
        Some(14)
    } else if name.starts_with("mixer_cog") {
        Some(15)
    } else if name.starts_with("crusher_roller_left") {
        Some(0)
    } else if name.starts_with("crusher_roller_right") {
        Some(1)
    } else if name.starts_with("crusher_cog") {
        Some(2)
    } else if name.starts_with("crank_rotating") {
        Some(3)
    } else if name.starts_with("kinetic_gear") {
        Some(4)
    } else if name.starts_with("kinetic_shaft") {
        Some(5)
    } else if name.starts_with("kinetic_wheel") {
        Some(6)
    } else if name.starts_with("kinetic_belt") {
        Some(7)
    } else {
        None
    }
}

fn pivot(rotor: u8) -> Vec3 {
    match rotor {
        0 => Vec3::new(0.5, 0.5, 0.35),
        1 => Vec3::new(0.5, 0.5, 0.65),
        2 => Vec3::new(0.94, 0.5, 0.5),
        3 => Vec3::new(0.22, 0.455, 0.515),
        14 => ROTARY_CENTER,
        15 => Vec3::new(0.95, 0.5, 0.5),
        7 => Vec3::new(
            0.5,
            terra_core::kinetic::CONVEYOR_AXIS_HEIGHT + BELT_RADIUS,
            0.5,
        ),
        _ => ROTARY_CENTER,
    }
}

#[derive(Clone)]
struct Part {
    material_id: u16,
    rotor: u8,
    mesh: Handle<Mesh>,
    materials: Vec<Handle<StandardMaterial>>,
    ribbon: bool,
    bounds: Aabb,
}

type PlacedMachine = (
    Entity,
    u8,
    u16,
    Option<[i64; 3]>,
    terra_core::WoodSpecies,
    Option<terra_world::conveyors::Span>,
    u64,
    Option<[u8; 3]>,
    u64,
);

const WOODS: [terra_core::WoodSpecies; 4] = [
    terra_core::WoodSpecies::Oak,
    terra_core::WoodSpecies::Birch,
    terra_core::WoodSpecies::Spruce,
    terra_core::WoodSpecies::Cherry,
];

fn wood_index(wood: terra_core::WoodSpecies) -> usize {
    WOODS.iter().position(|&species| species == wood).unwrap()
}

fn ribbon_index(raw_facing: u8) -> usize {
    terra_core::kinetic::belt_color(raw_facing).map_or(0, |color| {
        terra_core::beds::DyeColor::ALL
            .iter()
            .position(|&dye| dye == color)
            .unwrap()
            + 1
    })
}

fn wood_materials(
    material: &Handle<StandardMaterial>,
    wooden: bool,
    materials: &mut Assets<StandardMaterial>,
) -> Option<[Handle<StandardMaterial>; 4]> {
    let base = materials.get(material)?.clone();
    Some(std::array::from_fn(|index| {
        if !wooden || index == 0 {
            return material.clone();
        }
        let mut variant = base.clone();
        let base = variant.base_color.to_linear().to_f32_array();
        let tint = WOODS[index].tint();
        variant.base_color = Color::linear_rgba(
            base[0] * tint[0],
            base[1] * tint[1],
            base[2] * tint[2],
            base[3] * tint[3],
        );
        materials.add(variant)
    }))
}

fn ribbon_materials(
    material: &Handle<StandardMaterial>,
    materials: &mut Assets<StandardMaterial>,
) -> Option<Vec<Handle<StandardMaterial>>> {
    let base = materials.get(material)?.clone();
    let color = base.base_color.to_linear().to_f32_array();
    Some(
        std::iter::once(material.clone())
            .chain(terra_core::beds::DyeColor::ALL.into_iter().map(|dye| {
                let mut variant = base.clone();
                let tint = dye.tint();
                variant.base_color = Color::linear_rgba(
                    color[0] * tint[0],
                    color[1] * tint[1],
                    color[2] * tint[2],
                    color[3],
                );
                materials.add(variant)
            }))
            .collect(),
    )
}

#[derive(Resource, Default)]
struct Machines {
    parts: Vec<Part>,
    placed: HashMap<[i64; 3], PlacedMachine>,
    next_scan: f32,
    next_dust: f32,
    dust: Option<(Handle<Mesh>, Entity)>,
    sources: HashMap<terra_voxel::SectionPos, u64>,
    scratch_sources: HashMap<terra_voxel::SectionPos, u64>,
    candidates: Vec<([i64; 3], u16, BlockObjectState)>,
    connection: Option<Entity>,
    belts: belts::Cache,
    casings: casings::Cache,
    wheel_water: wheel_water::Cache,
    ribbon_tints: HashMap<bevy::asset::AssetId<StandardMaterial>, Handle<ribbon_tint::Material>>,
}

#[derive(Component)]
struct Drive {
    position: [i64; 3],
    working: bool,
    crank: bool,
    rpm: f32,
    speed: f32,
    phase: f32,
}

impl Drive {
    fn advance(&mut self, dt: f32) {
        let rpm = if self.rpm.is_finite() { self.rpm } else { 0.0 };
        let target = rpm * std::f32::consts::TAU / 60.0;
        self.speed += (target - self.speed) * (1.0 - (-dt * 9.0).exp());
        if target == 0.0 && self.speed.abs() < 0.002 {
            self.speed = 0.0;
        }
        self.phase = (self.phase + self.speed * dt).rem_euclid(std::f32::consts::TAU);
    }
}

#[derive(Component)]
struct Rotor {
    root: Entity,
    index: u8,
    belt_offset: f32,
    phase_offset: f32,
}

#[derive(Resource, Default, Debug)]
pub struct CrusherRenderStatus {
    pub machines: usize,
    pub cranks: usize,
    pub kinetic_parts: usize,
    pub belt_spans: usize,
    pub grounded_belt_cells: usize,
    pub forward_parts: usize,
    pub reverse_parts: usize,
    pub phase_checksum: f32,
    pub working: usize,
    pub cached_meshes: usize,
    pub particles: usize,
    pub source_scans: u64,
    pub source_sections: usize,
}

fn refresh_candidates(state: &mut Machines, journal: &EditJournal, chunks: &ChunkStore) -> bool {
    let _scope = terra_core::profiling::ScopeTimer::new("machines.refresh_candidates");
    state.scratch_sources.clear();
    for source in journal.kinetic_sections() {
        if [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .any(|(x, z)| {
                source
                    .offset([x, 0, z])
                    .is_ok_and(|neighbor| chunks.loaded.contains_key(&neighbor))
            })
        {
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
            if articulated_material(material) && !object.secondary && seen.insert(position) {
                state.candidates.push((position, material, object));
            }
        }
    }
    true
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Machines>()
        .init_resource::<Assets<ribbon_tint::Material>>()
        .init_resource::<CrusherRenderStatus>()
        .add_systems(
            Update,
            (
                synchronize,
                animate,
                belts::animate,
                wheel_water::animate,
                dust,
            )
                .chain(),
        );
}

pub(crate) fn install_materials(app: &mut App) {
    app.add_plugins(MaterialPlugin::<ribbon_tint::Material>::default());
}

fn shared_parts(
    templates: &TemplateSet,
    assets: &BlockAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Option<Vec<Part>> {
    let models = templates
        .blocks
        .iter()
        .filter(|(id, _)| articulated_material(**id));
    for primitive in models.clone().flat_map(|(_, block)| &block.model) {
        if templates
            .crusher_moving_materials
            .contains_key(&primitive.material)
            && assets
                .materials
                .get(primitive.material)
                .and_then(|handle| materials.get(handle))
                .is_none()
        {
            return None;
        }
    }
    let mut parts = Vec::new();
    for (&material_id, block) in models {
        for primitive in &block.model {
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
                    .map(|p| (Vec3::from_array(*p) - origin).to_array())
                    .collect::<Vec<_>>(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, primitive.normals.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, primitive.uvs.clone())
            .with_inserted_indices(Indices::U32(primitive.indices.clone()));
            let ribbon = templates
                .conveyor_ribbon_materials
                .contains(&primitive.material);
            let source = assets.materials.get(primitive.material)?;
            let variants = if ribbon {
                ribbon_materials(source, materials)?
            } else {
                wood_materials(
                    source,
                    templates.wood_materials.contains(&primitive.material),
                    materials,
                )?
                .to_vec()
            };
            parts.push(Part {
                material_id,
                rotor,
                bounds: mesh.get_aabb()?,
                mesh: meshes.add(mesh),
                materials: variants,
                ribbon,
            });
        }
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

fn assembly_pose(position: [i64; 3], material: u16, state: BlockObjectState) -> Transform {
    let facing = if material == 148 {
        terra_core::kinetic::belt_facing(state.facing)
    } else {
        state.facing
    };
    let rotary = material == terra_core::crusher::HAND_CRANK_MATERIAL
        || terra_core::kinetic::is_rotary(material);
    let center = if rotary { ROTARY_CENTER } else { CENTER };
    let rotation = if rotary {
        Quat::from_rotation_arc(
            Vec3::X,
            Vec3::from_array(terra_core::kinetic::axis(facing).map(|v| v as f32)),
        )
    } else {
        Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2)
    };
    let crank_offset = if material == terra_core::crusher::HAND_CRANK_MATERIAL {
        rotation
            * Vec3::from_array(terra_core::kinetic::crank_local_offset(
                position, state.peer,
            ))
    } else {
        Vec3::ZERO
    };
    let width = if material == terra_core::crusher::MATERIAL && state.peer.is_some() {
        2.0
    } else {
        1.0
    };
    Transform::from_translation(
        Vec3::from_array(position.map(|v| v as f32))
            + center
            + crank_offset
            + rotation * Vec3::new((width - 1.0) * 0.5, 0.0, 0.0),
    )
    .with_rotation(rotation)
    .with_scale(Vec3::new(width, 1.0, 1.0))
}

fn network_rpm(
    position: [i64; 3],
    owner: [i64; 3],
    session: Option<&ClientSession>,
    journal: &EditJournal,
    manual_active: bool,
) -> f32 {
    if let Some(visual) =
        session.and_then(|session| session.kinetics.iter().find(|v| v.position == position))
    {
        return if visual.overloaded || !visual.rpm.is_finite() {
            0.0
        } else {
            visual.rpm
        };
    }
    let kinetic = journal.kinetic_state(position);
    if kinetic.overloaded {
        0.0
    } else if kinetic.rpm != 0.0 && kinetic.rpm.is_finite() {
        kinetic.rpm
    } else if position != owner {
        network_rpm(owner, owner, session, journal, manual_active)
    } else if manual_active {
        30.0
    } else {
        0.0
    }
}

fn gear_phase(position: [i64; 3], material: u16, facing: u8) -> f32 {
    if !terra_core::kinetic::is_gear(material) {
        return 0.0;
    }
    let axis = terra_core::kinetic::axis(facing);
    let parity = position
        .into_iter()
        .zip(axis)
        .filter(|(_, axis)| *axis == 0)
        .fold(0, |parity, (cell, _)| parity ^ cell.rem_euclid(2));
    if parity == 0 {
        std::f32::consts::PI / 8.0
    } else if terra_core::kinetic::is_large_gear(material) {
        std::f32::consts::PI / 16.0
    } else {
        0.0
    }
}

// A complete closed loop keeps tread movement inside the one-cell frame.
fn belt_pose(phase: f32, offset: f32) -> Transform {
    const RUN: f32 = belts::LEGACY_RUN;
    const START: f32 = (1.0 - RUN) * 0.5;
    let center = terra_core::kinetic::CONVEYOR_AXIS_HEIGHT;
    let arc = std::f32::consts::PI * BELT_RADIUS;
    let length = RUN * 2.0 + arc * 2.0;
    let d = (phase / std::f32::consts::TAU + offset).rem_euclid(1.0) * length;
    let (y, z, angle) = if d < RUN {
        (center + BELT_RADIUS, START + d, 0.0)
    } else if d < RUN + arc {
        let angle = (d - RUN) / BELT_RADIUS;
        (
            center + BELT_RADIUS * angle.cos(),
            START + RUN + BELT_RADIUS * angle.sin(),
            angle,
        )
    } else if d < RUN * 2.0 + arc {
        (
            center - BELT_RADIUS,
            START + RUN - (d - RUN - arc),
            std::f32::consts::PI,
        )
    } else {
        let angle = std::f32::consts::PI + (d - RUN * 2.0 - arc) / BELT_RADIUS;
        (
            center + BELT_RADIUS * angle.cos(),
            START + BELT_RADIUS * angle.sin(),
            angle,
        )
    };
    Transform::from_translation(Vec3::new(0.0, y, z - 0.5))
        .with_rotation(Quat::from_rotation_x(angle))
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ribbon_materials: ResMut<Assets<ribbon_tint::Material>>,
    session: Option<Res<ClientSession>>,
    mut state: ResMut<Machines>,
    mut drives: Query<&mut Drive>,
    surfaces: Query<&belts::Surface>,
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
        let Some(parts) = shared_parts(templates, &assets, &mut meshes, &mut materials) else {
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
        let span = if matches!(material, 144..=146 | 148) {
            terra_world::conveyors::span_at(&journal, position).filter(|span| {
                span.linked && span.controller == position && span.loaded(&chunks, &journal)
            })
        } else {
            None
        };
        if material == terra_core::kinetic::CONVEYOR && object.peer.is_some() && span.is_none() {
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
        let rpm = network_rpm(position, owner, session.as_deref(), &journal, manual_active);
        let casing_mask = span.map_or(0, |span| casings::mask(&chunks, span));
        let previous_contacts = state
            .placed
            .get(&position)
            .and_then(|placed| surfaces.get(placed.0).ok())
            .map_or_else(belts::Contacts::default, |surface| surface.contacts);
        let contacts = span.map_or_else(belts::Contacts::default, |span| {
            belts::contacts(&chunks, &journal, span, previous_contacts)
        });
        let tint = span.map_or_else(
            || journal.tint_at(position),
            |span| terra_core::tints::color(terra_world::conveyors::item(span, &journal)),
        );
        let mesh_epoch = journal.mesh_epoch(terra_voxel::blocks::global_to_section(position).0);
        let mut motion = (0.0, 0.0);
        let mut surface_offset = 0.0;
        if let Some(&(
            entity,
            facing,
            previous_material,
            peer,
            wood,
            previous_span,
            previous_mask,
            previous_tint,
            previous_epoch,
        )) = state.placed.get(&position)
        {
            let same_assembly = facing == object.facing
                && previous_material == material
                && peer == object.peer
                && wood == object.wood
                && previous_span == span
                && previous_mask == casing_mask;
            let same_contacts = surfaces
                .get(entity)
                .map_or_else(|_| belts::Contacts::default(), |surface| surface.contacts)
                == contacts;
            if same_assembly
                && previous_tint == tint
                && same_contacts
                && (casing_mask == 0 || state.casings.cached_meshes() != 0)
            {
                if let Ok(mut drive) = drives.get_mut(entity) {
                    drive.working = working;
                    drive.rpm = rpm;
                }
                continue;
            }
            // A tint-only refresh does not restart the mechanism. A physical
            // edit/replacement changes the mesh epoch and starts a new pose.
            if same_assembly
                && (previous_tint != tint || !same_contacts)
                && previous_epoch == mesh_epoch
            {
                if let Ok(drive) = drives.get(entity) {
                    motion = (drive.speed, drive.phase);
                }
                surface_offset = surfaces.get(entity).map_or(0.0, |surface| surface.offset);
            }
            commands.entity(entity).try_despawn();
            state.placed.remove(&position);
        }
        let root = commands
            .spawn((
                Name::new(if crank {
                    "Hand crank moving assembly"
                } else if material >= 141 {
                    "Kinetic moving assembly"
                } else {
                    "Block Crusher moving assembly"
                }),
                Drive {
                    position: owner,
                    working,
                    crank,
                    rpm,
                    speed: motion.0,
                    phase: motion.1,
                },
                span.map_or_else(|| assembly_pose(position, material, object), belts::pose),
                Visibility::Inherited,
            ))
            .id();
        let mut rotor_parent = root;
        if let Some(span) = span {
            let source = state
                .parts
                .iter()
                .find(|part| {
                    part.material_id == terra_core::kinetic::CONVEYOR
                        && part.rotor == 7
                        && part.ribbon
                })
                .map(|part| part.materials[0].clone());
            if source.is_none_or(|source| {
                belts::spawn(
                    &mut commands,
                    &mut state.belts,
                    &mut meshes,
                    &mut materials,
                    &mut ribbon_materials,
                    span,
                    root,
                    &source,
                    tint,
                    surface_offset,
                    contacts,
                    if material == terra_core::kinetic::CONVEYOR {
                        terra_core::kinetic::belt_facing(object.facing)
                    } else {
                        object.facing
                    },
                )
                .is_none()
            }) {
                commands.entity(root).try_despawn();
                continue;
            }
            casings::spawn(
                &mut commands,
                &mut state.casings,
                &mut meshes,
                &mut materials,
                templates,
                &assets,
                span,
                root,
                casing_mask,
            );
            if material == terra_core::kinetic::CONVEYOR {
                state.placed.insert(
                    position,
                    (
                        root,
                        object.facing,
                        material,
                        object.peer,
                        object.wood,
                        Some(span),
                        casing_mask,
                        tint,
                        mesh_epoch,
                    ),
                );
                continue;
            }
            let belt_pose = belts::pose(span);
            let shaft_pose = assembly_pose(position, material, object);
            let inverse = belt_pose.rotation.inverse();
            rotor_parent = commands
                .spawn((
                    Transform::from_translation(
                        inverse * (shaft_pose.translation - belt_pose.translation),
                    )
                    .with_rotation(inverse * shaft_pose.rotation),
                    Visibility::Inherited,
                    ChildOf(root),
                ))
                .id();
        }
        let center = if crank || terra_core::kinetic::is_rotary(material) {
            ROTARY_CENTER
        } else {
            CENTER
        };
        let indices: &[u8] = match material {
            99 => &[0, 1, 2],
            100 => &[3],
            141..=143 | 149..=151 => &[4],
            144..=146 => &[5],
            147 => &[6],
            154 => &[8, 9, 11, 12],
            155 => &[8, 10, 11, 13],
            156 => &[14, 15],
            168 => &[16],
            _ => &[7],
        };
        for &index in indices {
            for tread in 0..if index == 7 { 12 } else { 1 } {
                let belt_offset = tread as f32 / 12.0;
                let rotor = commands
                    .spawn((
                        Name::new(format!(
                            "{} rotor {index}",
                            if crank { "Hand crank" } else { "Mechanical" }
                        )),
                        Rotor {
                            root,
                            index,
                            belt_offset,
                            phase_offset: gear_phase(position, material, object.facing),
                        },
                        if index == 7 {
                            belt_pose(0.0, belt_offset)
                        } else {
                            Transform::from_translation(pivot(index) - center).with_rotation(
                                Quat::from_rotation_x(gear_phase(
                                    position,
                                    material,
                                    object.facing,
                                )),
                            )
                        },
                        Visibility::Inherited,
                        ChildOf(rotor_parent),
                    ))
                    .id();
                let Machines {
                    parts,
                    ribbon_tints,
                    ..
                } = &mut *state;
                for part in parts
                    .iter()
                    .filter(|part| part.rotor == index && part.material_id == material)
                {
                    let source = part.materials[if part.ribbon {
                        if tint.is_some() {
                            0
                        } else {
                            ribbon_index(object.facing)
                        }
                    } else {
                        wood_index(object.wood)
                    }]
                    .clone();
                    let rgb_material = if part.ribbon && tint.is_some() {
                        ribbon_tint::cached_material(
                            ribbon_tints,
                            &source,
                            &materials,
                            &mut ribbon_materials,
                        )
                    } else {
                        None
                    };
                    let mut entity = commands.spawn((
                        Mesh3d(part.mesh.clone()),
                        part.bounds,
                        NoAutoAabb,
                        Transform::default(),
                        crate::miniature::layers(),
                        terra_core::miniature::MiniatureMesh::Terrain,
                        ChildOf(rotor),
                    ));
                    if let Some(material) = rgb_material {
                        entity.insert((MeshMaterial3d(material), ribbon_tint::tag(tint.unwrap())));
                    } else {
                        entity.insert(MeshMaterial3d(source));
                    }
                }
            }
        }
        if material == terra_core::kinetic::WATER_WHEEL {
            wheel_water::spawn(
                &mut commands,
                &mut state.wheel_water,
                &mut meshes,
                &mut materials,
                root,
            );
        }
        state.placed.insert(
            position,
            (
                root,
                object.facing,
                material,
                object.peer,
                object.wood,
                span,
                casing_mask,
                tint,
                mesh_epoch,
            ),
        );
    }
    state
        .placed
        .retain(|position, (entity, _, _, _, _, _, _, _, _)| {
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
    status.machines = state
        .candidates
        .iter()
        .filter(|(p, m, _)| *m == 99 && state.placed.contains_key(p))
        .count();
    status.kinetic_parts = state.placed.len() - status.cranks - status.machines;
    status.belt_spans = state
        .placed
        .values()
        .filter(|placed| placed.5.is_some())
        .count();
    status.grounded_belt_cells = state
        .placed
        .values()
        .map(|placed| placed.6.count_ones() as usize)
        .sum();
    status.cached_meshes =
        state.parts.len() + state.casings.cached_meshes() + state.wheel_water.cached_meshes();
}

fn animate(
    time: Res<Time>,
    mut drives: Query<&mut Drive>,
    mut rotors: Query<(&Rotor, &mut Transform)>,
    mut status: ResMut<CrusherRenderStatus>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let mut working = 0;
    let mut forward = 0;
    let mut reverse = 0;
    let mut phase_checksum = 0.0;
    for mut drive in &mut drives {
        drive.advance(dt);
        working += usize::from(drive.working && !drive.crank);
        forward += usize::from(drive.speed > 0.002);
        reverse += usize::from(drive.speed < -0.002);
        phase_checksum += drive.phase;
    }
    for (rotor, mut transform) in &mut rotors {
        let Ok(drive) = drives.get(rotor.root) else {
            continue;
        };
        if drive.speed == 0.0 {
            continue;
        }
        if rotor.index == 7 {
            *transform = belt_pose(drive.phase, rotor.belt_offset);
            continue;
        }
        let angle = rotor.phase_offset
            + drive.phase
                * match rotor.index {
                    0 => 1.0,
                    1 => -1.0,
                    2 => -0.75,
                    9..=11 => -1.0,
                    _ => 1.0,
                };
        transform.rotation = match rotor.index {
            9 | 12 => Quat::from_rotation_z(angle),
            10 | 13 => Quat::from_rotation_y(angle),
            _ => Quat::from_rotation_x(angle),
        };
    }
    if status.working != working {
        status.working = working;
    }
    status.forward_parts = forward;
    status.reverse_parts = reverse;
    status.phase_checksum = phase_checksum;
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
            let x = if i % 2 == 0 { 0.35 } else { 0.65 };
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
#[path = "crushers/belt_owner_tests.rs"]
mod belt_owner_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_machine_sources_ignore_empty_loaded_terrain_and_follow_mutations_and_unloads() {
        use terra_voxel::{BlockSection, SectionPos, global_to_section};
        use terra_world::streaming::LoadedSection;
        let empty = |section| LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        };
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in 0..128 {
            let section = SectionPos::new(x, 0, 8);
            chunks.loaded.insert(section, empty(section));
            journal.record([x * 16, 1, 128], 3).unwrap();
        }
        let mut state = Machines::default();
        for _ in 0..8 {
            assert!(!refresh_candidates(&mut state, &journal, &chunks));
            assert!(state.sources.is_empty() && state.scratch_sources.is_empty());
        }
        let position = [-17, 4, -17];
        let (owner, local) = global_to_section(position);
        journal.record(position, 144).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        assert!(!refresh_candidates(&mut state, &journal, &chunks));
        let neighbor = owner.offset([1, 0, 0]).unwrap();
        chunks.loaded.insert(neighbor, empty(neighbor));
        assert!(refresh_candidates(&mut state, &journal, &chunks));
        assert_eq!(state.sources.len(), 1);
        assert_eq!(state.candidates[0].0, position);
        assert!(!loaded(&chunks, position, 144, state.candidates[0].2));
        let mut section = empty(owner);
        section.data.set_block(local, 144).unwrap();
        chunks.loaded.insert(owner, section);
        assert!(!refresh_candidates(&mut state, &journal, &chunks));
        assert!(loaded(&chunks, position, 144, state.candidates[0].2));
        let changed = BlockObjectState {
            facing: 4,
            wood: terra_core::WoodSpecies::Cherry,
            ..default()
        };
        journal.set_object_state(position, Some(changed)).unwrap();
        assert!(refresh_candidates(&mut state, &journal, &chunks));
        assert_eq!(state.candidates, vec![(position, 144, changed)]);
        journal.record([4000, 1, 4000], 3).unwrap();
        assert!(!refresh_candidates(&mut state, &journal, &chunks));
        chunks.loaded.remove(&owner);
        assert!(!refresh_candidates(&mut state, &journal, &chunks));
        chunks.loaded.remove(&neighbor);
        assert!(refresh_candidates(&mut state, &journal, &chunks));
        assert!(state.sources.is_empty() && state.candidates.is_empty());
        chunks.loaded.insert(owner, empty(owner));
        assert!(refresh_candidates(&mut state, &journal, &chunks));
        journal.record(position, 0).unwrap();
        assert!(refresh_candidates(&mut state, &journal, &chunks));
        assert!(state.sources.is_empty() && state.candidates.is_empty());
    }

    #[test]
    fn tint_only_refresh_preserves_real_belt_drive_and_span_uv_phase_but_replacement_does_not() {
        use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        for linked in [false, true] {
            let position = [-12, 4, -12];
            let start = [-12, 4, -13];
            let end = [-12, 4, -11];
            let object = BlockObjectState {
                peer: linked.then_some(end),
                ..default()
            };
            let section = SectionPos::new(-1, 0, -1);
            let mut data = BlockSection::empty(section).unwrap();
            let mut journal = EditJournal::default();
            for (cell, material, state) in std::iter::once((position, 148, object)).chain(
                linked
                    .then_some([
                        (
                            start,
                            144,
                            BlockObjectState {
                                peer: Some(position),
                                ..default()
                            },
                        ),
                        (
                            end,
                            144,
                            BlockObjectState {
                                peer: Some(start),
                                ..default()
                            },
                        ),
                    ])
                    .into_iter()
                    .flatten(),
            ) {
                journal.record(cell, material).unwrap();
                journal.set_object_state(cell, Some(state)).unwrap();
                data.set_block(cell.map(|v| v.rem_euclid(16) as usize), material)
                    .unwrap();
            }
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut templates = TemplateSet::default();
            templates.conveyor_ribbon_materials.insert(0);
            templates.crusher_moving_materials.insert(0, 7);
            templates.blocks.insert(
                148,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: vec![PrimitiveTemplate {
                        material: 0,
                        positions: vec![[0.2, 0.6, 0.2], [0.8, 0.6, 0.2], [0.2, 0.6, 0.8]],
                        normals: vec![[0.0, 1.0, 0.0]; 3],
                        tangents: Vec::new(),
                        uvs: vec![[0.0; 2]; 3],
                        indices: vec![0, 1, 2],
                    }],
                },
            );
            let mut materials = Assets::<StandardMaterial>::default();
            let mut assets = BlockAssets::default();
            assets
                .materials
                .push(materials.add(StandardMaterial::default()));
            assets.templates = Some(std::sync::Arc::new(templates));
            let mut app = App::new();
            app.insert_resource(Time::<()>::default())
                .insert_resource(journal)
                .insert_resource(chunks)
                .insert_resource(assets)
                .insert_resource(materials)
                .init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<ribbon_tint::Material>>()
                .init_resource::<Machines>()
                .init_resource::<CrusherRenderStatus>()
                .add_systems(Update, synchronize);
            app.update();
            let root = app.world().resource::<Machines>().placed[&position].0;
            {
                let mut drive = app.world_mut().get_mut::<Drive>(root).unwrap();
                drive.speed = -1.7;
                drive.phase = 2.3;
            }
            if linked {
                let world = app.world_mut();
                let mut query = world.query::<&mut belts::Surface>();
                query.single_mut(world).unwrap().offset = 0.37;
            }
            let meshes = app.world().resource::<Assets<Mesh>>().len();
            let rendered = app.world_mut().query::<&Mesh3d>().iter(app.world()).count();
            let epoch = app.world().resource::<EditJournal>().mesh_epoch(section);
            for color in [Some([71, 93, 184]), Some([237, 92, 31]), None] {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_tint(position, color)
                    .unwrap();
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_millis(110));
                app.update();
                let root = app.world().resource::<Machines>().placed[&position].0;
                let drive = app.world().get::<Drive>(root).unwrap();
                assert_eq!((drive.speed, drive.phase), (-1.7, 2.3));
                assert_eq!(
                    app.world().resource::<EditJournal>().mesh_epoch(section),
                    epoch
                );
                assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
                assert_eq!(
                    app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
                    rendered
                );
                if linked {
                    let world = app.world_mut();
                    let mut query = world.query::<&belts::Surface>();
                    let surface = query.single(world).unwrap();
                    assert_eq!(surface.root, root);
                    assert_eq!(surface.offset, 0.37);
                    let uv = match &surface.material {
                        belts::SurfaceMaterial::Standard(handle) => {
                            world
                                .resource::<Assets<StandardMaterial>>()
                                .get(handle)
                                .unwrap()
                                .uv_transform
                        }
                        belts::SurfaceMaterial::Rgb(handle) => {
                            world
                                .resource::<Assets<ribbon_tint::Material>>()
                                .get(handle)
                                .unwrap()
                                .base
                                .uv_transform
                        }
                    };
                    assert_eq!(uv.translation, Vec2::new(0.0, 0.37));
                }
            }
            {
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                journal.record(position, 0).unwrap();
                journal.record(position, 148).unwrap();
                journal.set_object_state(position, Some(object)).unwrap();
                journal.set_tint(position, Some([20, 200, 80])).unwrap();
            }
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(110));
            app.update();
            let root = app.world().resource::<Machines>().placed[&position].0;
            let drive = app.world().get::<Drive>(root).unwrap();
            assert_eq!(
                (drive.speed, drive.phase),
                (0.0, 0.0),
                "new logical belts must not inherit old motion"
            );
        }
    }

    #[test]
    fn gearbox_rotor_axes_and_signs_match_all_authoritative_yawed_ports() {
        assert_eq!(rotor_material("kinetic_box_x"), Some(8));
        assert_eq!(rotor_material("kinetic_box_z"), Some(9));
        assert_eq!(rotor_material("kinetic_box_y"), Some(10));
        assert_eq!(rotor_material("kinetic_box_nx"), Some(11));
        assert_eq!(rotor_material("kinetic_box_nz"), Some(12));
        assert_eq!(rotor_material("kinetic_box_ny"), Some(13));
        for material in [154, 155] {
            for facing in 0..4 {
                let ports = terra_core::kinetic::gearbox_ports(material, facing);
                for (index, expected) in [(0, 1), (1, -1), (2, -1), (3, 1)] {
                    let port = ports[index];
                    let world_sign = ports[if index < 2 { 0 } else { 2 }]
                        .into_iter()
                        .sum::<i64>() as i8;
                    assert_eq!(
                        terra_core::kinetic::gearbox_port_factor(material, facing, port).unwrap()
                            * world_sign,
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn ribbon_palette_is_gray_or_dyed_independently_of_wood_and_packed_pose() {
        let mut materials = Assets::<StandardMaterial>::default();
        let base = materials.add(StandardMaterial {
            base_color: Color::linear_rgba(0.68, 0.70, 0.72, 1.0),
            ..default()
        });
        let palette = ribbon_materials(&base, &mut materials).unwrap();
        assert_eq!(palette.len(), 14);
        assert_eq!(materials.len(), 14);
        assert_eq!(palette[0], base);
        for (index, dye) in terra_core::beds::DyeColor::ALL.into_iter().enumerate() {
            let tint = dye.tint();
            assert_eq!(
                materials
                    .get(&palette[index + 1])
                    .unwrap()
                    .base_color
                    .to_linear()
                    .to_f32_array(),
                [0.68 * tint[0], 0.70 * tint[1], 0.72 * tint[2], 1.0]
            );
            for facing in 0..4 {
                let raw = terra_core::kinetic::dyed_belt_facing(facing, dye);
                assert_eq!(ribbon_index(raw), index + 1);
                let pose = |facing| {
                    assembly_pose(
                        [1, -16, 2],
                        148,
                        BlockObjectState {
                            facing,
                            ..default()
                        },
                    )
                };
                assert!(pose(raw).rotation.abs_diff_eq(pose(facing).rotation, 1e-6));
            }
            for facing in [4, 5] {
                let raw = terra_core::kinetic::dyed_belt_facing(
                    terra_core::kinetic::belt_pose(facing),
                    dye,
                );
                assert_eq!(ribbon_index(raw), index + 1);
            }
        }
    }

    #[test]
    fn kinetic_wood_variants_share_geometry_and_never_tint_the_metal_edges() {
        for name in [
            "kinetic_gear_wooden",
            "kinetic_gear_large_wooden",
            "kinetic_shaft_wooden",
        ] {
            assert!(wood_material(name));
            assert!(!wood_material(&format!("{name}_edge")));
        }
        assert!(!wood_material("kinetic_belt"));
        let mut materials = Assets::<StandardMaterial>::default();
        let base = materials.add(StandardMaterial {
            base_color: Color::linear_rgba(0.6, 0.4, 0.2, 1.0),
            metallic: 0.2,
            ..default()
        });
        let wooden = wood_materials(&base, true, &mut materials).unwrap();
        assert_eq!(wooden[0], base);
        assert_eq!(materials.len(), 4);
        for wood in WOODS {
            let material = materials.get(&wooden[wood_index(wood)]).unwrap();
            let tint = wood.tint();
            assert_eq!(
                material.base_color.to_linear().to_f32_array(),
                [0.6 * tint[0], 0.4 * tint[1], 0.2 * tint[2], tint[3]]
            );
            assert_eq!(material.metallic, 0.2);
        }
        let metal = wood_materials(&base, false, &mut materials).unwrap();
        assert!(metal.iter().all(|handle| *handle == base));
        assert_eq!(materials.len(), 4);
        for _ in 0..100 {
            for wood in WOODS {
                assert!(materials.get(&wooden[wood_index(wood)]).is_some());
            }
        }
        assert_eq!(
            materials.len(),
            4,
            "stationary variants reuse cached materials"
        );
    }

    #[test]
    fn actual_moving_belt_palette_dyes_only_the_ribbon_and_keeps_frame_wood_independent() {
        use crate::blocks::{BlockTemplate, PrimitiveTemplate};
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = BlockAssets::default();
        for color in [
            [0.68, 0.70, 0.72, 1.0],
            [0.6, 0.4, 0.2, 1.0],
            [0.4, 0.5, 0.6, 1.0],
        ] {
            assets.materials.push(materials.add(StandardMaterial {
                base_color: Color::linear_rgba(color[0], color[1], color[2], color[3]),
                ..default()
            }));
        }
        let primitive = |material| PrimitiveTemplate {
            material,
            positions: vec![[0.2, 0.6, 0.2], [0.8, 0.6, 0.2], [0.2, 0.6, 0.8]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            tangents: Vec::new(),
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
        };
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            148,
            BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1), primitive(2)],
            },
        );
        templates.conveyor_ribbon_materials.insert(0);
        templates.wood_materials.insert(1);
        templates.crusher_moving_materials.extend([(0, 7), (2, 7)]);
        let frames = wood_materials(&assets.materials[1], true, &mut materials).unwrap();
        let frame_colors: Vec<_> = frames
            .iter()
            .map(|handle| materials.get(handle).unwrap().base_color)
            .collect();
        let mut meshes = Assets::<Mesh>::default();
        let parts = shared_parts(&templates, &assets, &mut meshes, &mut materials).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(meshes.len(), 2, "dye variants share geometry");
        let ribbon = parts.iter().find(|part| part.ribbon).unwrap();
        let metal = parts.iter().find(|part| !part.ribbon).unwrap();
        for wood in WOODS {
            assert_eq!(metal.materials[wood_index(wood)], assets.materials[2]);
            for color in terra_core::beds::DyeColor::ALL {
                let facing = terra_core::kinetic::dyed_belt_facing(0, color);
                assert_ne!(ribbon.materials[ribbon_index(facing)], assets.materials[1]);
                assert_eq!(
                    materials.get(&frames[wood_index(wood)]).unwrap().base_color,
                    frame_colors[wood_index(wood)]
                );
            }
        }
        assert_eq!(
            materials.get(&assets.materials[1]).unwrap().base_color,
            Color::linear_rgba(0.6, 0.4, 0.2, 1.0)
        );
    }
    #[test]
    fn moving_drops_and_gameplay_revisions_do_not_rescan_or_rebuild_machine_layouts() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let position = [4, 4, 4];
        let section = SectionPos::new(0, 0, 0);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([4, 4, 4], 141).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        journal.record(position, 141).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let mut machines = Machines::default();
        assert!(refresh_candidates(&mut machines, &journal, &chunks));
        for i in 0..16 {
            assert!(journal.gameplay_mut().drop_stack(
                [8.5, 5.0, i as f64 + 0.5],
                terra_core::ItemStack {
                    item: terra_core::ItemKind::Block(3),
                    count: 1
                },
                [0.0; 3],
                0.0
            ));
            assert!(!refresh_candidates(&mut machines, &journal, &chunks));
        }
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    wood: terra_core::WoodSpecies::Birch,
                    ..default()
                }),
            )
            .unwrap();
        assert!(refresh_candidates(&mut machines, &journal, &chunks));
        assert_eq!(machines.candidates.len(), 1);
    }

    #[test]
    fn mixer_paddles_and_drive_gear_follow_the_same_real_rpm_without_extra_entities() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .init_resource::<CrusherRenderStatus>()
            .add_systems(Update, animate);
        let root = app
            .world_mut()
            .spawn(Drive {
                position: [-17, 3, -33],
                working: false,
                crank: false,
                rpm: -12.5,
                speed: 0.0,
                phase: 0.0,
            })
            .id();
        let parts: Vec<_> = [14, 15]
            .into_iter()
            .map(|index| {
                app.world_mut()
                    .spawn((
                        Rotor {
                            root,
                            index,
                            belt_offset: 0.0,
                            phase_offset: 0.0,
                        },
                        Transform::from_translation(pivot(index) - CENTER),
                    ))
                    .id()
            })
            .collect();
        for _ in 0..120 {
            app.update();
        }
        let drive = app.world().get::<Drive>(root).unwrap();
        assert!(drive.speed < 0.0);
        let expected = Quat::from_rotation_x(drive.phase);
        for (entity, index) in parts.iter().zip([14, 15]) {
            let transform = app.world().get::<Transform>(*entity).unwrap();
            assert_eq!(transform.translation, pivot(index) - CENTER);
            assert!((transform.rotation * Vec3::Y).abs_diff_eq(expected * Vec3::Y, 0.00001));
        }
        assert_eq!(
            app.world_mut().query::<&Rotor>().iter(app.world()).count(),
            2
        );
        assert!(articulated_material(156));
    }

    #[test]
    fn compact_and_legacy_crusher_rotors_share_the_exact_static_body_pose() {
        let position = [-17, 3, -33];
        for facing in 0..4 {
            for legacy in [false, true] {
                let pose = assembly_pose(
                    position,
                    99,
                    BlockObjectState {
                        facing,
                        peer: legacy.then_some([-16, 3, -33]),
                        ..default()
                    },
                );
                let width = if legacy { 2.0 } else { 1.0 };
                assert_eq!(pose.scale, Vec3::new(width, 1.0, 1.0));
                for point in [Vec3::ZERO, Vec3::ONE, pivot(0), pivot(1), pivot(2)] {
                    let actual = pose.transform_point(point - CENTER);
                    let expected = Vec3::from_array(position.map(|v| v as f32))
                        + CENTER
                        + pose.rotation * (Vec3::new(point.x * width, point.y, point.z) - CENTER);
                    assert!(actual.abs_diff_eq(expected, 0.00001));
                }
            }
        }
        assert_eq!(pivot(2).y, 0.5);
        assert_eq!(pivot(2).z, 0.5);
    }

    #[test]
    fn generic_and_legacy_crank_rotors_are_coaxial_with_the_centered_drive() {
        let position = [-16, 4, -16];
        for facing in 0..6 {
            let axis = terra_core::kinetic::axis(facing);
            for cells in [1, 2] {
                let peer = std::array::from_fn(|i| position[i] - axis[i] * cells);
                let pose = assembly_pose(
                    position,
                    100,
                    BlockObjectState {
                        facing,
                        peer: Some(peer),
                        ..default()
                    },
                );
                let actual = pose.transform_point(pivot(3) - ROTARY_CENTER);
                let local = Vec3::new(-0.28, 0.0, 0.0);
                let expected = Vec3::from_array(position.map(|v| v as f32))
                    + ROTARY_CENTER
                    + pose.rotation * local;
                assert!(actual.distance(expected) < 0.00001);
            }
        }
    }

    #[test]
    fn gear_tooth_phase_alternates_in_its_radial_plane_but_not_along_shafts() {
        let small = terra_core::kinetic::WOODEN_GEAR;
        let large = terra_core::kinetic::WOODEN_LARGE_GEAR;
        for (facing, axis, radial) in [
            (0, [1, 0, 0], [0, 1, 0]),
            (4, [0, 1, 0], [0, 0, 1]),
            (1, [0, 0, 1], [1, 0, 0]),
        ] {
            let origin = gear_phase([0; 3], small, facing);
            assert_eq!(gear_phase(axis, small, facing), origin);
            assert_ne!(gear_phase(radial, small, facing), origin);
            let negative = radial.map(|v| -v);
            assert_eq!(
                gear_phase(negative, small, facing),
                gear_phase(radial, small, facing)
            );
            assert_eq!(
                gear_phase(radial, large, facing),
                std::f32::consts::PI / 16.0
            );
        }
    }

    #[test]
    fn fractional_rpm_is_not_rounded_and_invalid_visual_speed_cannot_poison_transforms() {
        let mut drive = Drive {
            position: [0; 3],
            working: false,
            crank: false,
            rpm: 7.5,
            speed: 0.0,
            phase: 0.0,
        };
        for _ in 0..240 {
            drive.advance(1.0 / 60.0);
        }
        assert!((drive.speed - std::f32::consts::PI / 4.0).abs() < 0.00001);
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            drive.rpm = invalid;
            drive.advance(1.0 / 60.0);
            assert!(drive.speed.is_finite() && drive.phase.is_finite());
        }
    }

    #[test]
    fn rotary_axes_and_closed_belt_treads_stay_aligned_with_their_cells() {
        for facing in 0..6 {
            let pose = assembly_pose(
                [-3, 4, -5],
                141,
                BlockObjectState {
                    facing,
                    ..default()
                },
            );
            assert_eq!(pose.translation, Vec3::new(-2.5, 4.5, -4.5));
            let axis = Vec3::from_array(terra_core::kinetic::axis(facing).map(|v| v as f32));
            assert!((pose.rotation * Vec3::X - axis).length() < 0.00001);
        }
        for step in -600..=600 {
            let phase = step as f32 * 0.017;
            for offset in [0.0, 0.25, 0.5, 0.75] {
                let pose = belt_pose(phase, offset);
                for x in [-0.32, 0.32] {
                    for y in [-0.01, 0.011] {
                        for z in [-0.045, 0.045] {
                            let point = CENTER + pose.transform_point(Vec3::new(x, y, z));
                            assert!(point.x >= 0.0 && point.x <= 1.0);
                            assert!(point.y > 0.0 && point.y <= terra_core::kinetic::CONVEYOR_TOP);
                            assert!(point.z >= 0.0 && point.z <= 1.0);
                        }
                    }
                }
            }
        }
        assert_eq!(rotor_material("kinetic_gear_iron_edge"), Some(4));
        assert_eq!(rotor_material("kinetic_shaft_wooden"), Some(5));
        assert_eq!(rotor_material("kinetic_wheel_edge"), Some(6));
        assert_eq!(rotor_material("kinetic_belt"), Some(7));
    }

    #[test]
    fn network_rpm_overrides_working_animation_and_overload_stops_all_parts() {
        let owner = [1, 2, 3];
        let crank = [2, 2, 3];
        let mut journal = EditJournal::default();
        journal.replace_kinetic_states(std::collections::BTreeMap::from([(
            owner,
            terra_core::kinetic::KineticState {
                rpm: 45.0,
                overloaded: false,
                ..default()
            },
        )]));
        assert_eq!(network_rpm(owner, owner, None, &journal, false), 45.0);
        assert_eq!(network_rpm(crank, owner, None, &journal, false), 45.0);
        let mut session = ClientSession::default();
        session
            .kinetics
            .push(terra_network::protocol::session::KineticVisual {
                position: owner,
                rpm: -7.5,
                overloaded: false,
                ..default()
            });
        assert_eq!(
            network_rpm(owner, owner, Some(&session), &journal, true),
            -7.5
        );
        session.kinetics[0].overloaded = true;
        assert_eq!(
            network_rpm(owner, owner, Some(&session), &journal, true),
            0.0
        );
    }

    #[test]
    fn roller_drive_is_smooth_and_stops_without_an_unbounded_phase() {
        let mut drive = Drive {
            position: [0; 3],
            working: true,
            crank: false,
            rpm: 30.0,
            speed: 0.0,
            phase: 0.0,
        };
        drive.advance(1.0 / 60.0);
        assert!(drive.speed > 0.0 && drive.speed < std::f32::consts::PI);
        for _ in 0..600 {
            drive.advance(1.0 / 60.0);
        }
        assert!((drive.speed - std::f32::consts::PI).abs() < 0.01);
        assert!((0.0..std::f32::consts::TAU).contains(&drive.phase));
        drive.working = false;
        drive.rpm = 0.0;
        for _ in 0..120 {
            drive.advance(1.0 / 60.0);
        }
        assert_eq!(drive.speed, 0.0);
        let phase = drive.phase;
        drive.advance(1.0 / 60.0);
        assert_eq!(phase, drive.phase);
        drive.rpm = -60.0;
        drive.advance(1.0 / 60.0);
        assert!(
            drive.speed < 0.0,
            "negative network RPM must reverse the attached crank"
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
        assert_eq!(rotor_material("mixer_paddle_wood"), Some(14));
        assert_eq!(rotor_material("mixer_paddle_iron"), Some(14));
        assert_eq!(rotor_material("mixer_cog_wood"), Some(15));
        assert_eq!(rotor_material("mixer_cog_iron"), Some(15));
        assert_eq!(pivot(14), ROTARY_CENTER);
        assert_eq!(pivot(15), Vec3::new(0.95, 0.5, 0.5));
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
                        tangents: Vec::new(),
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
        assert_eq!(
            state.sources.len(),
            1,
            "only the mechanical owner section is indexed"
        );
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
