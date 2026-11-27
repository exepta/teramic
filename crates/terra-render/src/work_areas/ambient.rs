//! One terrain-following atmosphere pass, independent of light sources.
mod field;
mod material;
use self::{
    field::{Field, SIZE},
    material::BiomeMistMaterial,
};
use super::{Cache, MistMaterial, geometry::Geometry, ground};
use bevy::{camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*};
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const REFRESH_SECONDS: f32 = 0.5;
const OPTICAL_DENSITY: f32 = 0.09;
#[derive(Resource, Default, Debug)]
pub struct SpiritMistStatus {
    /// Occupied field cells, retained for native biome diagnostics.
    pub banks: usize,
    pub blood_cells: usize,
    pub mushroom_cells: usize,
    pub sampled_columns: usize,
    pub generation: u64,
    pub drift: [f32; 2],
    /// Actual submitted volumes, independent of the number of occupied cells.
    pub volumes: usize,
    pub ray_steps: usize,
    pub terrain_texture_bytes: usize,
}
#[derive(Component)]
struct AmbientMist;
#[derive(Resource, Default)]
struct Banks {
    field: Field,
    occupied: usize,
    next_selection: f32,
    mesh: Handle<Mesh>,
    material: Handle<BiomeMistMaterial>,
    terrain: Handle<Image>,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<SpiritMistStatus>();
    // Diagnostic A/B only; no save/config state is changed.
    if std::env::var("TERRA_SPIRIT_MIST").as_deref() == Ok("off") {
        return;
    }
    crate::material_work::add::<BiomeMistMaterial>(app)
        .init_resource::<Banks>()
        .add_systems(Startup, setup.after(super::setup))
        .add_systems(
            PostUpdate,
            update.before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
}
fn setup(
    mut commands: Commands,
    contours: Res<Cache>,
    source_materials: Res<Assets<MistMaterial>>,
    mut banks: ResMut<Banks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BiomeMistMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(source) = source_materials.get(&contours.materials[0]) else {
        return;
    };
    banks.terrain = images.add(material::terrain_image(vec![0; SIZE * SIZE * 4]));
    banks.material = materials.add(BiomeMistMaterial {
        color: Color::srgba(0.67, 0.86, 0.94, 0.60).to_linear(),
        flow: super::material::flow(0.).with_w(OPTICAL_DENSITY),
        center: Vec4::ZERO,
        half_extent: Vec4::ONE,
        field: Vec4::ZERO,
        noise: source.noise.clone(),
        terrain: banks.terrain.clone(),
        outdoors: Vec4::ONE,
    });
    banks.mesh = meshes.add(Geometry::placeholder_mesh());
    commands.spawn((
        Name::new("Terrain-following biome mist"),
        AmbientMist,
        Mesh3d(banks.mesh.clone()),
        MeshMaterial3d(banks.material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
    ));
}
fn surface_sample(
    chunks: &ChunkStore,
    journal: &EditJournal,
    x: i64,
    z: i64,
) -> Option<(f32, f32, f32)> {
    let column = chunks.surface_column_at(x, z)?;
    let weights = column.biome.weights;
    let spirit = weights.spirit_blossom as f32;
    let blood = weights.blood_forest as f32;
    // Wetland ribbons share the neutral low mist used on damp mushroom banks.
    let wetland = weights.swamp as f32;
    let mushroom = weights.mushroom_park as f32;
    if spirit.max(blood).max(mushroom).max(wetland) <= 0.05 {
        return None;
    }
    let open_level = |c: &terra_world::SurfaceColumn| {
        c.water_level
            .filter(|&h| h > c.height + 0.1 && c.river_cavity.is_none())
    };
    // Sample a narrow stream between the 16 m field nodes. Moist banks only
    // contribute near the water level, so mist cannot climb up a dry cliff.
    let shore = if open_level(column).is_some() {
        1.0
    } else if mushroom.max(wetland) > 0.05
        && [(5, 0), (-5, 0), (0, 5), (0, -5)]
            .into_iter()
            .any(|(dx, dz)| {
                chunks
                    .surface_column_at(x + dx, z + dz)
                    .and_then(open_level)
                    .is_some_and(|level| (column.height - level).abs() < 2.5)
            })
    {
        0.45
    } else {
        0.0
    };
    let regional = blood - mushroom * shore - wetland * (0.57 + 0.43 * shore);
    if spirit <= 0.05 && regional.abs() < 0.045 {
        return None;
    }
    let hint = column.height.ceil() as i64;
    let height = {
        let floor = ground::height_matching(chunks, journal, x, z, hint, |id| {
            terra_voxel::MaterialId::from_raw(id).is_some_and(|m| m.is_natural_ground())
        })?;
        if floor < column.height as f32 - 3.0 {
            return None;
        }
        floor
    };
    let floor = height.max(column.water_level.map_or(height, |h| h as f32));
    Some((floor, spirit, regional))
}
type OutdoorEnvironment<'w> = (
    Option<Res<'w, crate::CaveState>>,
    Option<Res<'w, crate::UnderwaterState>>,
    Option<Res<'w, crate::scene::daylight::Daylight>>,
);
#[allow(clippy::too_many_arguments)] // Independent cached terrain and render assets.
fn update(
    time: Res<Time<Real>>,
    access: Option<Res<WorldAccess>>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    camera: Query<&GlobalTransform, With<TerraCamera>>,
    (cave, water, daylight): OutdoorEnvironment,
    mut banks: ResMut<Banks>,
    mut status: ResMut<SpiritMistStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BiomeMistMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut visibility: Query<&mut Visibility, With<AmbientMist>>,
) {
    let ready = access.is_some_and(|access| access.ready());
    let eye = camera.iter().next().map(GlobalTransform::translation);
    if time.elapsed_secs() >= banks.next_selection || !ready {
        banks.next_selection = time.elapsed_secs() + REFRESH_SECONDS;
        status.sampled_columns = 0;
        let next = if ready && let (Some(eye), Some(chunks), Some(journal)) = (eye, chunks, journal)
        {
            status.sampled_columns = SIZE * SIZE;
            Field::sample(eye, |x, z| surface_sample(&chunks, &journal, x, z))
        } else {
            Field::default()
        };
        if next != banks.field {
            banks.field = next;
            banks.occupied = banks.field.occupied();
            status.banks = banks.field.weights.iter().filter(|&&w| w > 0.002).count();
            status.blood_cells = banks.field.regional.iter().filter(|&&w| w > 0.004).count();
            status.mushroom_cells = banks.field.regional.iter().filter(|&&w| w < -0.004).count();
            if banks.occupied > 0 {
                let (center, extent) = banks.field.bounds();
                let (pixels, base, span) = banks.field.pixels();
                let mut geometry = Geometry::default();
                geometry.bank(center, extent, [0.; 2], field::HEIGHT);
                if let Some(mut mesh) = meshes.get_mut(&banks.mesh) {
                    *mesh = geometry.mesh();
                }
                if let Some(mut image) = images.get_mut(&banks.terrain) {
                    image.data = Some(pixels);
                }
                if let Some(mut material) = materials.get_mut(&banks.material) {
                    material.center = center.extend(field::RANGE);
                    material.half_extent = extent.extend(field::HEIGHT);
                    material.field = Vec4::new(
                        banks.field.origin[0] as f32,
                        banks.field.origin[1] as f32,
                        base,
                        span,
                    );
                }
            }
            status.generation = status.generation.wrapping_add(1);
        }
    }
    let outside = cave.as_deref().map_or(1., crate::CaveState::outdoor_mist)
        * (1. - water.map_or(0., |w| w.blend));
    let visible =
        ready && eye.is_some() && banks.occupied > 0 && (status.banks > 0 || outside > 0.005);
    for mut visibility in &mut visibility {
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    status.volumes = usize::from(visible);
    status.ray_steps = if visible { 6 } else { 0 };
    status.terrain_texture_bytes = SIZE * SIZE * 4;
    if visible && let Some(mut material) = materials.get_mut(&banks.material) {
        material.outdoors = Vec4::new(
            outside,
            daylight.map_or(1., |d| 0.15 + 0.85 * d.daylight),
            0.,
            0.,
        );
        material.flow = super::material::flow(time.elapsed_secs_f64()).with_w(OPTICAL_DENSITY);
        status.drift = [material.flow.x, material.flow.y];
    }
}
#[cfg(test)]
mod tests;
