//! Moving breakers at the actual contact between a marine water column and coastal rock.
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
use std::collections::BTreeMap;
use terra_core::{GameConfig, WaterQuality};
use terra_voxel::{MaterialId as M, SectionPos};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct Foam {
    #[uniform(0)]
    light_and_strength: Vec4,
}
impl Material for Foam {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_surf.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_shadows() -> bool {
        false
    }
    fn enable_prepass() -> bool {
        false
    }
    fn specialize(
        _: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
        ])?];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct Edge {
    rock: [i64; 2],
    normal: [i64; 2],
}
// Include neighbours because a rock and its water can stream in different sections.
type Stamp = [[u64; 3]; 10];
#[derive(Resource, Default)]
struct Surf {
    tiles: BTreeMap<[i64; 2], (Stamp, Vec<Edge>)>,
    mesh: Option<Handle<Mesh>>,
    last: f32,
}
#[derive(Component)]
struct Breakers;
pub(super) fn install(app: &mut App) {
    crate::material_work::add::<Foam>(app)
        .init_resource::<Surf>()
        .add_systems(Startup, setup)
        .add_systems(Update, refresh);
}
fn setup(mut commands: Commands, mut materials: ResMut<Assets<Foam>>) {
    commands.spawn((
        Name::new("Waves breaking on coastal rock"),
        Breakers,
        MeshMaterial3d(materials.add(Foam {
            light_and_strength: Vec4::ONE,
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        bevy::camera::visibility::NoFrustumCulling,
    ));
}
const DIRECTIONS: [[i64; 2]; 4] = [[1, 0], [-1, 0], [0, 1], [0, -1]];
const ROCKS: [M; 5] = [
    M::CoastalGranite,
    M::TidewornRock,
    M::SaltStainedStone,
    M::LichenCoastalStone,
    M::MossyCoastalStone,
];

fn signature(chunks: &ChunkStore, journal: &EditJournal, tile: [i64; 2], y: i64) -> Stamp {
    std::array::from_fn(|i| {
        let [dx, dz] = if i % 5 == 4 {
            [0, 0]
        } else {
            DIRECTIONS[i % 5]
        };
        let section = SectionPos::new(
            tile[0] + dx,
            (y - (i / 5) as i64).div_euclid(16),
            tile[1] + dz,
        );
        chunks
            .loaded
            .get(&section)
            .map_or([0; 3], |s| [1, s.mesh_epoch, journal.revision(section)])
    })
}
fn survey(chunks: &ChunkStore, journal: &EditJournal, tile: [i64; 2], sea: f64) -> Vec<Edge> {
    let y = sea.floor() as i64;
    let beach = |yy: i64| {
        chunks
            .loaded
            .get(&SectionPos::new(tile[0], yy.div_euclid(16), tile[1]))
            .is_some_and(|s| ROCKS.iter().any(|m| s.data.may_contain(*m as u16)))
    };
    if !beach(y) && !beach(y - 1) {
        return Vec::new();
    }
    let block = |x, y, z| {
        journal
            .block([x, y, z])
            .or_else(|| chunks.block_at([x, y, z]))
    };
    let marine_water = |x, z| {
        chunks.surface_column_at(x, z).is_some_and(|c| {
            c.biome.weights.ocean > 0.18
                && c.water_columns()
                    .at(y - 1)
                    .is_some_and(|w| (w.level - sea).abs() < 0.05)
        })
    };
    wet_rock_edges(tile, y, block, marine_water)
}

fn wet_rock_edges(
    tile: [i64; 2],
    y: i64,
    block: impl Fn(i64, i64, i64) -> Option<u16>,
    marine_water: impl Fn(i64, i64) -> bool,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for x in tile[0] * 16..tile[0] * 16 + 16 {
        for z in tile[1] * 16..tile[1] * 16 + 16 {
            let is_rock =
                |yy| block(x, yy, z).is_some_and(|id| ROCKS.iter().any(|m| *m as u16 == id));
            if !is_rock(y) && !is_rock(y - 1) {
                continue;
            }
            for [dx, dz] in DIRECTIONS {
                let wx = x + dx;
                let wz = z + dz;
                if !matches!(block(wx, y, wz), Some(0)) || !matches!(block(wx, y - 1, wz), Some(0))
                {
                    continue;
                }
                if marine_water(wx, wz) {
                    edges.push(Edge {
                        rock: [x, z],
                        normal: [dx, dz],
                    });
                }
            }
        }
    }
    edges
}

type Environment<'w> = (
    Res<'w, Time>,
    Res<'w, GameConfig>,
    Res<'w, WorldAccess>,
    Res<'w, WorldTerrain>,
    Res<'w, ChunkStore>,
    Res<'w, EditJournal>,
    Option<Res<'w, crate::scene::daylight::Daylight>>,
);
fn refresh(
    (time, config, access, terrain, chunks, journal, day): Environment,
    cameras: Query<&GlobalTransform, With<terra_player::TerraCamera>>,
    mut batch: Query<(Entity, &MeshMaterial3d<Foam>, &mut Visibility), With<Breakers>>,
    mut state: ResMut<Surf>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Foam>>,
    mut commands: Commands,
) {
    let Ok((entity, material, mut visible)) = batch.single_mut() else {
        return;
    };
    let Ok(camera) = cameras.single() else { return };
    let eye = camera.translation();
    let world = terrain.0.config();
    let sea = world.sea_level;
    let strength = config.client.water_wave_strength;
    if !access.ready()
        || world.ecology_revision < 22
        || !world.water
        || strength <= 0.0
        || (eye.y - sea as f32).abs() > 76.0
    {
        *visible = Visibility::Hidden;
        state.tiles.clear();
        return;
    }
    let now = time.elapsed_secs();
    if now - state.last < 0.24 && now >= state.last {
        return;
    }
    state.last = now;
    let daylight = day.as_ref().map_or(1.0, |d| d.daylight);
    let fill = (0.12 + 0.88 * daylight) * 24.;
    let value = Vec4::new(fill.round() / 24., strength.min(1.5), 0., 0.);
    if materials
        .get(&material.0)
        .is_some_and(|m| m.light_and_strength != value)
        && let Some(mut m) = materials.get_mut(&material.0)
    {
        m.light_and_strength = value;
    }
    let center = [eye.x, eye.z].map(|v| (v.floor() as i64).div_euclid(16));
    let reach = match config.client.water_quality {
        WaterQuality::Low => 1,
        WaterQuality::Medium => 2,
        WaterQuality::High => 3,
    };
    let before = state.tiles.len();
    state
        .tiles
        .retain(|p, _| (p[0] - center[0]).abs().max((p[1] - center[1]).abs()) <= reach);
    let mut dirty = before != state.tiles.len();
    let mut budget = 5;
    // Fill the nearest rings first, spreading shoreline discovery over several frames.
    for ring in 0..=reach {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                let tile = [center[0] + dx, center[1] + dz];
                let stamp = signature(&chunks, &journal, tile, sea.floor() as i64);
                if state.tiles.get(&tile).is_some_and(|(old, _)| *old == stamp) {
                    continue;
                }
                let edges = survey(&chunks, &journal, tile, sea);
                let changed = state
                    .tiles
                    .get(&tile)
                    .map_or(!edges.is_empty(), |(_, old)| old != &edges);
                state.tiles.insert(tile, (stamp, edges));
                dirty |= changed;
                budget -= 1;
                if budget == 0 {
                    break;
                }
            }
            if budget == 0 {
                break;
            }
        }
        if budget == 0 {
            break;
        }
    }
    if !dirty {
        if state.tiles.values().all(|(_, e)| e.is_empty()) {
            *visible = Visibility::Hidden;
        }
        return;
    }
    let mut edges: Vec<_> = state
        .tiles
        .values()
        .flat_map(|(_, e)| e.iter().copied())
        .collect();
    if edges.is_empty() {
        *visible = Visibility::Hidden;
        return;
    }
    edges.sort_by_key(|e| {
        let p = [e.rock[0] - eye.x as i64, e.rock[1] - eye.z as i64];
        p[0] * p[0] + p[1] * p[1]
    });
    let cap = match config.client.water_quality {
        WaterQuality::Low => 112,
        WaterQuality::Medium => 400,
        WaterQuality::High => 768,
    };
    let mesh = ribbons(&edges[..edges.len().min(cap)], sea as f32);
    if let Some(handle) = &state.mesh
        && let Some(mut current) = meshes.get_mut(handle)
    {
        *current = mesh;
    } else {
        let handle = meshes.add(mesh);
        commands.entity(entity).insert(Mesh3d(handle.clone()));
        state.mesh = Some(handle);
    }
    *visible = Visibility::Visible;
}

