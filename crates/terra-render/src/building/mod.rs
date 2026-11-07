//! Client presentation only; removal, resources and placement remain authoritative.
mod drops;
mod effects;
mod felling;
mod fracture;
mod ghost;
mod held;
mod isolation;
mod mining_geometry;
pub(crate) mod model_pieces;
mod outline;
#[cfg(test)]
mod tests;

use crate::blocks::BlockAssets;
use bevy::{ecs::system::SystemParam, light::NotShadowCaster, prelude::*};
use terra_core::{DebugSettings, ItemKind, ResourcePropKind};
use terra_network::client::ClientSession;
use terra_player::{
    CameraInputState, PlayerInputBlock,
    interaction::{InteractionMotion, PlacementPreview},
};
use terra_world::streaming::ChunkStore;

pub use felling::TreeFallStatus;
pub use outline::PlacementOutlineMaterial;

#[derive(Resource, Default, Debug)]
pub struct BuildingStatus {
    pub isolated_block: bool,
    pub crack_segments: usize,
    pub preview_visible: bool,
    pub preview_valid: bool,
    pub particles: usize,
    pub drops: usize,
}

#[derive(Resource)]
struct BuildingAssets {
    cube: Handle<Mesh>,
    turf_cube: Handle<Mesh>,
    outline: Handle<PlacementOutlineMaterial>,
    fallback: Handle<StandardMaterial>,
    fracture: Handle<StandardMaterial>,
}

#[derive(SystemParam)]
struct BuildingWorld<'w> {
    assets: Res<'w, BuildingAssets>,
    blocks: Res<'w, BlockAssets>,
    session: Option<Res<'w, ClientSession>>,
    input: Res<'w, CameraInputState>,
    blocked: Res<'w, PlayerInputBlock>,
    motion: Res<'w, InteractionMotion>,
    mining: Res<'w, terra_player::interaction::MiningVisual>,
    chunks: Res<'w, ChunkStore>,
    time: Res<'w, Time>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
}
impl BuildingWorld<'_> {
    fn mesh(&self, item: ItemKind) -> Handle<Mesh> {
        if item == ItemKind::Block(1) {
            self.assets.turf_cube.clone()
        } else {
            self.assets.cube.clone()
        }
    }
    fn active(&self) -> bool {
        self.session
            .as_ref()
            .is_none_or(|s| s.ready && s.error.is_none())
    }
    fn material(&self, item: ItemKind) -> Handle<StandardMaterial> {
        let slot = self
            .blocks
            .templates
            .as_ref()
            .and_then(|templates| match item {
                ItemKind::Block(id)
                | ItemKind::WoodenBlock { material: id, .. }
                | ItemKind::TintedBlock { material: id, .. }
                | ItemKind::DyedBed { material: id, .. } => {
                    templates.blocks.get(&id).map(|b| b.face_materials[3])
                }
                ItemKind::Tool(_) | ItemKind::FilledWateringCan { .. } | ItemKind::Stick => {
                    templates.blocks.get(&6).map(|b| b.face_materials[3])
                }
                ItemKind::Mineral(kind) => {
                    templates.ore_materials.get(&kind.ore().material()).copied()
                }
                ItemKind::Animal(_) => Some(templates.prop_materials.flowers[2]),
                ItemKind::Flower(color) => Some(
                    templates.prop_materials.flowers
                        [usize::from(terra_core::crusher::flower_variant(color))],
                ),
                ItemKind::Bellows(_)
                | ItemKind::GlassBottle
                | ItemKind::WaterBottle
                | ItemKind::Potion(_)
                | ItemKind::Dye(_)
                | ItemKind::Armor(_)
                | ItemKind::Accessory(_)
                | ItemKind::Cape(_)
                | ItemKind::HeraldicCape { .. }
                | ItemKind::Crop(_)
                | ItemKind::Apple
                | ItemKind::AppleCore
                | ItemKind::Dust(_) => Some(templates.item_materials.opaque),
                ItemKind::Prop(kind) => Some(match kind {
                    ResourcePropKind::Flower => templates.prop_materials.flowers[0],
                    ResourcePropKind::Mushroom => templates.prop_materials.mushroom,
                    _ => templates.prop_materials.foliage[0],
                }),
            });
        slot.and_then(|slot| self.blocks.materials.get(slot))
            .cloned()
            .unwrap_or_else(|| self.assets.fallback.clone())
    }
}

#[derive(Component)]
pub struct PlacementOutline;

fn orientation_preview(material: u16) -> bool {
    terra_world::placement::furniture(material)
        || material == 40
        || terra_core::lantern::is_part(material)
}

