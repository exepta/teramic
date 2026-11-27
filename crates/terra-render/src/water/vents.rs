//! Reuse the water-contact batch to animate nearby submerged vent mouths.
use bevy::prelude::*;
use terra_core::footsteps::{WaterMotion, WaterMotionKind};
use terra_voxel::{MaterialId, SectionPos, global_to_section};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
};

pub(super) fn emit(
    time: Res<Time>,
    mut last: Local<f32>,
    camera: Query<&GlobalTransform, With<terra_player::TerraCamera>>,
    chunks: Res<ChunkStore>,
    terrain: Res<WorldTerrain>,
    journal: Res<EditJournal>,
    mut motion: MessageWriter<WaterMotion>,
) {
    if time.elapsed_secs() - *last < 0.85 {
        return;
    }
    *last = time.elapsed_secs();
    let Ok(camera) = camera.single() else { return };
    let eye = camera.translation();
    let (center, _) = global_to_section(eye.floor().as_i64vec3().to_array());
    let vent = MaterialId::BubbleVent as u16;
    let mut remaining = 8;
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let pos = SectionPos::new(center.x + dx, center.y + dy, center.z + dz);
                let Some(section) = chunks.loaded.get(&pos).filter(|s| s.data.may_contain(vent))
                else {
                    continue;
                };
                for (idx, _) in section
                    .data
                    .blocks()
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| **m == vent)
                {
                    let Ok(cell) = pos.global_block([idx % 16, idx / 16 % 16, idx / 256]) else {
                        continue;
                    };
                    let source = Vec3::new(
                        cell[0] as f32 + 0.5,
                        cell[1] as f32 + 0.37,
                        cell[2] as f32 + 0.5,
                    );
                    if source.distance_squared(eye) > 18. * 18.
                        || chunks.block_at(cell) != Some(vent)
                        || chunks
                            .water_column_at_point(
                                &terrain.0,
                                &journal,
                                source.as_dvec3().to_array(),
                            )
                            .is_none_or(|water| water.level - f64::from(source.y) < 1.3)
                    {
                        continue;
                    }
                    motion.write(WaterMotion {
                        position: source,
                        velocity: Vec3::Y,
                        strength: 0.55,
                        kind: WaterMotionKind::Bubble,
                    });
                    remaining -= 1;
                    if remaining == 0 {
                        return;
                    }
                }
            }
        }
    }
}
