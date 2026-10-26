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
    if std::env::var_os("TERRA_SMOKE_PIPE_ROOFS").is_some() {
        app.insert_resource(CustomSmoke {
            complete: false,
            error: None,
            reported_error: false,
            placements: None,
            started: Instant::now(),
        })
        .add_systems(PostStartup, prepare_pipe_roofs)
        .add_systems(
            Update,
            inspect_pipe_roofs
                .after(StreamingSet::Dispatch)
                .after(terra_player::PlayerSet::Movement),
        );
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

#[derive(Resource)]
struct PipeRoofs {
    cells: std::collections::BTreeMap<[i64; 3], (u16, u8)>,
    pipes: [[i64; 3]; 4],
    camera: Transform,
    ready_at: Option<Instant>,
}

fn prepare_pipe_roofs(
    mut commands: Commands,
    terrain: Res<WorldTerrain>,
    mut journal: ResMut<EditJournal>,
) {
    use terra_world::edits::BlockObjectState;
    // Isolate the roof profiles above the local treetops; all geometry remains
    // ordinary editable voxels and shared production model batches.
    let base = (terrain.0.surface_height(8.5, 0.5) - 0.5).floor() as i64 + 28;
    let mut cells = std::collections::BTreeMap::new();
    let pipes = [
        [0, base + 1, 0],
        [6, base + 1, 0],
        [12, base + 1, 0],
        [18, base + 1, 0],
    ];
    for (index, &pipe) in pipes.iter().enumerate() {
        for x in -2..=2 {
            for z in -2..=2 {
                cells.insert([pipe[0] + x, base - 1, z], (3, 0));
                if x == 0 && (z >= 0 || index == 3) {
                    continue;
                }
                let roof = if index == 0 {
                    (3, 0)
                } else if index == 1 {
                    (77, 2)
                } else if index == 3 {
                    (77, if x < 0 { 1 } else { 3 })
                } else {
                    match (x.signum(), z.signum()) {
                        (-1, _) => (82, 0),
                        (1, _) => (85, 4),
                        _ => (
                            83,
                            terra_core::construction::merged_slab(83, 0, 85).unwrap(),
                        ),
                    }
                };
                cells.insert([pipe[0] + x, pipe[1], z], roof);
            }
        }
        for y in base..=base + 4 {
            cells.insert([pipe[0], y, 0], (60, 0));
        }
    }
    for (&position, &(material, facing)) in &cells {
        journal
            .record(position, material)
            .expect("Valid pipe roof fixture voxel");
        if material == 60 || terra_core::construction::is_shape(material) {
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .expect("Valid pipe roof fixture pose");
        }
    }
    for pipe in pipes {
        let pieces = terra_world::objects::chimney_infill_with_states(60, 0, |offset| {
            cells
                .get(&std::array::from_fn(|axis| pipe[axis] + offset[axis]))
                .copied()
        });
        assert!(
            !pieces.is_empty(),
            "Each roof must have three contacts or two opposing ridge profiles"
        );
        assert!(
            pieces
                .iter()
                .all(|(bounds, _)| !bounds.intersects([0.27; 3], [0.73; 3])),
            "Smoke lumen must stay free"
        );
    }
    let target = Vec3::new(9.5, base as f32 + 1.8, 0.5);
    let camera =
        Transform::from_translation(target + Vec3::new(7.0, 8.0, 22.0)).looking_at(target, Vec3::Y);
    tracing::info!(
        ?pipes,
        cells = cells.len(),
        "Pipe roof fixture seeded: full block, rotated stairs, mixed slabs and a two-sided pitched ridge"
    );
    commands.insert_resource(PipeRoofs {
        cells,
        pipes,
        camera,
        ready_at: None,
    });
}

fn inspect_pipe_roofs(
    world: CustomWorld,
    roofs: Option<ResMut<PipeRoofs>>,
    mut smoke: ResMut<CustomSmoke>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut focus: ResMut<ChunkFocus>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut roofs) = roofs else {
        return;
    };
    for mut camera in &mut camera {
        *camera = roofs.camera;
    }
    focus.0 = roofs.camera.translation.to_array().map(f64::from);
    if smoke.complete || smoke.reported_error {
        return;
    }
    if let Some(error) = world.renderer.error.clone().or_else(|| {
        (smoke.started.elapsed() > Duration::from_secs(120))
            .then(|| "Pipe roof fixture timed out".into())
    }) {
        tracing::error!(%error, "Pipe roof inspection failed");
        smoke.reported_error = true;
        exit.write(AppExit::error());
        return;
    }
    if world.generation.status != GenerationStatus::Ready || !world.renderer.ready {
        roofs.ready_at = None;
        return;
    }
    for (&position, &(material, facing)) in &roofs.cells {
        let (owner, _) = global_to_section(position);
        if world.chunks.block_at(position) != Some(material)
            || ((material == 60 || terra_core::construction::is_shape(material))
                && world
                    .journal
                    .object_state(position)
                    .is_none_or(|state| state.facing != facing))
            || world.rendered.sections.get(&owner).is_none_or(|section| {
                section.mesh_epoch != world.journal.mesh_epoch(owner) || section.meshes.is_empty()
            })
        {
            roofs.ready_at = None;
            return;
        }
    }
    if roofs.ready_at.get_or_insert_with(Instant::now).elapsed() < Duration::from_secs(1) {
        return;
    }
    smoke.complete = true;
    tracing::info!(pipes = ?roofs.pipes, "Pipe roof inspection passed: actual loaded voxels, orientation states, current uploaded mesh epochs and open lumens; full/stair/mixed-slab comparison ready for screenshot");
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
