//! Replicated furnace fire and smoke share two meshes and a fixed local light budget.
mod alchemy;
use bevy::{
    asset::RenderAssetUsages,
    camera::Exposure,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use std::collections::{BTreeSet, HashMap};
use terra_network::{client::ClientSession, protocol::session::FurnaceVisual};
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const MAX_FIRES: usize = 32;
const MAX_LIGHTS: usize = 8;
const FIRE_VIEW_DISTANCE: f32 = 96.0;

// Match the visible fire mesh and soften the last sixteen metres of its pool.
fn fire_light_gain(distance: f32) -> f32 {
    let t = ((FIRE_VIEW_DISTANCE - distance) / 16.0).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
const MAX_SMOKE_CELLS: usize = 256;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(crate) struct SmokeExtension {
    #[uniform(100)]
    pub(crate) animation: Vec4,
}
impl MaterialExtension for SmokeExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_furnace_smoke.wesl".into()
    }
}
pub(crate) type SmokeMaterial = ExtendedMaterial<StandardMaterial, SmokeExtension>;

#[derive(Resource)]
struct Effects {
    fire: Handle<Mesh>,
    smoke: Handle<Mesh>,
    fire_entity: Entity,
    smoke_entity: Entity,
    next: f32,
    visible: bool,
    vents: Vec<Vent>,
    next_vents: f32,
}

struct Vent {
    cell: [i64; 3],
    direction: [i64; 3],
    smoke_cells: usize,
    strength: f32,
}

fn vent_strength(smoke_cells: usize) -> f32 {
    match smoke_cells {
        0 => 0.14,
        1..=7 => 0.30,
        8..=31 => 0.50,
        32..=95 => 0.75,
        _ => 1.0,
    }
}

#[derive(Component)]
struct FurnaceLamp([i64; 3]);

pub(crate) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<SmokeMaterial>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, update_haze.after(crate::water::environment::update))
        .add_systems(
            PostUpdate,
            update.in_set(crate::scene::caves::lights::ApplyLocalLights),
        );
}

pub(crate) fn mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    // The renderer's mesh allocator requires nonempty buffers even while hidden.
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0; 3]; 3])
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 3])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0; 4]; 3])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0; 2]; 3])
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut smoke_materials: ResMut<Assets<SmokeMaterial>>,
) {
    let fire = meshes.add(mesh());
    let smoke = meshes.add(mesh());
    let fire_entity = commands
        .spawn((
            Name::new("Furnace flames"),
            Mesh3d(fire.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                unlit: true,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            bevy::camera::visibility::NoFrustumCulling,
        ))
        .id();
    let smoke_material = smoke_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            fog_enabled: false,
            ..default()
        },
        extension: SmokeExtension {
            animation: Vec4::ZERO,
        },
    });
    let smoke_entity = commands
        .spawn((
            Name::new("Furnace smoke wisps"),
            Mesh3d(smoke.clone()),
            MeshMaterial3d(smoke_material.clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            bevy::camera::visibility::NoFrustumCulling,
        ))
        .id();
    commands.insert_resource(Effects {
        fire,
        smoke,
        fire_entity,
        smoke_entity,
        next: 0.0,
        visible: false,
        vents: Vec::new(),
        next_vents: 0.0,
    });
}

