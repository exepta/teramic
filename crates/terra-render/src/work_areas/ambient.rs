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
    app.add_plugins(MaterialPlugin::<BiomeMistMaterial>::default())
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
        color: Color::srgba(0.69, 0.88, 0.91, 0.60).to_linear(),
        flow: super::material::flow(0.).with_w(OPTICAL_DENSITY),
        center: Vec4::ZERO,
        half_extent: Vec4::ONE,
        field: Vec4::ZERO,
        noise: source.noise.clone(),
        terrain: banks.terrain.clone(),
    });
    banks.mesh = meshes.add(Geometry::placeholder_mesh());
    commands.spawn((
        Name::new("Spirit Blossom single atmosphere volume"),
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
) -> Option<(f32, f32)> {
    let column = chunks.surface_column_at(x, z)?;
    let weight = column.biome.weights.spirit_blossom as f32;
    if weight <= 0.05 {
        return None;
    }
    let height = ground::height(chunks, journal, x, z, column.height.ceil() as i64)?;
    // The air over a swamp follows its water plane instead of its submerged bed.
    Some((
        height.max(column.water_level.map_or(height, |h| h as f32)),
        weight,
    ))
}
#[allow(clippy::too_many_arguments)] // Independent cached terrain and render assets.
fn update(
    time: Res<Time<Real>>,
    access: Option<Res<WorldAccess>>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    camera: Query<&GlobalTransform, With<TerraCamera>>,
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
            if banks.field.occupied() > 0 {
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
    let visible = ready && eye.is_some() && banks.field.occupied() > 0;
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
    status.banks = if visible { banks.field.occupied() } else { 0 };
    status.volumes = usize::from(visible);
    status.ray_steps = if visible { 6 } else { 0 };
    status.terrain_texture_bytes = SIZE * SIZE * 4;
    if visible && let Some(mut material) = materials.get_mut(&banks.material) {
        material.flow = super::material::flow(time.elapsed_secs_f64()).with_w(OPTICAL_DENSITY);
        status.drift = [material.flow.x, material.flow.y];
    }
}
#[cfg(test)]
mod tests;
