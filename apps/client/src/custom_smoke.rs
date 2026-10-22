use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};
use std::time::{Duration, Instant};
use terra_player::TerraCamera;
use terra_render::{
    RenderStatus, RenderedSections,
    blocks::definitions::{BlockLibrary, BlockRenderKind, ModelLayout},
};
use terra_voxel::global_to_section;
use terra_world::{
    edits::EditJournal,
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, GenerationStatus, StreamingSet, WorldGeneration,
        WorldTerrain,
    },
};

const FIXTURE_MATERIAL: u16 = 65000;

#[derive(Resource)]
pub(crate) struct CustomSmoke {
    pub(crate) complete: bool,
    error: Option<String>,
    reported_error: bool,
    placements: Option<[[i64; 3]; 4]>,
    started: Instant,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-custom") {
        return;
    }
    let error = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
        .and_then(|library| {
            library
                .blocks
                .iter()
                .any(|block| {
                    block.id == FIXTURE_MATERIAL
                        && block.render_kind == BlockRenderKind::CustomMesh
                        && block.model_layout == ModelLayout::Scene
                })
                .then_some(())
                .ok_or_else(|| {
                    "Custom smoke requires an opt-in scene fixture with block ID 65000 in TERRA_ASSET_DIR"
                        .to_owned()
                })
        })
        .err();
    app.insert_resource(CustomSmoke {
        complete: false,
        error,
        reported_error: false,
        placements: None,
        started: Instant::now(),
    })
    .add_systems(Update, inspect_fixture.after(StreamingSet::Dispatch));
}

#[derive(SystemParam)]
struct CustomWorld<'w> {
    chunks: Res<'w, ChunkStore>,
    generation: Res<'w, WorldGeneration>,
    terrain: Res<'w, WorldTerrain>,
    renderer: Res<'w, RenderStatus>,
    rendered: Res<'w, RenderedSections>,
    journal: Res<'w, EditJournal>,
}

fn inspect_fixture(
    world: CustomWorld,
    mut smoke: ResMut<CustomSmoke>,
    mut edits: MessageWriter<BlockEditRequest>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut focus: ResMut<ChunkFocus>,
    mut exit: MessageWriter<AppExit>,
) {
    if smoke.complete || smoke.reported_error {
        return;
    }
    if smoke.error.is_none() {
        smoke.error = world.renderer.error.clone().or_else(|| {
            (smoke.started.elapsed() > Duration::from_secs(120))
                .then(|| "Custom smoke fixture placement/rendering timed out".to_owned())
        });
    }
    if let Some(error) = &smoke.error {
        tracing::error!(%error, "Custom mesh smoke failed");
        smoke.reported_error = true;
        exit.write(AppExit::error());
        return;
    }
    if world.generation.status != GenerationStatus::Ready {
        return;
    }
    let Some(placements) = smoke.placements else {
        let Some(mut camera) = camera.iter_mut().next() else {
            return;
        };
        let Some(placements) = find_ground_row(&world, camera.translation) else {
            smoke.error = Some("No unobstructed four-cell fixture site near the camera".to_owned());
            return;
        };
        for (index, &position) in placements.iter().enumerate() {
            edits.write(BlockEditRequest {
                position,
                expected_material: 0,
                material: if index < 3 { FIXTURE_MATERIAL } else { 3 },
            });
        }
        let [x, y, z] = placements[0].map(|value| value as f32);
        let target = Vec3::new(x + 1.75, y + 0.3, z + 0.5);
        let eye = target + Vec3::new(-2.6, 2.1, 5.0);
        *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        focus.0 = eye.to_array().map(f64::from);
        smoke.placements = Some(placements);
        tracing::info!(
            ?placements,
            "Custom smoke placed three static models beside one cube"
        );
        return;
    };
    if !world.renderer.ready {
        return;
    }
    for (index, &position) in placements.iter().enumerate() {
        let material = if index < 3 { FIXTURE_MATERIAL } else { 3 };
        if world.chunks.block_at(position) != Some(material) {
            return;
        }
        let (owner, _) = global_to_section(position);
        if world.rendered.sections.get(&owner).is_none_or(|section| {
            section.mesh_epoch != world.journal.mesh_epoch(owner) || section.meshes.is_empty()
        }) {
            return;
        }
    }
    smoke.complete = true;
    tracing::info!(
        ?placements,
        "Custom mesh smoke fixture uploaded with current edit epochs"
    );
}

fn find_ground_row(world: &CustomWorld, eye: Vec3) -> Option<[[i64; 3]; 4]> {
    let [center_x, _, center_z] = eye.to_array().map(|value| value.floor() as i64);
    for radius in 0_i64..=32 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dz.abs()) != radius {
                    continue;
                }
                let x = center_x + dx;
                let z = center_z + dz;
                let surface = world
                    .terrain
                    .0
                    .surface_height(x as f64 + 0.5, z as f64 + 0.5);
                let y = (surface - 0.5).floor() as i64 + 1;
                if (y as f64) <= world.terrain.0.config().sea_level + 1.0 {
                    continue;
                }
                let placements = std::array::from_fn(|index| [x + index as i64, y, z]);
                if !placements.iter().all(|&[x, y, z]| {
                    matches!(world.chunks.block_at([x, y - 1, z]), Some(1..=5))
                        && (0..=3).all(|dy| world.chunks.block_at([x, y + dy, z]) == Some(0))
                }) {
                    continue;
                }
                // Keep both the close camera and its view of the models out of tree trunks.
                if (-2..=4).all(|x_offset| {
                    (1..=6).all(|z_offset| {
                        (1..=3).all(|height| {
                            world
                                .chunks
                                .block_at([x + x_offset, y + height, z + z_offset])
                                == Some(0)
                        })
                    })
                }) {
                    return Some(placements);
                }
            }
        }
    }
    None
}