#[derive(Default)]
pub(crate) struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}
impl Geometry {
    fn cuboid(&mut self, center: Vec3, half: Vec3, rotation: Quat, color: [f32; 4]) {
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let mut normal = Vec3::ZERO;
                normal[axis] = sign;
                let mut u = Vec3::ZERO;
                let mut v = Vec3::ZERO;
                u[(axis + 1) % 3] = half[(axis + 1) % 3] * sign;
                v[(axis + 2) % 3] = half[(axis + 2) % 3];
                let middle = normal * half[axis];
                let base = self.positions.len() as u32;
                self.positions.extend(
                    [-u - v, u - v, u + v, -u + v]
                        .map(|p| (center + rotation * (middle + p)).to_array()),
                );
                self.normals.extend([(rotation * normal).to_array(); 4]);
                self.colors.extend([color; 4]);
                self.uvs.extend([[0.0; 2]; 4]);
                self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
            }
        }
    }
    pub(crate) fn wisp(
        &mut self,
        center: Vec3,
        radius: f32,
        right: Vec3,
        up: Vec3,
        color: [f32; 4],
    ) {
        let base = self.positions.len() as u32;
        self.positions.extend(
            [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .map(|(x, y)| (center + (right * x + up * y) * radius).to_array()),
        );
        self.normals.extend([right.cross(up).to_array(); 4]);
        self.colors.extend([color; 4]);
        self.uvs
            .extend([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    }
    fn confine_last_wisp(&mut self, cell: [i64; 3], mut passable: impl FnMut([i64; 3]) -> bool) {
        let mut low = Vec3::splat(f32::NEG_INFINITY);
        let mut high = Vec3::splat(f32::INFINITY);
        for axis in 0..3 {
            let mut neighbor = cell;
            neighbor[axis] -= 1;
            if !passable(neighbor) {
                low[axis] = cell[axis] as f32 + 0.002;
            }
            neighbor[axis] += 2;
            if !passable(neighbor) {
                high[axis] = cell[axis] as f32 + 0.998;
            }
        }
        let start = self.positions.len().saturating_sub(4);
        for position in &mut self.positions[start..] {
            *position = Vec3::from_array(*position).clamp(low, high).to_array();
        }
    }
    pub(crate) fn write(self, target: &mut Mesh) -> bool {
        if self.positions.is_empty() || self.indices.is_empty() {
            return false;
        }
        target.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        target.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        target.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        target.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        target.insert_indices(Indices::U32(self.indices));
        true
    }

    fn upload(self, meshes: &mut Assets<Mesh>, handle: &Handle<Mesh>) -> bool {
        use bevy::mesh::VertexAttributeValues as V;
        if self.positions.is_empty() || self.indices.is_empty() {
            return false;
        }
        let Some(target) = meshes.get(handle) else {
            return false;
        };
        // Cold stored brews do not animate. Do not mark their shared asset
        // modified merely because the fire-effect cadence elapsed again.
        if matches!(target.attribute(Mesh::ATTRIBUTE_POSITION),Some(V::Float32x3(values)) if values==&self.positions)
            && matches!(target.attribute(Mesh::ATTRIBUTE_NORMAL),Some(V::Float32x3(values)) if values==&self.normals)
            && matches!(target.attribute(Mesh::ATTRIBUTE_COLOR),Some(V::Float32x4(values)) if values==&self.colors)
            && matches!(target.attribute(Mesh::ATTRIBUTE_UV_0),Some(V::Float32x2(values)) if values==&self.uvs)
            && matches!(target.indices(),Some(Indices::U32(values)) if values==&self.indices)
        {
            return true;
        }
        meshes
            .get_mut(handle)
            .is_some_and(|mut target| self.write(&mut target))
    }
}

/// Isolated native art fixture using the same batch/shader as runtime smoke.
pub(crate) fn showcase_wisps(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SmokeMaterial>,
    camera: &Transform,
    angled: bool,
    room_cells: usize,
    layer: usize,
) -> Entity {
    let mut geometry = Geometry::default();
    let (origin, direction) = if angled {
        (Vec3::new(-1.5, 2.5, 1.50), Vec3::Z)
    } else {
        (Vec3::new(-1.5, 2.02, 0.0), Vec3::Y)
    };
    for index in 0..12 {
        let age = index as f32 / 12.0;
        let center = origin + direction * age * 0.9 + Vec3::Y * age * age * 2.2;
        geometry.wisp(
            center,
            0.16 + age * 0.32,
            *camera.right(),
            *camera.up(),
            [0.22, 0.205, 0.19, (1.0 - age) * 0.34],
        );
    }
    for index in 0..room_cells.min(96) {
        let center = Vec3::new(
            (index % 6) as f32 - 2.5,
            (index / 36) as f32 + 0.5,
            ((index / 6) % 6) as f32 - 2.5,
        );
        geometry.wisp(
            center,
            0.35,
            *camera.right(),
            *camera.up(),
            [0.17, 0.16, 0.15, 0.22],
        );
    }
    let mut data = mesh();
    geometry.write(&mut data);
    commands
        .spawn((
            Name::new("Native furnace smoke shader fixture"),
            Mesh3d(meshes.add(data)),
            MeshMaterial3d(materials.add(ExtendedMaterial {
                base: StandardMaterial {
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    cull_mode: None,
                    double_sided: true,
                    fog_enabled: false,
                    ..default()
                },
                extension: SmokeExtension {
                    animation: Vec4::ZERO,
                },
            })),
            Transform::default(),
            NotShadowCaster,
            bevy::camera::visibility::RenderLayers::layer(layer),
        ))
        .id()
}

pub(crate) fn fire_position(position: [i64; 3], journal: &EditJournal) -> Vec3 {
    let facing = journal.object_state(position).unwrap_or_default().facing;
    let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
    Vec3::from_array(position.map(|v| v as f32))
        + Vec3::new(0.5, 0.0, 0.5)
        + rotation * Vec3::new(0.0, 0.55, 0.38)
}

fn nearby_smoke_cells(
    furnaces: &[FurnaceVisual],
    eye: Vec3,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Vec<[i64; 3]> {
    let distance = |cell: [i64; 3]| {
        (Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5)).distance_squared(eye)
    };
    let unique: BTreeSet<_> = furnaces
        .iter()
        .flat_map(|furnace| &furnace.smoke_cells)
        .copied()
        .filter(|&cell| distance(cell) <= 64.0_f32.powi(2))
        .filter(|&cell| block(cell).is_some_and(terra_world::furnaces::smoke_passable))
        .collect();
    let mut cells: Vec<_> = unique.into_iter().collect();
    cells.sort_unstable_by(|&a, &b| distance(a).total_cmp(&distance(b)).then(a.cmp(&b)));
    cells.truncate(MAX_SMOKE_CELLS);
    cells
}

#[derive(Default)]
struct HazeState {
    next: f32,
    target: f32,
    density: f32,
}

fn smoke_density(
    eye: Vec3,
    smoke: &BTreeSet<[i64; 3]>,
    mut passable: impl FnMut([i64; 3]) -> bool,
) -> f32 {
    let origin = eye.floor().as_i64vec3().to_array();
    if smoke.is_empty() || !passable(origin) {
        return 0.0;
    }
    let mut visited = BTreeSet::from([origin]);
    let mut queue = std::collections::VecDeque::from([(origin, 0)]);
    let mut amount = 0.0;
    while let Some((cell, steps)) = queue.pop_front() {
        if smoke.contains(&cell) {
            let center = Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5);
            amount += (1.0 - center.distance(eye) / 2.5).max(0.0);
        }
        if steps >= 3 {
            continue;
        }
        for direction in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let next = std::array::from_fn(|axis| cell[axis] + direction[axis]);
            if visited.insert(next) && passable(next) {
                queue.push_back((next, steps + 1));
            }
        }
    }
    (amount / 8.0).min(1.0)
}

