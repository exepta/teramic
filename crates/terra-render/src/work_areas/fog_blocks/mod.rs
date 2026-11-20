//! Placed and naturally generated fog stones share one bounded atmosphere pass.
mod field;
mod material;
#[cfg(test)]
mod tests;
use super::{Cache, MistMaterial, geometry::Geometry};
use bevy::{camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*};
use field::{Field, HEIGHT, Index, MAX_SOURCES, SIZE};
use material::FogMaterial;
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, SectionEditApplied, SectionPublished, SectionRemoved, WorldAccess},
};

#[derive(Resource, Default, Debug)]
pub struct FogBlockStatus {
    pub sources: usize,
    pub volumes: usize,
    pub radius: f32,
    pub sampled_cells: usize,
    pub generation: u64,
    pub drift: [f32; 2],
}
#[derive(Component)]
struct FogVolume;
#[derive(Resource, Default)]
struct State {
    index: Index,
    next_refresh: f32,
    field: Option<Field>,
    mesh: Handle<Mesh>,
    material: Handle<FogMaterial>,
    terrain: Handle<Image>,
}
pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FogMaterial>::default())
        .init_resource::<State>()
        .init_resource::<FogBlockStatus>()
        .add_systems(Startup, setup.after(super::setup))
        .add_systems(PostUpdate, invalidate.before(update))
        .add_systems(
            PostUpdate,
            update.before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
}
fn invalidate(
    mut state: ResMut<State>,
    mut published: MessageReader<SectionPublished>,
    mut removed: MessageReader<SectionRemoved>,
    mut edited: MessageReader<SectionEditApplied>,
) {
    for position in published
        .read()
        .map(|e| e.position)
        .chain(removed.read().map(|e| e.position))
        .chain(edited.read().map(|e| e.position))
    {
        state.index.invalidate(position);
        state.next_refresh = 0.0;
    }
}
fn setup(
    mut commands: Commands,
    contours: Res<Cache>,
    source: Res<Assets<MistMaterial>>,
    mut state: ResMut<State>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<FogMaterial>>,
) {
    let Some(base) = source.get(&contours.materials[0]) else {
        return;
    };
    state.mesh = meshes.add(Geometry::placeholder_mesh());
    state.terrain = images.add(material::terrain_image(vec![0; SIZE * SIZE * 4]));
    state.material = materials.add(FogMaterial {
        color: Color::srgba(0.62, 0.79, 0.80, 0.65).to_linear(),
        flow: super::material::flow(0.0).with_w(0.20),
        center: Vec4::ZERO,
        half_extent: Vec4::ONE,
        field: Vec4::ZERO,
        noise: base.noise.clone(),
        terrain: state.terrain.clone(),
        sources: [Vec4::ZERO; MAX_SOURCES],
    });
    commands.spawn((
        Name::new("Fog blocks: one circular ground mist volume"),
        FogVolume,
        Mesh3d(state.mesh.clone()),
        MeshMaterial3d(state.material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
    ));
}
#[allow(clippy::too_many_arguments)] // Shared cached GPU assets and independently streamed world.
fn update(
    time: Res<Time<Real>>,
    access: Option<Res<WorldAccess>>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    camera: Query<&GlobalTransform, With<TerraCamera>>,
    mut state: ResMut<State>,
    mut status: ResMut<FogBlockStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut visibility: Query<&mut Visibility, With<FogVolume>>,
) {
    let eye = camera
        .iter()
        .next()
        .map(GlobalTransform::translation)
        .filter(|eye| eye.is_finite());
    let ready = access.is_some_and(|a| a.ready());
    if time.elapsed_secs() >= state.next_refresh || !ready {
        state.next_refresh = time.elapsed_secs() + 0.5;
        let next = if ready && let (Some(eye), Some(chunks), Some(journal)) = (eye, chunks, journal)
        {
            state.index.refresh(&chunks, &journal);
            let nearby = state.index.nearby(eye);
            Some(Field::sample(eye, &nearby, &chunks, &journal))
        } else {
            state.index = Index::default();
            None
        };
        if next != state.field {
            state.field = next;
            if let Some(field) = state.field.as_ref().filter(|f| f.occupied() > 0) {
                let (center, extent) = field.bounds();
                let (pixels, base, span) = field.pixels();
                let mut geometry = Geometry::default();
                geometry.bank(center, extent, [0.0; 2], HEIGHT);
                if let Some(mut mesh) = meshes.get_mut(&state.mesh) {
                    *mesh = geometry.mesh();
                }
                if let Some(mut image) = images.get_mut(&state.terrain) {
                    image.data = Some(pixels);
                }
                if let Some(mut material) = materials.get_mut(&state.material) {
                    material.center = center.extend(field.sources.len() as f32);
                    material.half_extent = extent.extend(HEIGHT);
                    material.field =
                        Vec4::new(field.origin[0] as f32, field.origin[1] as f32, base, span);
                    material.sources = [Vec4::ZERO; MAX_SOURCES];
                    for (target, source) in material.sources.iter_mut().zip(&field.sources) {
                        *target = *source;
                    }
                }
            }
            status.generation = status.generation.wrapping_add(1);
        }
    }
    let visible = ready && eye.is_some() && state.field.as_ref().is_some_and(|f| f.occupied() > 0);
    for mut visibility in &mut visibility {
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    status.sources = if visible {
        state.field.as_ref().map_or(0, |f| f.sources.len())
    } else {
        0
    };
    status.volumes = usize::from(visible);
    status.radius = terra_core::fog_blocks::RADIUS;
    status.sampled_cells = if visible { SIZE * SIZE } else { 0 };
    if let Some(mut material) = materials.get_mut(&state.material) {
        material.flow = super::material::flow(time.elapsed_secs_f64()).with_w(0.20);
        status.drift = [material.flow.x, material.flow.y];
    }
}
