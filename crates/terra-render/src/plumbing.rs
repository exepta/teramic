//! Confirmed finite-water contents share one bounded mesh; node models stay static.
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_network::client::ClientSession;
use terra_player::TerraCamera;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

mod basin;
pub(crate) mod glass;
pub(crate) mod infusion;
pub(crate) mod intake;
pub(crate) mod jet;
mod level;

const MAX_NODES: usize = 48;

#[derive(Resource)]
struct Effects {
    entity: Entity,
    mesh: Handle<Mesh>,
    water_entity: Entity,
    water_mesh: Handle<Mesh>,
    water_geometry: Geometry,
    next: f32,
    geometry: Geometry,
    intakes: std::collections::HashMap<[i64; 3], intake::Pulse>,
    glass_shapes: glass::Shapes,
}

#[derive(Default, PartialEq)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Geometry {
    fn quad(&mut self, p: [Vec3; 4], normal: Vec3, color: [f32; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend(p.map(|point| point.to_array()));
        self.normals.extend([normal.to_array(); 4]);
        self.colors.extend([color; 4]);
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    fn cuboid(&mut self, minimum: Vec3, maximum: Vec3) {
        for axis in 0..3 {
            for positive in [false, true] {
                let u = (axis + 1) % 3;
                let v = (axis + 2) % 3;
                let mut normal = Vec3::ZERO;
                normal[axis] = if positive { 1. } else { -1. };
                let mut points = std::array::from_fn(|index| {
                    let mut p = minimum;
                    p[axis] = if positive {
                        maximum[axis]
                    } else {
                        minimum[axis]
                    };
                    p[u] = if index == 1 || index == 2 {
                        maximum[u]
                    } else {
                        minimum[u]
                    };
                    p[v] = if index >= 2 { maximum[v] } else { minimum[v] };
                    p
                });
                if !positive {
                    points.reverse();
                }
                self.quad(
                    points,
                    normal,
                    [
                        (maximum - minimum).min_element().clamp(0.015, 0.22),
                        0.,
                        1.,
                        7.,
                    ],
                );
            }
        }
    }
    fn water_mesh(&self, terrain: &terra_world::TerraGenerator) -> Mesh {
        use crate::water::{
            biome::{ATTRIBUTE_BIOME_TINT, ATTRIBUTE_MEDIUM_WEIGHTS, WaterTint},
            material::ATTRIBUTE_WAVE_WEIGHT,
        };
        let mut tints = std::collections::HashMap::new();
        let (colors, medium_weights): (Vec<_>, Vec<_>) = self
            .positions
            .iter()
            .map(|p| {
                let origin = [
                    (p[0].floor() as i64).div_euclid(16) * 16,
                    0,
                    (p[2].floor() as i64).div_euclid(16) * 16,
                ];
                let tint = tints
                    .entry(origin)
                    .or_insert_with(|| WaterTint::new(terrain, origin));
                let local = [p[0] - origin[0] as f32, 0., p[2] - origin[2] as f32];
                (tint.payload(local), tint.medium_at(local))
            })
            .unzip();
        self.mesh()
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_UV_0,
                self.colors
                    .iter()
                    .map(|c| [c[0], if c[3] == 8. { c[1] } else { 0. }])
                    .collect::<Vec<_>>(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_UV_1,
                self.colors.iter().map(|c| [c[1], c[2]]).collect::<Vec<_>>(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_COLOR,
                self.colors
                    .iter()
                    .map(|c| [0., 0., 0., c[3]])
                    .collect::<Vec<_>>(),
            )
            .with_inserted_attribute(ATTRIBUTE_BIOME_TINT, colors)
            .with_inserted_attribute(ATTRIBUTE_MEDIUM_WEIGHTS, medium_weights)
            .with_inserted_attribute(ATTRIBUTE_WAVE_WEIGHT, vec![0.; self.positions.len()])
    }
    fn mesh(&self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors.clone())
        .with_inserted_indices(Indices::U32(self.indices.clone()))
    }
}

pub(crate) fn install(app: &mut App) {
    basin::install(app);
    infusion::install(app);
    app.init_resource::<intake::IntakeRenderStatus>();
    app.init_resource::<glass::GlassPipeRenderStatus>();
    app.init_resource::<jet::InfuserJetRenderStatus>();
    app.add_systems(Startup, setup.after(crate::particle_fluids::setup_surface))
        .add_systems(Update, update);
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    water: Res<crate::particle_fluids::HydraulicMaterials>,
    terrain: Res<terra_world::streaming::WorldTerrain>,
) {
    let placeholder = Geometry {
        positions: vec![[0.; 3]; 3],
        normals: vec![[0., 1., 0.]; 3],
        colors: vec![[0.; 4]; 3],
        indices: vec![0, 1, 2],
    };
    let mesh = meshes.add(placeholder.mesh());
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.13,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    let entity = commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            Name::new("Finite hydraulic water (shared)"),
        ))
        .id();
    let water_mesh = meshes.add(placeholder.water_mesh(&terrain.0));
    let water_entity = commands
        .spawn((
            Mesh3d(water_mesh.clone()),
            MeshMaterial3d::<crate::particle_fluids::FluidMaterial>(water.0[0].clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            crate::miniature::layers(),
            terra_core::miniature::MiniatureMesh::Water,
            Name::new("Contained hydraulic water (shared natural material)"),
        ))
        .id();
    commands.insert_resource(Effects {
        entity,
        mesh,
        water_entity,
        water_mesh,
        water_geometry: Geometry::default(),
        next: 0.,
        geometry: Geometry::default(),
        intakes: Default::default(),
        glass_shapes: Default::default(),
    });
}