fn ribbons(edges: &[Edge], sea: f32) -> Mesh {
    let mut pos = Vec::new();
    let mut normal = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for edge in edges {
        let [x, z] = edge.rock.map(|n| n as f32 + 0.5);
        let [dx, dz] = edge.normal.map(|n| n as f32);
        let base = pos.len() as u32;
        // A vertical curl touching the stone joins a subdivided 3.8 m run-up.
        // The shader animates its height, runback and foam without a mesh upload.
        for (row, (distance, height, v)) in [
            (0.035, 1.1, -1.),
            (0.065, 0.34, -0.32),
            (0.12, 0.10, 0.),
            (0.72, 0.09, 0.17),
            (1.5, 0.09, 0.38),
            (2.5, 0.09, 0.65),
            (3.8, 0.07, 1.),
        ]
        .into_iter()
        .enumerate()
        {
            for u in [0.0, 1.0] {
                let end = u - 0.5;
                pos.push([
                    x + dx * (0.5 + distance) - dz * end,
                    sea + height,
                    z + dz * (0.5 + distance) + dx * end,
                ]);
                normal.push([dx, 0., dz]);
                uvs.push([u, v]);
            }
            if row > 0 {
                let i = base + row as u32 * 2;
                indices.extend([i - 2, i, i - 1, i - 1, i, i + 1]);
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakers_belong_to_submerged_coastal_rock_faces_including_chunk_seams() {
        // West is an inland pool, south a jetty, north hasn't streamed yet.
        // East is open sea on the other side of the section boundary.
        let block = |x, _, z| match (x, z) {
            (15, 4) => Some(M::TidewornRock as u16),
            (15, 5) => Some(M::SprucePlanks as u16),
            (15, 3) => None,
            _ => Some(M::Air as u16),
        };
        let edges = wet_rock_edges([0, 0], 8, block, |x, _| x >= 15);
        assert_eq!(
            edges,
            [Edge {
                rock: [15, 4],
                normal: [1, 0]
            }]
        );
        let mesh = ribbons(&edges, 8.0);
        assert_eq!(mesh.count_vertices(), 14);
        assert_eq!(mesh.indices().unwrap().len(), 36);
        assert!(wet_rock_edges([1, 0], 8, block, |_, _| true).is_empty());
    }
}
