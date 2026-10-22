//! Client presentation only; removal, resources and placement remain authoritative.
mod drops;
mod effects;
mod fracture;
mod held;
mod isolation;
mod mining_geometry;
mod model_pieces;
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
    ghost: [Handle<StandardMaterial>; 2],
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
    debug: Res<'w, DebugSettings>,
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
                | ItemKind::DyedBed { material: id, .. } => {
                    templates.blocks.get(&id).map(|b| b.face_materials[3])
                }
                ItemKind::Tool(_) | ItemKind::Stick => {
                    templates.blocks.get(&6).map(|b| b.face_materials[3])
                }
                ItemKind::Mineral(kind) => {
                    templates.ore_materials.get(&kind.ore().material()).copied()
                }
                ItemKind::Animal(_) => Some(templates.prop_materials.flowers[2]),
                ItemKind::Bellows(_) | ItemKind::Dye(_) => Some(templates.item_materials.opaque),
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
struct BuildGhost;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<BuildingStatus>()
        .init_resource::<effects::Effects>()
        .init_resource::<fracture::FractureState>()
        .init_resource::<drops::DropModels>()
        .init_resource::<isolation::Isolation>()
        .init_resource::<DebugSettings>()
        .add_message::<terra_network::client::ConfirmedEdit>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                surfaces,
                isolation::update,
                isolation::animate,
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
            .after(drops::animate),
    );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        ghost: [
            Color::srgba(0.85, 0.16, 0.12, 0.17),
            Color::srgba(0.32, 0.80, 0.30, 0.17),
        ]
        .map(|base_color| {
            materials.add(StandardMaterial {
                base_color,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })
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
    mut ghost: Query<
        (
            Entity,
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<BuildGhost>,
    >,
    mut gizmos: Gizmos,
    mut status: ResMut<BuildingStatus>,
) {
    let interactive = world.active() && world.input.captured && !world.blocked.0;
    let preview_active =
        interactive && world.debug.enabled && world.debug.block_placement && !world.motion.swinging;
    status.preview_visible = preview_active && preview.position.is_some();
    status.preview_valid = status.preview_visible && preview.valid;
    if let Some(position) = preview.position.filter(|_| preview_active) {
        let transform =
            Transform::from_translation(Vec3::from_array(position.map(|v| v as f32 + 0.5)))
                .with_scale(Vec3::splat(1.004));
        let material = world.assets.ghost[usize::from(preview.valid)].clone();
        if let Ok((_, mut t, mut m)) = ghost.single_mut() {
            *t = transform;
            m.0 = material;
        } else if ghost.is_empty() {
            commands.spawn((
                BuildGhost,
                Mesh3d(world.assets.cube.clone()),
                MeshMaterial3d(material),
                transform,
                NotShadowCaster,
            ));
        }
        gizmos.cube(
            transform,
            if preview.valid {
                Color::srgb(0.50, 0.85, 0.30)
            } else {
                Color::srgb(0.95, 0.25, 0.17)
            },
        );
    } else {
        for (entity, _, _) in &ghost {
            commands.entity(entity).despawn();
        }
    }
}
