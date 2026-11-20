//! Reservoir surfaces use the exact material and vertex contracts of river water.
use crate::particle_fluids::{FluidMaterial, HydraulicMaterials};
use crate::water::{
    biome::{ATTRIBUTE_BIOME_TINT, ATTRIBUTE_BLOOD_WEIGHT, WaterTint},
    material::ATTRIBUTE_WAVE_WEIGHT,
};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_network::client::ClientSession;
use terra_player::TerraCamera;
use terra_world::{
    TerraGenerator,
    streaming::{ChunkStore, WorldTerrain},
};

#[derive(Default, PartialEq)]
struct SurfaceGeometry {
    positions: Vec<[f32; 3]>,
    depth_shore: Vec<[f32; 2]>,
    tints: Vec<[f32; 4]>,
    blood: Vec<f32>,
    waves: Vec<f32>,
    indices: Vec<u32>,
}
impl SurfaceGeometry {
    fn mesh(&self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions.clone())
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![[0.0, 1.0, 0.0]; self.positions.len()],
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.depth_shore.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.0; 2]; self.positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0; 4]; self.positions.len()])
        .with_inserted_attribute(ATTRIBUTE_BIOME_TINT, self.tints.clone())
        .with_inserted_attribute(ATTRIBUTE_BLOOD_WEIGHT, self.blood.clone())
        .with_inserted_attribute(ATTRIBUTE_WAVE_WEIGHT, self.waves.clone())
        .with_inserted_indices(Indices::U32(self.indices.clone()))
    }
    fn append(
        &mut self,
        shape: terra_core::plumbing::BasinShape,
        amount: u16,
        chunks: &ChunkStore,
        terrain: &TerraGenerator,
    ) {
        if !shape.valid() || amount == 0 {
            return;
        }
        let level = terra_world::plumbing::basin_water_level(shape, amount);
        let bed = shape.origin[1] as f64 + 0.13;
        let layer = level.floor() as i64;
        let mut tints = std::collections::HashMap::new();
        for x in 0..shape.size[0] {
            for z in 0..shape.size[2] {
                let cell = [
                    shape.origin[0] + i64::from(x),
                    layer,
                    shape.origin[2] + i64::from(z),
                ];
                if chunks.block_at(cell) != Some(terra_core::plumbing::BASIN) {
                    continue;
                }
                let lo = [
                    cell[0] as f32 + if x == 0 { 0.16 } else { 0.0 },
                    cell[2] as f32 + if z == 0 { 0.16 } else { 0.0 },
                ];
                let hi = [
                    cell[0] as f32 + 1.0 - if x + 1 == shape.size[0] { 0.16 } else { 0.0 },
                    cell[2] as f32 + 1.0 - if z + 1 == shape.size[2] { 0.16 } else { 0.0 },
                ];
                // Half-meter quads anchor the shell while interior waves remain live.
                for dx in 0..2 {
                    for dz in 0..2 {
                        let bounds = [
                            [
                                lo[0] + (hi[0] - lo[0]) * dx as f32 * 0.5,
                                lo[1] + (hi[1] - lo[1]) * dz as f32 * 0.5,
                            ],
                            [
                                lo[0] + (hi[0] - lo[0]) * (dx + 1) as f32 * 0.5,
                                lo[1] + (hi[1] - lo[1]) * (dz + 1) as f32 * 0.5,
                            ],
                        ];
                        let base = self.positions.len() as u32;
                        for [px, pz] in [
                            [bounds[0][0], bounds[0][1]],
                            [bounds[0][0], bounds[1][1]],
                            [bounds[1][0], bounds[1][1]],
                            [bounds[1][0], bounds[0][1]],
                        ] {
                            let p = [px, level as f32, pz];
                            let origin = [
                                (px.floor() as i64).div_euclid(16) * 16,
                                0,
                                (pz.floor() as i64).div_euclid(16) * 16,
                            ];
                            let tint = tints
                                .entry(origin)
                                .or_insert_with(|| WaterTint::new(terrain, origin));
                            self.tints.push(tint.payload([
                                px - origin[0] as f32,
                                0.0,
                                pz - origin[2] as f32,
                            ]));
                            self.blood.push(tint.blood_at([
                                px - origin[0] as f32,
                                0.0,
                                pz - origin[2] as f32,
                            ]));
                            let edge = (px - (shape.origin[0] as f32 + 0.16))
                                .min(shape.origin[0] as f32 + f32::from(shape.size[0]) - 0.16 - px)
                                .min(pz - (shape.origin[2] as f32 + 0.16))
                                .min(shape.origin[2] as f32 + f32::from(shape.size[2]) - 0.16 - pz);
                            self.positions.push(p);
                            self.depth_shore
                                .push([(level - bed) as f32, (1.0 - edge / 0.65).clamp(0.0, 1.0)]);
                            self.waves.push((edge / 0.65).clamp(0.0, 1.0));
                        }
                        self.indices
                            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                    }
                }
            }
        }
    }
}
#[derive(Resource)]
struct BasinSurface {
    entity: Entity,
    mesh: Handle<Mesh>,
    geometry: SurfaceGeometry,
    inputs: Vec<(terra_core::plumbing::BasinShape, u16, u32)>,
    next: f32,
}

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, setup.after(crate::particle_fluids::setup_surface))
        .add_systems(Update, update);
}
fn setup(
    mut commands: Commands,
    assets: Res<HydraulicMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mesh = meshes.add(
        SurfaceGeometry {
            positions: vec![[0.0; 3]; 3],
            depth_shore: vec![[0.0; 2]; 3],
            tints: vec![[0.0; 4]; 3],
            blood: vec![0.0; 3],
            waves: vec![0.0; 3],
            indices: vec![0, 1, 2],
        }
        .mesh(),
    );
    let entity = commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d::<FluidMaterial>(assets.0[0].clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            crate::miniature::layers(),
            terra_core::miniature::MiniatureMesh::Water,
            Name::new("Finite basin water (river material)"),
        ))
        .id();
    commands.insert_resource(BasinSurface {
        entity,
        mesh,
        geometry: SurfaceGeometry::default(),
        inputs: vec![],
        next: 0.0,
    });
}
fn update(
    time: Res<Time>,
    session: Option<Res<ClientSession>>,
    world: (
        Res<ChunkStore>,
        Res<WorldTerrain>,
        Res<terra_core::GameConfig>,
    ),
    cameras: Query<&Transform, With<TerraCamera>>,
    mut surface: ResMut<BasinSurface>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visibility: Query<&mut Visibility>,
) {
    if time.elapsed_secs() < surface.next {
        return;
    }
    surface.next = time.elapsed_secs() + 0.1;
    let (chunks, terrain, config) = world;
    let eye = cameras
        .iter()
        .next()
        .map_or(Vec3::ZERO, |camera| camera.translation);
    let mut inputs = vec![];
    let mut seen = std::collections::HashSet::new();
    if config.world.water {
        for node in session
            .as_deref()
            .filter(|session| session.ready)
            .into_iter()
            .flat_map(|session| &session.plumbing)
        {
            let Some(shape) = node.basin else {
                continue;
            };
            if seen.len() >= 48
                || !seen.insert(shape.origin)
                || Vec3::from_array(shape.origin.map(|v| v as f32)).distance_squared(eye)
                    > 96.0_f32.powi(2)
            {
                continue;
            }
            let level = terra_world::plumbing::basin_water_level(shape, node.amount).floor() as i64;
            let mut mask = 0_u32;
            if shape.valid() {
                for x in 0..shape.size[0] {
                    for z in 0..shape.size[2] {
                        let cell = [
                            shape.origin[0] + i64::from(x),
                            level,
                            shape.origin[2] + i64::from(z),
                        ];
                        if chunks.block_at(cell) == Some(terra_core::plumbing::BASIN) {
                            mask |= 1 << (u32::from(x) + u32::from(z) * 5);
                        }
                    }
                }
            }
            inputs.push((shape, node.amount, mask));
        }
    }
    inputs.sort_unstable();
    if surface.inputs == inputs && !terrain.is_changed() {
        return;
    }
    let mut next = SurfaceGeometry::default();
    for &(shape, amount, _) in &inputs {
        next.append(shape, amount, &chunks, &terrain.0);
    }
    surface.inputs = inputs;
    let visible = !next.indices.is_empty();
    if next != surface.geometry {
        if visible && let Some(mut mesh) = meshes.get_mut(&surface.mesh) {
            *mesh = next.mesh();
        }
        surface.geometry = next;
    }
    if let Ok(mut v) = visibility.get_mut(surface.entity) {
        let state = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != state {
            *v = state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn basin_surface_has_natural_water_depth_tint_and_anchored_wave_attributes() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let shape = terra_core::plumbing::BasinShape {
            origin: [-1, 192, -1],
            size: [2, 3, 2],
        };
        let mut chunks = ChunkStore::default();
        for cell in shape.cells() {
            let (section, local) = global_to_section(cell);
            let section = chunks
                .loaded
                .entry(section)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                });
            section
                .data
                .set_block(local, terra_core::plumbing::BASIN)
                .unwrap();
        }
        let mut geometry = SurfaceGeometry::default();
        geometry.append(shape, shape.capacity(), &chunks, &terrain);
        assert_eq!(geometry.positions.len(), 64);
        let level = terra_world::plumbing::basin_water_level(shape, shape.capacity()) as f32;
        assert!(geometry.positions.iter().all(|p| p[1] == level));
        assert!(
            geometry
                .depth_shore
                .iter()
                .all(|uv| (uv[0] - 2.79).abs() < 1e-5)
        );
        for (p, tint) in geometry.positions.iter().zip(&geometry.tints) {
            let origin = [
                (p[0].floor() as i64).div_euclid(16) * 16,
                0,
                (p[2].floor() as i64).div_euclid(16) * 16,
            ];
            assert_eq!(
                *tint,
                WaterTint::new(&terrain, origin).payload([
                    p[0] - origin[0] as f32,
                    0.0,
                    p[2] - origin[2] as f32
                ])
            );
        }
        assert!(geometry.waves.contains(&0.0));
        assert!(geometry.waves.iter().any(|&weight| weight > 0.9));
        for indices in geometry.indices.chunks_exact(3) {
            let [a, b, c] =
                std::array::from_fn(|i| Vec3::from_array(geometry.positions[indices[i] as usize]));
            assert!((b - a).cross(c - a).y > 0.0);
        }
        let mesh = geometry.mesh();
        assert!(
            matches!(mesh.attribute(ATTRIBUTE_BIOME_TINT), Some(bevy::mesh::VertexAttributeValues::Float32x4(values)) if values.len() == geometry.positions.len())
        );
        for attribute in [
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_UV_1,
            Mesh::ATTRIBUTE_COLOR,
            ATTRIBUTE_BIOME_TINT,
            ATTRIBUTE_BLOOD_WEIGHT,
            ATTRIBUTE_WAVE_WEIGHT,
        ] {
            assert!(mesh.attribute(attribute).is_some());
        }
    }

    #[test]
    fn reservoir_entity_uses_the_shared_confined_fluid_material_handle() {
        let mut app = App::new();
        let handle = Handle::<FluidMaterial>::default();
        app.init_resource::<Assets<Mesh>>()
            .insert_resource(HydraulicMaterials(std::array::from_fn(|_| handle.clone())))
            .add_systems(Startup, setup);
        app.update();
        let startup_mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&app.world().resource::<BasinSurface>().mesh)
            .unwrap();
        assert!(
            matches!(startup_mesh.attribute(ATTRIBUTE_BIOME_TINT), Some(bevy::mesh::VertexAttributeValues::Float32x4(values)) if values == &vec![[0.; 4]; 3])
        );
        let entity = app.world().resource::<BasinSurface>().entity;
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<FluidMaterial>>(entity)
                .unwrap()
                .0,
            handle
        );
        assert!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(entity)
                .is_none()
        );
    }
}