#[allow(clippy::too_many_arguments)]
fn update_haze(
    time: Res<Time>,
    access: Res<WorldAccess>,
    session: Option<Res<ClientSession>>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    underwater: Res<crate::water::UnderwaterState>,
    mut cameras: Query<
        (&Transform, &mut crate::scene::caves::shader::CaveSettings),
        With<TerraCamera>,
    >,
    mut state: Local<HazeState>,
) {
    for (camera, mut settings) in &mut cameras {
        if time.elapsed_secs() >= state.next {
            state.next = time.elapsed_secs() + 0.10;
            let smoke: BTreeSet<_> = session
                .as_ref()
                .filter(|session| access.ready() && session.ready)
                .into_iter()
                .flat_map(|session| &session.furnaces)
                .flat_map(|furnace| furnace.smoke_cells.iter().copied())
                .filter(|cell| {
                    let center = Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5);
                    center.distance_squared(camera.translation) < 2.5_f32.powi(2)
                })
                .collect();
            state.target = smoke_density(camera.translation, &smoke, |cell| {
                chunks.block_at(cell).is_some_and(|raw| {
                    terra_world::furnaces::smoke_cell_passable(
                        journal.block(cell).unwrap_or(raw),
                        journal.object_state(cell).map_or(0, |object| object.facing),
                    )
                })
            }) * (1.0 - underwater.blend);
        }
        if state.target == 0.0 && state.density == 0.0 && settings.smoke == Vec4::ZERO {
            continue;
        }
        state.density += (state.target - state.density) * (1.0 - (-3.0 * time.delta_secs()).exp());
        if state.density < 0.001 && state.target == 0.0 {
            state.density = 0.0;
        }
        let smoke = Vec4::new(
            state.density,
            if state.density > 0.0 {
                time.elapsed_secs()
            } else {
                0.0
            },
            0.0,
            0.0,
        );
        if settings.smoke != smoke {
            settings.smoke = smoke;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update(
    mut commands: Commands,
    time: Res<Time>,
    access: Res<WorldAccess>,
    session: Option<Res<ClientSession>>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    mut effects: ResMut<Effects>,
    mut meshes: ResMut<Assets<Mesh>>,
    cameras: Query<(&Transform, &Exposure), (With<TerraCamera>, Without<FurnaceLamp>)>,
    mut lamps: Query<(Entity, &FurnaceLamp, &mut PointLight, &mut Transform), Without<TerraCamera>>,
) {
    let camera = cameras.iter().next();
    let eye = camera.map_or(Vec3::ZERO, |(camera, _)| camera.translation);
    let ev100 = camera.map_or(6.0, |(_, exposure)| exposure.ev100);
    let right = camera.map_or(Vec3::X, |(camera, _)| *camera.right());
    let up = camera.map_or(Vec3::Y, |(camera, _)| *camera.up());
    let session = session
        .as_ref()
        .filter(|s| access.ready() && s.ready && camera.is_some());
    let mut nearby: Vec<_> = session
        .into_iter()
        .flat_map(|s| &s.furnaces)
        .filter(|f| f.lit)
        .filter(|f| {
            chunks
                .block_at(f.position)
                .is_some_and(|raw| journal.block(f.position).unwrap_or(raw) == 59)
        })
        .filter(|f| {
            Vec3::from_array(f.position.map(|v| v as f32)).distance_squared(eye)
                < FIRE_VIEW_DISTANCE.powi(2)
        })
        .collect();
    nearby.sort_by(|a, b| {
        let distance = |p: [i64; 3]| Vec3::from_array(p.map(|v| v as f32)).distance_squared(eye);
        distance(a.position).total_cmp(&distance(b.position))
    });
    nearby.truncate(MAX_FIRES);
    let mut alchemy = alchemy::nearby(
        session.map(|session| &**session),
        &chunks,
        &journal,
        eye,
        MAX_FIRES,
    );
    let mut fire_cells: Vec<_> = nearby
        .iter()
        .map(|fire| fire.position)
        .chain(alchemy.fire_cells())
        .collect();
    fire_cells.sort_by(|a, b| {
        let distance = |p: [i64; 3]| Vec3::from_array(p.map(|v| v as f32)).distance_squared(eye);
        distance(*a).total_cmp(&distance(*b))
    });
    let fire_cells: BTreeSet<_> = fire_cells.into_iter().take(MAX_FIRES).collect();
    nearby.retain(|fire| fire_cells.contains(&fire.position));
    alchemy.retain_fires(&fire_cells);
    let gain = 2.0_f32.powf(ev100.clamp(6.0, 13.7) - 6.0);
    let intensity = 950.0 * gain * (1.0 + 0.07 * (time.elapsed_secs() * 7.1).sin());
    let mut light_positions: Vec<_> = nearby
        .iter()
        .filter(|f| f.lit)
        .map(|f| (f.position, fire_position(f.position, &journal)))
        .chain(alchemy.lights())
        .collect();
    light_positions.sort_by(|a, b| {
        a.1.distance_squared(eye)
            .total_cmp(&b.1.distance_squared(eye))
    });
    let mut desired: HashMap<_, _> = light_positions.into_iter().take(MAX_LIGHTS).collect();
    for (entity, lamp, mut light, mut transform) in &mut lamps {
        if let Some(position) = desired.remove(&lamp.0) {
            transform.translation = position;
            light.intensity = intensity * fire_light_gain(position.distance(eye));
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (cell, position) in desired {
        commands.spawn((
            Name::new("Furnace fire light"),
            FurnaceLamp(cell),
            PointLight {
                color: Color::srgb(1.0, 0.62, 0.28),
                intensity: intensity * fire_light_gain(position.distance(eye)),
                range: 8.0,
                radius: 0.16,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(position),
        ));
    }
    let has_smoke = session.is_some_and(|session| {
        session
            .furnaces
            .iter()
            .any(|furnace| !furnace.smoke_cells.is_empty())
    });
    let active = !nearby.is_empty() || has_smoke || alchemy.active();
    if !active && !effects.visible {
        return;
    }
    if time.elapsed_secs() < effects.next && active {
        return;
    }
    let block = |cell| {
        chunks
            .block_at(cell)
            .map(|raw| journal.block(cell).unwrap_or(raw))
    };
    let cells = session.map_or_else(Vec::new, |session| {
        nearby_smoke_cells(&session.furnaces, eye, block)
    });
    if session.is_none() || (!has_smoke && nearby.is_empty()) {
        effects.vents.clear();
    } else if time.elapsed_secs() >= effects.next_vents {
        effects.next_vents = time.elapsed_secs() + 0.5;
        let previous_vents = std::mem::take(&mut effects.vents);
        if let Some(session) = session {
            let mut sources: Vec<_> = session
                .furnaces
                .iter()
                .filter(|f| f.lit || !f.smoke_cells.is_empty())
                .filter(|f| {
                    Vec3::from_array(f.outlet.map(|v| v as f32)).distance_squared(eye)
                        < 48.0_f32.powi(2)
                })
                .collect();
            sources.sort_by(|a, b| {
                let d = |f: &FurnaceVisual| {
                    Vec3::from_array(f.outlet.map(|v| v as f32)).distance_squared(eye)
                };
                d(a).total_cmp(&d(b))
            });
            for furnace in sources.into_iter().take(4) {
                for vent in
                    terra_world::furnaces::ventilation_outlets(furnace.outlet, &chunks, &journal)
                {
                    if vent.0 == furnace.outlet {
                        continue;
                    }
                    if let Some(existing) = effects
                        .vents
                        .iter_mut()
                        .find(|v| (v.cell, v.direction) == vent)
                    {
                        existing.smoke_cells = existing
                            .smoke_cells
                            .saturating_add(furnace.smoke_cells.len());
                    } else if effects.vents.len() < 8 {
                        effects.vents.push(Vent {
                            cell: vent.0,
                            direction: vent.1,
                            smoke_cells: furnace.smoke_cells.len(),
                            strength: previous_vents
                                .iter()
                                .find(|v| (v.cell, v.direction) == vent)
                                .map_or(0.0, |v| v.strength),
                        });
                    }
                }
            }
        }
    }
    effects.visible = !nearby.is_empty() || !cells.is_empty() || alchemy.active();
    effects.next = time.elapsed_secs() + 0.05;
    let mut fire = Geometry::default();
    let mut smoke = Geometry::default();
    alchemy.append(
        &mut fire,
        &mut smoke,
        (&chunks, &journal),
        (time.elapsed_secs(), right, up),
    );
    for furnace in nearby {
        let facing = journal
            .object_state(furnace.position)
            .unwrap_or_default()
            .facing;
        let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
        let anchor =
            Vec3::from_array(furnace.position.map(|v| v as f32)) + Vec3::new(0.5, 0.0, 0.5);
        for index in 0..5 {
            let phase = time.elapsed_secs() * 7.7 + index as f32 * 1.9;
            let height = 0.16 + (phase.sin() * 0.5 + 0.5) * 0.19;
            let x = (index as f32 - 2.0) * 0.072;
            fire.cuboid(
                anchor + rotation * Vec3::new(x, 0.36 + height * 0.5, 0.29),
                Vec3::new(0.036, height * 0.5, 0.045),
                rotation,
                [3.0, 0.35 + height, 0.025, 1.0],
            );
            fire.cuboid(
                anchor + rotation * Vec3::new(x, 0.38 + height * 0.25, 0.34),
                Vec3::new(0.019, height * 0.25, 0.012),
                rotation,
                [5.0, 2.1, 0.12, 1.0],
            );
        }
        if !block(furnace.outlet).is_some_and(terra_world::furnaces::smoke_passable) {
            continue;
        }
        let direction = terra_world::furnaces::chimney_outlet(furnace.position, &chunks, &journal)
            .filter(|(outlet, _)| *outlet == furnace.outlet)
            .map_or(Vec3::Y, |(_, direction)| {
                Vec3::from_array(direction.map(|v| v as f32))
            });
        let outlet = Vec3::from_array(furnace.outlet.map(|v| v as f32)) + Vec3::splat(0.5)
            - direction * 0.40;
        for index in 0..12 {
            let age = (time.elapsed_secs() * 0.28 + index as f32 / 12.0).fract();
            let phase = index as f32 * 2.1 + time.elapsed_secs();
            let center = outlet
                + direction * (age * 0.90)
                + Vec3::new(
                    phase.sin() * age * 0.18,
                    age * age * 2.2,
                    phase.cos() * age * 0.18,
                );
            let cell = center.floor().as_i64vec3().to_array();
            if !block(cell).is_some_and(terra_world::furnaces::smoke_passable) {
                continue;
            }
            let radius = 0.16 + age * 0.32;
            smoke.wisp(
                center,
                radius,
                right,
                up,
                [0.22, 0.205, 0.19, (1.0 - age).powf(1.2) * 0.34],
            );
            smoke.confine_last_wisp(cell, |neighbor| {
                block(neighbor).is_some_and(|material| {
                    terra_world::furnaces::smoke_cell_passable(
                        material,
                        journal
                            .object_state(neighbor)
                            .map_or(0, |object| object.facing),
                    )
                })
            });
        }
    }
    for cell in cells.into_iter().rev() {
        let center = Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5);
        smoke.wisp(center, 0.35, right, up, [0.17, 0.16, 0.15, 0.22]);
    }
    for vent in &mut effects.vents {
        vent.strength += (vent_strength(vent.smoke_cells) - vent.strength) * 0.18;
        let strength = vent.strength;
        let (cell, direction) = (vent.cell, vent.direction);
        let direction = Vec3::from_array(direction.map(|v| v as f32));
        let origin = Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5) - direction * 0.48;
        for index in 0..12 {
            let age = (time.elapsed_secs() * 0.26 + index as f32 / 12.0).fract();
            let phase = index as f32 * 2.1 + time.elapsed_secs();
            let center = origin
                + direction * (age * (0.65 + strength * 0.65))
                + Vec3::new(
                    phase.sin() * age * 0.14,
                    age * age * (1.6 + strength * 1.6),
                    phase.cos() * age * 0.14,
                );
            let location = center.floor().as_i64vec3().to_array();
            let passable = |p| {
                block(p).is_some_and(|m| {
                    terra_world::furnaces::smoke_cell_passable(
                        m,
                        journal.object_state(p).map_or(0, |s| s.facing),
                    )
                })
            };
            if !passable(location) {
                continue;
            }
            smoke.wisp(
                center,
                0.15 + strength * 0.07 + age * (0.20 + strength * 0.30),
                right,
                up,
                [
                    0.21,
                    0.205,
                    0.19,
                    (1.0 - age).powf(1.3) * (0.10 + strength * 0.36),
                ],
            );
            smoke.confine_last_wisp(location, passable);
        }
    }
    for (geometry, handle, entity) in [
        (fire, &effects.fire, effects.fire_entity),
        (smoke, &effects.smoke, effects.smoke_entity),
    ] {
        let visible = geometry.upload(&mut meshes, handle);
        commands.entity(entity).insert(if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_light_keeps_its_strength_at_midrange_and_fades_with_its_mesh() {
        for d in [20.0, 28.0, 40.0, 79.0] {
            assert_eq!(super::fire_light_gain(d), 1.0);
        }
        assert_eq!(super::fire_light_gain(88.0), 0.5);
        assert_eq!(super::fire_light_gain(super::FIRE_VIEW_DISTANCE), 0.0);
    }

    #[test]
    fn escaping_smoke_has_bounded_increasing_tiers_without_adding_wisps() {
        let tiers = [0, 1, 8, 32, 96].map(vent_strength);
        assert!(tiers.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(vent_strength(4096) <= 1.0);
        let mut strength = 0.0;
        for _ in 0..30 {
            let previous = strength;
            strength += (vent_strength(100) - strength) * 0.18;
            assert!(strength > previous && strength < 1.0);
            assert!(strength - previous <= 0.181);
        }
        assert!(strength > 0.99);
    }
    #[test]
    fn nearby_smoke_survives_a_distant_unloaded_or_unlit_furnace_anchor() {
        let furnace = FurnaceVisual {
            position: [500, 1, 0],
            outlet: [500, 3, 0],
            lit: false,
            smoke_cells: vec![[0, 1, 0], [65, 1, 0], [0, 1, 0]],
        };
        let cells = nearby_smoke_cells(&[furnace], Vec3::ZERO, |cell| {
            (cell == [0, 1, 0]).then_some(terra_voxel::MaterialId::Torch as u16)
        });
        assert_eq!(cells, vec![[0, 1, 0]]);
    }

    #[test]
    fn smoke_geometry_uses_the_authoritative_passability_rule() {
        use terra_voxel::MaterialId;
        let materials = [
            MaterialId::Air,
            MaterialId::Torch,
            MaterialId::CaveVine,
            MaterialId::CaveRoot,
            MaterialId::CaveMushroom,
            MaterialId::FluidWater,
            MaterialId::Stone,
            MaterialId::ChimneyStraight,
        ];
        let furnace = FurnaceVisual {
            position: [200, 1, 0],
            outlet: [200, 3, 0],
            lit: true,
            smoke_cells: (0..materials.len()).map(|x| [x as i64, 0, 0]).collect(),
        };
        let cells = nearby_smoke_cells(&[furnace], Vec3::ZERO, |cell| {
            materials
                .get(cell[0] as usize)
                .map(|material| *material as u16)
        });
        assert_eq!(cells, (0..5).map(|x| [x, 0, 0]).collect::<Vec<_>>());
    }

    #[test]
    fn smoke_geometry_budget_keeps_the_nearest_unique_cells() {
        let cells: Vec<_> = (-4..4)
            .flat_map(|x| (-4..4).flat_map(move |y| (-4..4).map(move |z| [x, y, z])))
            .collect();
        let mut furnaces: Vec<_> = cells
            .chunks(MAX_SMOKE_CELLS)
            .map(|cells| FurnaceVisual {
                position: [500, 1, 0],
                outlet: [500, 3, 0],
                lit: false,
                smoke_cells: cells.to_vec(),
            })
            .collect();
        furnaces.push(furnaces[0].clone());
        let selected = nearby_smoke_cells(&furnaces, Vec3::ZERO, |_| Some(0));
        assert_eq!(selected.len(), MAX_SMOKE_CELLS);
        assert_eq!(
            selected.iter().collect::<BTreeSet<_>>().len(),
            MAX_SMOKE_CELLS
        );
        let distance = |cell: [i64; 3]| {
            (Vec3::from_array(cell.map(|v| v as f32)) + Vec3::splat(0.5)).length_squared()
        };
        assert!(
            selected
                .windows(2)
                .all(|pair| distance(pair[0]) <= distance(pair[1]))
        );
        let farthest = distance(*selected.last().unwrap());
        assert!(
            cells
                .iter()
                .filter(|cell| !selected.contains(cell))
                .all(|cell| distance(*cell) >= farthest)
        );
    }

    #[test]
    fn empty_active_empty_keeps_nonempty_mesh_buffers_and_hides_empty_effects() {
        let mut mesh = mesh();
        assert_eq!(mesh.count_vertices(), 3);
        assert!(!Geometry::default().write(&mut mesh));
        assert_eq!(mesh.count_vertices(), 3);
        let mut fire = Geometry::default();
        fire.cuboid(Vec3::ZERO, Vec3::splat(0.1), Quat::IDENTITY, [1.0; 4]);
        assert!(fire.write(&mut mesh));
        assert_eq!(mesh.count_vertices(), 24);
        assert!(!Geometry::default().write(&mut mesh));
        assert_eq!(mesh.count_vertices(), 24);
        assert_eq!(mesh.indices().unwrap().len(), 36);
    }

    #[test]
    fn particle_faces_are_outward_and_geometry_is_indexed() {
        let mut geometry = Geometry::default();
        geometry.cuboid(Vec3::ZERO, Vec3::splat(0.5), Quat::IDENTITY, [1.0; 4]);
        assert_eq!(geometry.positions.len(), 24);
        for triangle in geometry.indices.chunks_exact(3) {
            let points: [Vec3; 3] =
                std::array::from_fn(|i| Vec3::from_array(geometry.positions[triangle[i] as usize]));
            let normal = Vec3::from_array(geometry.normals[triangle[0] as usize]);
            assert!(
                (points[1] - points[0])
                    .cross(points[2] - points[0])
                    .dot(normal)
                    > 0.0
            );
        }
    }

    #[test]
    fn smoke_uses_one_soft_quad_per_wisp_not_six_cube_faces() {
        let mut geometry = Geometry::default();
        geometry.wisp(Vec3::ZERO, 0.35, Vec3::X, Vec3::Y, [0.2; 4]);
        assert_eq!(geometry.positions.len(), 4);
        assert_eq!(geometry.indices, [0, 1, 2, 0, 2, 3]);
        assert_eq!(geometry.uvs.len(), geometry.positions.len());
        assert!(
            geometry
                .positions
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
        assert!(
            geometry
                .positions
                .iter()
                .all(|p| { p.iter().all(|axis| axis.abs() < 0.5) })
        );
        let mut target = mesh();
        assert!(geometry.write(&mut target));
        assert_eq!(target.count_vertices(), 4);
    }

    #[test]
    fn outlet_wisps_cross_open_air_but_not_a_roof_or_unloaded_wall() {
        let mut geometry = Geometry::default();
        geometry.wisp(Vec3::new(0.9, 0.9, 0.5), 0.4, Vec3::X, Vec3::Y, [0.2; 4]);
        geometry.confine_last_wisp([0; 3], |cell| cell[0] == 1);
        assert!(geometry.positions.iter().all(|p| p[1] <= 0.998));
        assert!(geometry.positions.iter().any(|p| p[0] > 1.0));
        assert!(
            geometry
                .positions
                .iter()
                .all(|p| p.iter().all(|value| value.is_finite()))
        );
    }

    #[test]
    fn room_haze_increases_with_smoke_without_probing_empty_air_or_crossing_walls() {
        let eye = Vec3::splat(0.5);
        let empty = BTreeSet::new();
        let light = BTreeSet::from([[0, 0, 0]]);
        let dense = (-2..=2)
            .flat_map(|x| (-2..=2).flat_map(move |y| (-2..=2).map(move |z| [x, y, z])))
            .collect();
        assert_eq!(
            smoke_density(eye, &empty, |_| panic!(
                "Empty smoke must not probe terrain"
            )),
            0.0
        );
        assert!(smoke_density(eye, &light, |_| true) > 0.0);
        assert!(smoke_density(eye, &dense, |_| true) > smoke_density(eye, &light, |_| true));
        let outside = BTreeSet::from([[2, 0, 0], [2, 1, 0]]);
        assert_eq!(smoke_density(eye, &outside, |cell| cell[0] != 1), 0.0);
        assert!(smoke_density(eye, &outside, |_| true) > 0.0);
        assert_eq!(smoke_density(eye, &dense, |_| false), 0.0);
    }
}
