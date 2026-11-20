//! Disposable benchmark chest placed through the integrated authority.
use bevy::prelude::*;
use std::time::{Duration, Instant};
use terra_network::{
    client::{ClientSession, InventoryRequest, SelectedInventorySlot},
    protocol::{
        inventory::{InventoryAction, InventoryCommand},
        session::EditOutcome,
    },
};
use terra_player::{TargetWorld, TerraCamera, WorldTarget};
use terra_voxel::{MaterialId, global_to_section};
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkStore, WorldTerrain},
};

#[derive(Clone, Copy, Debug)]
enum Stage {
    Select,
    Grant,
    Aim,
    Confirm { request: u64, revision: u64 },
    Ready,
}

pub(crate) struct ChestFixture {
    stage: Stage,
    entered: Option<Instant>,
    ground: [i64; 3],
    material: u16,
}

impl Default for ChestFixture {
    fn default() -> Self {
        Self {
            stage: Stage::Select,
            entered: None,
            ground: [0; 3],
            material: MaterialId::Chest as u16,
        }
    }
}

impl ChestFixture {
    pub(crate) fn for_material(material: u16) -> Self {
        Self {
            material,
            ..default()
        }
    }

    pub(crate) fn position(&self) -> [i64; 3] {
        [self.ground[0], self.ground[1] + 1, self.ground[2]]
    }

    pub(crate) fn prepare(&mut self, world: &mut World) -> Result<bool, String> {
        let session = world.resource::<ClientSession>();
        if session.game_mode != terra_core::GameMode::Creative || !session.allow_build {
            return Err("The chest benchmark requires an explicit Creative configuration with building permission for its authoritative fixture placement".into());
        }
        let elapsed = self.entered.get_or_insert_with(Instant::now).elapsed();
        if elapsed > Duration::from_secs(8) {
            return Err(format!(
                "Authoritative benchmark chest {:?} exceeded eight seconds",
                self.stage
            ));
        }
        match self.stage {
            Stage::Select => {
                let Some(ground) = choose_ground(world) else {
                    return Ok(false);
                };
                self.ground = ground;
                let revision = world.resource::<ClientSession>().inventory.revision;
                world.write_message(InventoryRequest(InventoryCommand {
                    revision,
                    action: InventoryAction::CreativeSet {
                        slot: 0,
                        item: terra_core::ItemKind::Block(self.material),
                        count: 1,
                    },
                }));
                self.advance(Stage::Grant);
            }
            Stage::Grant => {
                if !world
                    .resource::<ClientSession>()
                    .inventory
                    .slot(0)
                    .is_some_and(|stack| {
                        stack.valid() && stack.item == terra_core::ItemKind::Block(self.material)
                    })
                {
                    return Ok(false);
                }
                world.resource_mut::<SelectedInventorySlot>().0 = Some(0);
                world.write_message(terra_ui::InventoryAction::Hotbar(0));
                self.advance(Stage::Aim);
            }
            Stage::Aim => {
                aim(world, self.ground)?;
                if elapsed < Duration::from_millis(350)
                    || !matches!(world.resource::<TargetWorld>().0,
                        Some(WorldTarget::Block(hit)) if hit.position == self.ground && hit.adjacent() == self.position())
                {
                    return Ok(false);
                }
                let position = self.position();
                if world.resource::<ChunkStore>().block_at(position) != Some(0) {
                    return Err("Benchmark chest placement cell is no longer empty".into());
                }
                let request = world
                    .resource::<ClientSession>()
                    .last_edit
                    .map_or(0, |(id, _)| id);
                let revision = world
                    .resource::<EditJournal>()
                    .revision(global_to_section(position).0);
                world.write_message(BlockEditRequest {
                    position,
                    expected_material: 0,
                    material: self.material,
                });
                self.advance(Stage::Confirm { request, revision });
            }
            Stage::Confirm { request, revision } => {
                let Some((actual, outcome)) = world.resource::<ClientSession>().last_edit else {
                    return Ok(false);
                };
                if actual <= request {
                    return Ok(false);
                }
                if outcome != EditOutcome::Applied {
                    return Err(format!(
                        "Authority rejected benchmark chest placement: {outcome:?}"
                    ));
                }
                let position = self.position();
                if world.resource::<ChunkStore>().block_at(position) != Some(self.material)
                    || world
                        .resource::<EditJournal>()
                        .revision(global_to_section(position).0)
                        <= revision
                {
                    return Ok(false);
                }
                tracing::info!(
                    ?position,
                    material = self.material,
                    request = actual,
                    "Inventory benchmark chest fixture created through a real grant and acknowledged block edit; disposable world retains the chest"
                );
                self.advance(Stage::Ready);
            }
            Stage::Ready => return Ok(true),
        }
        Ok(false)
    }

    fn advance(&mut self, stage: Stage) {
        self.stage = stage;
        self.entered = Some(Instant::now());
    }
}

fn aim(world: &mut World, ground: [i64; 3]) -> Result<(), String> {
    let mut cameras = world.query_filtered::<&mut Transform, With<TerraCamera>>();
    let mut camera = cameras
        .iter_mut(world)
        .next()
        .ok_or("No real camera for benchmark chest placement")?;
    camera.look_at(
        Vec3::new(
            ground[0] as f32 + 0.5,
            ground[1] as f32 + 0.99,
            ground[2] as f32 + 0.5,
        ),
        Vec3::Y,
    );
    Ok(())
}

pub(crate) fn choose_ground(world: &mut World) -> Option<[i64; 3]> {
    let body = Vec3::from_array(
        world
            .resource::<ClientSession>()
            .confirmed_state()?
            .position,
    );
    let eye = world
        .query_filtered::<&Transform, With<TerraCamera>>()
        .iter(world)
        .next()?
        .translation;
    if eye.distance(body + Vec3::Y * terra_player::PLAYER_EYE_OFFSET) > 0.5 {
        return None;
    }
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let properties = world.resource::<terra_core::block_properties::BlockPropertiesCatalog>();
    let terrain = &world.resource::<WorldTerrain>().0;
    let base = body.floor().as_i64vec3().to_array();
    let pose = terra_world::props::PropPose::WeatherSwept {
        frame: terra_world::props::weather_wind_frame(
            world.resource::<terra_core::WorldWeather>().wind(),
        ),
    };
    for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        for dy in -3..=0 {
            let ground = [base[0] + dx, base[1] + dy, base[2] + dz];
            let position = [ground[0], ground[1] + 1, ground[2]];
            let Some(material) = chunks.block_at(ground) else {
                continue;
            };
            if !MaterialId::from_raw(material).is_some_and(MaterialId::is_opaque)
                || !properties.shape(material).is_empty()
                || chunks.block_at(position) != Some(0)
                || chunks.fluid_cell_at(terrain, journal, position, 0).amount != 0
            {
                continue;
            }
            let top = Vec3::new(
                ground[0] as f32 + 0.5,
                ground[1] as f32 + 0.99,
                ground[2] as f32 + 0.5,
            );
            if top.distance(eye) > 4.0 {
                continue;
            }
            let (_, target) = terra_world::targeting::pick_world_with_context(
                chunks,
                journal,
                properties,
                eye.to_array().map(f64::from),
                (top - eye).normalize_or_zero().to_array().map(f64::from),
                pose,
            );
            if matches!(target, Some(WorldTarget::Block(hit)) if hit.position == ground && hit.adjacent() == position)
            {
                return Some(ground);
            }
        }
    }
    None
}