/// The jet ends at the same loaded geometry/water that can receive its output.
fn stream_length(
    mouth: Vec3,
    tap: [i64; 3],
    radius: f32,
    session: Option<&ClientSession>,
    (chunks, journal, terrain, properties): (
        &ChunkStore,
        &EditJournal,
        &terra_world::TerraGenerator,
        &terra_core::block_properties::BlockPropertiesCatalog,
    ),
) -> f32 {
    for depth in 1..=32 {
        let cell = [tap[0], tap[1] - depth, tap[2]];
        let Some(raw) = chunks.block_at(cell) else {
            return (mouth.y - (cell[1] + 1) as f32).max(0.0);
        };
        let material = journal.block(cell).unwrap_or(raw);
        if material == terra_core::plumbing::BASIN
            && let Some(node) = session
                .into_iter()
                .flat_map(|s| &s.plumbing)
                .find(|node| node.basin.is_some_and(|shape| shape.contains(cell)))
            && let Some(shape) = node.basin
        {
            let height = terra_world::plumbing::basin_water_level(shape, node.amount) as f32;
            return (mouth.y - height).max(0.0);
        }
        let fluid = chunks.fluid_cell_at(terrain, journal, cell, material);
        if fluid.amount > 0 {
            return (mouth.y - (cell[1] as f32 + fluid.height() as f32)).max(0.0);
        }
        let mut top = f32::NEG_INFINITY;
        let shaped = terra_world::interaction_shapes::visit_boxes(
            chunks,
            journal,
            properties,
            cell,
            |bounds| {
                if bounds.minimum[0] < f64::from(mouth.x + radius)
                    && bounds.maximum[0] > f64::from(mouth.x - radius)
                    && bounds.minimum[2] < f64::from(mouth.z + radius)
                    && bounds.maximum[2] > f64::from(mouth.z - radius)
                {
                    top = top.max(bounds.maximum[1] as f32);
                }
            },
        );
        if top.is_finite() {
            return (mouth.y - top).max(0.0);
        }
        if !shaped && !terra_world::water::can_hold_fluid(material) {
            return (mouth.y - (cell[1] + 1) as f32).max(0.0);
        }
    }
    32.0
}