fn outline_color(seconds: f32) -> Color {
    let pulse = 0.5 + 0.5 * (seconds * std::f32::consts::TAU / 1.6).sin();
    Color::srgba(1.0, 0.68, 0.2, 0.4 + 0.55 * pulse)
}

fn placement_outline_color(material: u16, valid: bool, seconds: f32) -> Color {
    let ordinary = outline_color(seconds);
    if terra_core::rails::is_rail(material) && !valid {
        Color::srgba(1.0, 0.12, 0.06, ordinary.to_srgba().alpha)
    } else {
        ordinary
    }
}

pub(crate) fn install(app: &mut App) {
    felling::install(app);
    held::install(app);
    app.init_resource::<Assets<PlacementOutlineMaterial>>()
        .init_resource::<BuildingStatus>()
        .init_resource::<effects::Effects>()
        .init_resource::<fracture::FractureState>()
        .init_resource::<drops::DropModels>()
        .init_resource::<isolation::Isolation>()
        .init_resource::<ghost::Silhouette>()
        .init_resource::<DebugSettings>()
        .add_message::<terra_network::client::ConfirmedEdit>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                surfaces,
                isolation::update,
                isolation::animate,
                felling::update,
                felling::animate,
                fracture::animate,
                effects::animate,
                drops::animate,
            )
                .chain()
                .after(terra_player::PlayerSet::Interaction)
                .after(crate::meshing::upload_sections),
        );
    app.add_systems(
        Update,
        held::render
            .after(crate::players::animate)
            .after(crate::creatures::CreatureAnimationSet)
            .after(drops::animate),
    );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut outlines: ResMut<Assets<PlacementOutlineMaterial>>,
) {
    // Grass uses a neutral texture; small inventory/feedback cubes need a tint
    // too. Share one colored mesh without altering the terrain material.
    let mut turf_cube = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
    let tint = Color::srgb(0.66, 0.83, 0.36).to_linear().to_f32_array();
    turf_cube.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![tint; turf_cube.count_vertices()],
    );
    commands.insert_resource(BuildingAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        turf_cube: meshes.add(turf_cube),
        outline: outlines.add(PlacementOutlineMaterial {
            color: outline_color(0.0).to_linear(),
            parameters: Vec4::new(2.0, 0.0, 0.0, 0.0),
        }),
        fallback: materials.add(StandardMaterial {
            base_color: Color::srgb(0.57, 0.58, 0.54),
            perceptual_roughness: 0.9,
            ..default()
        }),
        fracture: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
}

fn surfaces(
    mut commands: Commands,
    world: BuildingWorld,
    preview: Res<PlacementPreview>,
    mut outline: Query<
        (
            Entity,
            &mut Transform,
            &mut MeshMaterial3d<PlacementOutlineMaterial>,
            &mut Mesh3d,
        ),
        With<PlacementOutline>,
    >,
    mut status: ResMut<BuildingStatus>,
    (mut silhouette, mut meshes): (ResMut<ghost::Silhouette>, ResMut<Assets<Mesh>>),
    mut materials: ResMut<Assets<PlacementOutlineMaterial>>,
) {
    let interactive = world.active() && world.input.captured && !world.blocked.0;
    let preview_active =
        interactive && !world.motion.swinging && orientation_preview(preview.material);
    let shape = preview
        .position
        .filter(|_| preview_active)
        .and_then(|position| {
            preview
                .plan
                .as_ref()
                .zip(world.journal.as_ref())
                .and_then(|(plan, journal)| {
                    world.blocks.templates.as_ref().and_then(|templates| {
                        silhouette.prepare(
                            plan,
                            position,
                            &world.chunks,
                            journal,
                            templates,
                            &mut meshes,
                        )
                    })
                })
                .map(|mesh| (position, mesh))
        });
    status.preview_visible = shape.is_some();
    status.preview_valid = status.preview_visible && preview.valid;
    if let Some((position, mesh)) = shape {
        let transform =
            Transform::from_translation(Vec3::from_array(position.map(|v| v as f32 + 0.5)))
                .with_scale(Vec3::splat(1.004));
        let material = world.assets.outline.clone();
        if let Some(mut surface) = materials.get_mut(&material) {
            surface.color = placement_outline_color(
                preview.material,
                preview.valid,
                world.time.elapsed_secs_wrapped(),
            )
            .to_linear();
        }
        if let Ok((_, mut t, mut m, mut current_mesh)) = outline.single_mut() {
            *t = transform;
            m.0 = material;
            current_mesh.0 = mesh;
        } else if outline.is_empty() {
            commands.spawn((
                PlacementOutline,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                transform,
                NotShadowCaster,
                bevy::camera::visibility::NoFrustumCulling,
            ));
        }
    } else {
        for (entity, _, _, _) in &outline {
            commands.entity(entity).despawn();
        }
    }
}
