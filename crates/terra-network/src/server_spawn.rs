//! Resolve the shared world spawn against procedural terrain and saved edits.
use super::ServerRuntime;
use bevy::{ecs::system::SystemParam, prelude::*};
use terra_physics::character::{PLAYER_HALF_HEIGHT, space_clear};
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    spawn::{WorldSpawn, find_land_spawn},
    streaming::{ChunkFocus, ChunkStore, WorldTerrain},
};

pub(super) fn initialize(world: &mut World) {
    let result = find_land_spawn(
        &world.resource::<WorldTerrain>().0,
        world.resource::<EditJournal>(),
        [8, 24],
    );
    match result {
        Ok(spawn) => {
            world.resource_mut::<ChunkFocus>().0 = body_position(spawn).to_array().map(f64::from);
            world.insert_resource(spawn);
            tracing::info!(ground = ?spawn.ground, "World spawn selected on dry solid terrain");
        }
        Err(error) => {
            world.resource_mut::<ServerRuntime>().error = Some(error.clone());
            tracing::error!(%error, "Cannot initialize a safe world spawn");
            world.write_message(AppExit::error());
        }
    }
}

fn body_position(spawn: WorldSpawn) -> Vec3 {
    Vec3::from_array(spawn.feet().map(|value| value as f32)) + Vec3::Y * (PLAYER_HALF_HEIGHT + 0.02)
}

#[derive(SystemParam)]
pub(super) struct SpawnWorld<'w> {
    terrain: Res<'w, WorldTerrain>,
    journal: Res<'w, EditJournal>,
    spawn: ResMut<'w, WorldSpawn>,
    focus: ResMut<'w, ChunkFocus>,
}

impl SpawnWorld<'_> {
    pub(super) fn position(&mut self) -> Result<Vec3, String> {
        if !self.spawn.is_valid(&self.terrain.0, &self.journal) {
            *self.spawn = find_land_spawn(
                &self.terrain.0,
                &self.journal,
                [self.spawn.ground[0], self.spawn.ground[2]],
            )?;
            self.focus.0 = body_position(*self.spawn).to_array().map(f64::from);
        }
        Ok(body_position(*self.spawn))
    }
}

pub(super) enum LoadedSpawn {
    Waiting,
    Invalid,
    Ready,
}

#[derive(SystemParam)]
pub(super) struct SpawnEnvironment<'w> {
    pub properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    pub config: Res<'w, terra_core::GameConfig>,
    pub chunks: Res<'w, ChunkStore>,
    pub terrain: Res<'w, WorldTerrain>,
    pub journal: Res<'w, EditJournal>,
}

pub(super) fn loaded_state(
    chunks: &ChunkStore,
    terrain: &TerraGenerator,
    journal: &EditJournal,
    position: Vec3,
) -> LoadedSpawn {
    let ground = spawn_at_body(position).ground;
    let Some(material) = chunks.block_at(ground) else {
        return LoadedSpawn::Waiting;
    };
    for height in 1..=3 {
        if chunks
            .block_at([ground[0], ground[1] + height, ground[2]])
            .is_none()
        {
            return LoadedSpawn::Waiting;
        }
    }
    if !terra_voxel::MaterialId::from_raw(material)
        .is_some_and(terra_voxel::MaterialId::is_natural_ground)
        || !space_clear(chunks, position)
        || !valid_position(terrain, journal, position)
    {
        return LoadedSpawn::Invalid;
    }
    LoadedSpawn::Ready
}

pub(super) fn valid_position(
    terrain: &TerraGenerator,
    journal: &EditJournal,
    position: Vec3,
) -> bool {
    spawn_at_body(position).is_valid(terrain, journal)
}

fn spawn_at_body(position: Vec3) -> WorldSpawn {
    WorldSpawn {
        ground: [
            position.x.floor() as i64,
            (position.y - PLAYER_HALF_HEIGHT).floor() as i64 - 1,
            position.z.floor() as i64,
        ],
    }
}
