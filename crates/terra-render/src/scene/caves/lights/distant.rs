//! One depth-tested batch preserves small lamp/flame sources in the distance.
//! It never adds PBR lights; illumination and shadow budgets stay in the local pool.
use super::*;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
};

const SEGMENTS: usize = 8;

#[derive(Component)]
struct DistantBatch;

#[derive(Resource, Default)]
struct Presentation {
    entity: Option<Entity>,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    next: f32,
    selection: u64,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Presentation>()
        .add_systems(Update, update.after(super::select));
}

fn budget(quality: CaveQuality) -> usize {
    match quality {
        CaveQuality::Low => 128,
        CaveQuality::Medium => 256,
        CaveQuality::High => 384,
        CaveQuality::Ultra => 512,
    }
}

fn opacity(distance: f32, range: f32, reach: f32) -> f32 {
    // Blend onto the authored flame instead of replacing it with a popping LOD.
    let near = (distance - range * 0.35) / (range * 0.65).max(1.0);
    let far = (reach - distance) / (reach * 0.12).max(8.0);
    let smooth = |value: f32| {
        let value = value.clamp(0.0, 1.0);
        value * value * (3.0 - 2.0 * value)
    };
    smooth(near) * smooth(far) * 0.72
}

fn radius(distance: f32) -> f32 {
    // Keep a small approximately pixel-sized source; never an oversized sky halo.
    (0.055 + distance * 0.0012).clamp(0.06, 0.42)
}

fn candidates(
    cache: &Lamps,
    chunks: &ChunkStore,
    journal: &EditJournal,
    camera: &Transform,
    definitions: &BTreeMap<u16, BlockLight>,
    reach: f32,
    budget: usize,
) -> Vec<(Vec3, BlockLight, f32)> {
    let eye = camera.translation;
    let forward = camera.forward().as_vec3();
    let mut candidates: Vec<_> = cache
        .emitters
        .iter()
        .filter_map(|&(cell, light)| {
            let section = global_to_section(cell).0;
            let material = chunks
                .block_at(cell)
                .map(|id| journal.block(cell).unwrap_or(id))?;
            if !definitions.contains_key(&material) {
                // A prop is indexed by its support cell. Validate the live
                // prop itself when the voxel underneath is ordinary terrain.
                let loaded = chunks.loaded.get(&section)?;
                let prop = loaded
                    .props
                    .iter()
                    .chain(journal.placed_props(section))
                    .any(|prop| {
                        prop.kind == terra_world::props::PropKind::Scarecrow
                            && !prop.removed
                            && !journal.prop_removed(section, prop.id)
                            && terra_world::props::placement::support(prop) == cell
                    });
                if !prop {
                    return None;
                }
            }
            let point = emitter_location(
                journal,
                cell,
                light,
                location(cell, material == 40, |p| {
                    chunks.block_at(p).map(|id| journal.block(p).unwrap_or(id))
                }),
            );
            let delta = point - eye;
            let distance = delta.length();
            let opacity = opacity(distance, light.range, reach);
            (distance.is_finite()
                && distance < reach
                && delta.dot(forward) > 0.0
                && opacity > 0.001)
                .then_some((point, light, opacity))
        })
        .collect();
    let compare = |a: &(Vec3, BlockLight, f32), b: &(Vec3, BlockLight, f32)| {
        a.0.distance_squared(eye)
            .total_cmp(&b.0.distance_squared(eye))
            .then(a.0.x.total_cmp(&b.0.x))
            .then(a.0.y.total_cmp(&b.0.y))
            .then(a.0.z.total_cmp(&b.0.z))
    };
    if candidates.len() > budget {
        candidates.select_nth_unstable_by(budget, compare);
        candidates.truncate(budget);
    }
    candidates.sort_unstable_by(compare);
    candidates
}

fn mesh(sources: &[(Vec3, BlockLight, f32)], camera: &Transform) -> Mesh {
    let mut positions = Vec::with_capacity(sources.len() * (SEGMENTS + 1));
    let mut colors = Vec::with_capacity(positions.capacity());
    let mut indices = Vec::with_capacity(sources.len() * SEGMENTS * 3);
    let right = camera.right().as_vec3();
    let up = camera.up().as_vec3();
    // Selection keeps the nearest sources inside the budget, but transparent
    // geometry must be submitted back-to-front within this single draw.
    for &(center, light, alpha) in sources.iter().rev() {
        let base = positions.len() as u32;
        positions.push(center.to_array());
        colors.push([
            light.color[0] * 1.6,
            light.color[1] * 1.6,
            light.color[2] * 1.6,
            alpha,
        ]);
        let size = radius(center.distance(camera.translation));
        for i in 0..SEGMENTS {
            let angle = i as f32 * std::f32::consts::TAU / SEGMENTS as f32;
            positions
                .push((center + right * angle.cos() * size + up * angle.sin() * size).to_array());
            colors.push([
                light.color[0] * 1.6,
                light.color[1] * 1.6,
                light.color[2] * 1.6,
                0.0,
            ]);
            indices.extend([
                base,
                base + i as u32 + 1,
                base + ((i + 1) % SEGMENTS) as u32 + 1,
            ]);
        }
    }
    let count = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![(-camera.forward()).to_array(); count],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

fn material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        fog_enabled: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    }
}

#[allow(clippy::too_many_arguments)] // Independent Bevy resources and the persistent single batch.
fn update(
    mut commands: Commands,
    time: Res<Time>,
    access: Res<WorldAccess>,
    graphics: Option<Res<GraphicsConfig>>,
    cache: Res<Lamps>,
    assets: Res<BlockAssets>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    cameras: Query<&Transform, With<terra_player::TerraCamera>>,
    mut presentation: ResMut<Presentation>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if time.elapsed_secs() < presentation.next
        && presentation.selection == cache.diagnostics.selections
    {
        return;
    }
    presentation.selection = cache.diagnostics.selections;
    presentation.next = time.elapsed_secs() + 0.1;
    let Some(camera) = cameras.iter().next().filter(|_| access.ready()) else {
        if let Some(entity) = presentation.entity {
            commands.entity(entity).insert(Visibility::Hidden);
        }
        return;
    };
    let reach = graphics
        .as_ref()
        .map_or(192.0, |config| f32::from(config.render_distance) * 16.0);
    let quality = graphics
        .as_ref()
        .map_or(CaveQuality::Medium, |config| config.cave_quality);
    let definitions = assets.lights().collect();
    let sources = candidates(
        &cache,
        &chunks,
        &journal,
        camera,
        &definitions,
        reach,
        budget(quality),
    );
    if sources.is_empty() {
        if let Some(entity) = presentation.entity {
            commands.entity(entity).insert(Visibility::Hidden);
        }
        return;
    }
    let geometry = mesh(&sources, camera);
    if let Some(mut existing) = meshes.get_mut(&presentation.mesh) {
        *existing = geometry;
    } else {
        presentation.mesh = meshes.add(geometry);
    }
    if let Some(entity) = presentation.entity {
        commands.entity(entity).insert(Visibility::Visible);
    } else {
        presentation.material = materials.add(material());
        presentation.entity = Some(
            commands
                .spawn((
                    Name::new("Distant village lamp sources"),
                    DistantBatch,
                    Mesh3d(presentation.mesh.clone()),
                    MeshMaterial3d(presentation.material.clone()),
                    Transform::default(),
                    Visibility::Visible,
                    NoFrustumCulling,
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .id(),
        );
    }
}

#[cfg(test)]
#[path = "distant_tests.rs"]
mod tests;