#[derive(bevy::ecs::system::SystemParam)]
struct WaterWorld<'w> {
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    terrain: Res<'w, terra_world::streaming::WorldTerrain>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    intake_status: ResMut<'w, intake::IntakeRenderStatus>,
    glass_status: ResMut<'w, glass::GlassPipeRenderStatus>,
    jet_status: ResMut<'w, jet::InfuserJetRenderStatus>,
}

fn update(
    time: Res<Time>,
    session: Option<Res<ClientSession>>,
    world: WaterWorld,
    cameras: Query<&Transform, With<TerraCamera>>,
    mut effects: ResMut<Effects>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visibility: Query<&mut Visibility>,
) {
    let WaterWorld {
        chunks,
        journal,
        terrain,
        properties,
        mut intake_status,
        mut glass_status,
        mut jet_status,
    } = world;
    let now = time.elapsed_secs();
    if now < effects.next {
        return;
    }
    effects.next = now + 0.05;
    let eye = cameras
        .iter()
        .next()
        .map_or(Vec3::ZERO, |camera| camera.translation);
    let mut sources: Vec<_> = session
        .as_deref()
        .filter(|session| session.ready)
        .into_iter()
        .flat_map(|session| &session.plumbing)
        .filter_map(|node| {
            let raw = chunks.block_at(node.position)?;
            let material = journal.block(node.position).unwrap_or(raw);
            let distance = Vec3::from_array(node.position.map(|v| v as f32)).distance_squared(eye);
            (matches!(
                material,
                terra_core::plumbing::FAUCET
                    | terra_core::plumbing::PUMP
                    | terra_core::infusion::FLUID_INFUSER
                    | terra_core::plumbing::GLASS_PIPE
            ) && distance < 96.0_f32.powi(2))
            .then_some((node, material, distance))
        })
        .collect();
    sources.sort_by(|a, b| a.2.total_cmp(&b.2));
    sources.truncate(MAX_NODES);
    let mut next = Geometry::default();
    let mut bubbles = Geometry::default();
    *intake_status = intake::append(
        &mut bubbles,
        &mut effects.intakes,
        now,
        session.as_deref(),
        eye,
        (&chunks, &journal, &terrain.0),
    );
    *glass_status = glass::GlassPipeRenderStatus {
        plumbing_sequence: session.as_deref().map_or(0, |s| s.plumbing_sequence),
        ..Default::default()
    };
    *jet_status = jet::InfuserJetRenderStatus {
        plumbing_sequence: session.as_deref().map_or(0, |s| s.plumbing_sequence),
        ..Default::default()
    };
    for (node, material, _) in sources {
        let anchor = Vec3::from_array(node.position.map(|v| v as f32));
        if material == terra_core::plumbing::GLASS_PIPE {
            glass_status.pipes += 1;
            let vertices = glass::append(
                &mut next,
                &mut effects.glass_shapes,
                node.position,
                glass::Liquid {
                    amount: node.amount,
                    flowing: node.flowing,
                    neighbors: std::array::from_fn(|side| {
                        let p = std::array::from_fn(|axis| {
                            node.position[axis] + terra_core::plumbing::DIRECTIONS[side][axis]
                        });
                        session
                            .as_deref()
                            .and_then(|s| s.plumbing.iter().find(|other| other.position == p))
                            .map(|other| (other.amount.min(8) as u8, other.flowing))
                    }),
                },
                &chunks,
                &journal,
            );
            glass_status.nodes.push(glass::GlassPipeNodeRenderStatus {
                position: node.position,
                amount: node.amount,
                flowing: node.flowing,
                vertices,
            });
            if node.flowing {
                glass_status.flowing += 1;
                if node.amount == 0 {
                    glass_status.transit_vertices += vertices;
                }
            }
            if node.amount > 0 && vertices > 0 {
                glass_status.filled += 1;
                glass_status.units += node.amount.min(8);
                glass_status.vertices += vertices;
            }
        } else if material == 168 && node.amount > 0 {
            let height = 0.30 + 0.39 * (f32::from(node.amount.min(8)) / 8.);
            next.cuboid(
                anchor + Vec3::new(0.28, 0.30, 0.29),
                anchor + Vec3::new(0.72, height, 0.71),
            );
        } else if material == terra_core::infusion::FLUID_INFUSER && node.supplied {
            let hold = terra_core::infusion::hold_below(node.position);
            let gap = terra_core::infusion::gap_above(hold);
            if chunks
                .block_at(gap)
                .map(|raw| journal.block(gap).unwrap_or(raw))
                != Some(0)
            {
                continue;
            }
            if let Some(target) = session
                .as_deref()
                .and_then(|session| jet::target(session, node, &journal))
            {
                let mouth = anchor + Vec3::new(0.5, 0.012, 0.5);
                let vertices = jet::append(&mut next, mouth, target, now);
                jet_status.streams += usize::from(vertices > 0);
                jet_status.vertices += vertices;
            }
        } else if material == 167
            && node.supplied
            && node.open
            && node.flowing
            && terra_core::plumbing::open(
                material,
                journal
                    .object_state(node.position)
                    .unwrap_or_default()
                    .facing,
            )
        {
            let facing = journal
                .object_state(node.position)
                .unwrap_or_default()
                .facing
                & 3;
            let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
            let mouth = anchor + Vec3::splat(0.5) + rotation * Vec3::new(0., -0.225, 0.13);
            let level = terra_core::plumbing::faucet_level(
                journal
                    .object_state(node.position)
                    .unwrap_or_default()
                    .facing,
            );
            let radius = 0.014 + f32::from(level) * 0.008;
            let length = stream_length(
                mouth,
                node.position,
                radius,
                session.as_deref(),
                (&chunks, &journal, &terrain.0, &properties),
            );
            let drops = (length * 8.0).ceil().clamp(1.0, 128.0) as usize;
            for drop in 0..drops {
                let progress = (now * (1.5 + f32::from(level)) / length.max(0.1)
                    + drop as f32 / drops as f32)
                    .fract();
                let distance = progress * length;
                let half_height = (length / drops as f32 * 0.42).min(0.07);
                let center = mouth - Vec3::Y * distance;
                next.cuboid(
                    center
                        - Vec3::new(
                            radius,
                            half_height.min((length - distance).max(0.0)),
                            radius,
                        ),
                    center + Vec3::new(radius, half_height.min(distance), radius),
                );
            }
        }
    }
    let wet = !next.indices.is_empty();
    if next != effects.water_geometry {
        if wet && let Some(mut mesh) = meshes.get_mut(&effects.water_mesh) {
            *mesh = next.water_mesh(&terrain.0);
        }
        effects.water_geometry = next;
    }
    let visible = !bubbles.indices.is_empty();
    if bubbles != effects.geometry {
        if visible && let Some(mut mesh) = meshes.get_mut(&effects.mesh) {
            *mesh = bubbles.mesh();
        }
        effects.geometry = bubbles;
    }
    for (entity, shown) in [(effects.entity, visible), (effects.water_entity, wet)] {
        if let Ok(mut state) = visibility.get_mut(entity) {
            let next = if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *state != next {
                *state = next;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_shared_water_faces_are_finite_and_outward_with_bounded_payload() {
        let mut geometry = Geometry::default();
        geometry.cuboid(Vec3::ZERO, Vec3::ONE);
        assert_eq!(geometry.positions.len(), 24);
        assert_eq!(geometry.indices.len(), 36);
        for triangle in geometry.indices.chunks_exact(3) {
            let [a, b, c] =
                std::array::from_fn(|i| Vec3::from_array(geometry.positions[triangle[i] as usize]));
            let n = Vec3::from_array(geometry.normals[triangle[0] as usize]);
            assert!((b - a).cross(c - a).dot(n) > 0.);
        }
        assert!(geometry.positions.iter().flatten().all(|v| v.is_finite()));
    }
    #[test]
    fn hydraulic_water_uses_shared_confined_fluid_optics_without_container_waves() {
        use crate::water::{biome::ATTRIBUTE_BIOME_TINT, material::ATTRIBUTE_WAVE_WEIGHT};
        let mut app = App::new();
        let handle = Handle::<crate::particle_fluids::FluidMaterial>::default();
        let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(crate::particle_fluids::HydraulicMaterials(
                std::array::from_fn(|_| handle.clone()),
            ))
            .insert_resource(terra_world::streaming::WorldTerrain(terrain))
            .add_systems(Startup, setup);
        app.update();
        let entity = app.world().resource::<Effects>().water_entity;
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<crate::particle_fluids::FluidMaterial>>(entity)
                .unwrap()
                .0,
            handle
        );
        assert!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(entity)
                .is_none()
        );
        let mut geometry = Geometry::default();
        geometry.cuboid(Vec3::new(-16.1, 5., -0.1), Vec3::new(-15.9, 5.1, 0.1));
        let mesh = geometry.water_mesh(
            &app.world()
                .resource::<terra_world::streaming::WorldTerrain>()
                .0,
        );
        for attr in [
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_UV_1,
            Mesh::ATTRIBUTE_COLOR,
            ATTRIBUTE_BIOME_TINT,
            ATTRIBUTE_WAVE_WEIGHT,
        ] {
            assert!(mesh.attribute(attr).is_some());
        }
        assert!(
            matches!(mesh.attribute(ATTRIBUTE_WAVE_WEIGHT), Some(bevy::mesh::VertexAttributeValues::Float32(values)) if values.iter().all(|&v| v == 0.))
        );
        assert!(
            matches!(mesh.attribute(ATTRIBUTE_BIOME_TINT), Some(bevy::mesh::VertexAttributeValues::Float32x4(values)) if values.len() == geometry.positions.len() && values.iter().all(|v| (0.0..=1.0).contains(&v[3])))
        );
        let startup_mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&app.world().resource::<Effects>().water_mesh)
            .unwrap();
        assert!(matches!(
            startup_mesh.attribute(ATTRIBUTE_BIOME_TINT),
            Some(bevy::mesh::VertexAttributeValues::Float32x4(_))
        ));
        assert!(
            matches!(mesh.attribute(Mesh::ATTRIBUTE_COLOR), Some(bevy::mesh::VertexAttributeValues::Float32x4(values)) if values.iter().all(|v| *v == [0.,0.,0.,7.]))
        );
    }

    #[test]
    fn faucet_jet_reaches_a_lower_basin_surface_and_stops_at_an_intervening_block() {
        use terra_voxel::{BlockSection, SectionPos, global_to_section};
        use terra_world::streaming::LoadedSection;
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            sea_level: -100.0,
            ..Default::default()
        })
        .unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let section = SectionPos::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let basin = [0, 2, 0];
        let (_, local) = global_to_section(basin);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 169)
            .unwrap();
        journal.record(basin, 169).unwrap();
        journal
            .set_object_state(basin, Some(Default::default()))
            .unwrap();
        let mut session = ClientSession::default();
        session
            .plumbing
            .push(terra_network::protocol::session::PlumbingVisual {
                intake: None,
                position: basin,
                amount: 0,
                capacity: 8,
                open: true,
                flowing: false,
                supplied: false,
                pump_rpm: 0.0,
                basin: Some(terra_core::plumbing::BasinShape {
                    origin: basin,
                    size: [1; 3],
                }),
            });
        let mouth = Vec3::new(0.5, 8.275, 0.63);
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        let context = (&chunks, &journal, &terrain, &properties);
        let length = stream_length(mouth, [0, 8, 0], 0.03, Some(&session), context);
        assert!((length - (8.275 - 2.13)).abs() < 0.0001);
        let (_, local) = global_to_section([0, 5, 0]);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        assert!(
            (stream_length(
                mouth,
                [0, 8, 0],
                0.03,
                Some(&session),
                (&chunks, &journal, &terrain, &properties)
            ) - (8.275 - 6.0))
                .abs()
                < 0.0001
        );
    }
}
