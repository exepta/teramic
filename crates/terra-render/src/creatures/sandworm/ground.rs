//! Ground-projected cached wedge patches, with bounded, throttled voxel probes.
use super::super::{CreatureAnimationSet, CreatureAvatar, Limb, Models, Motion, model::Part};
use bevy::prelude::*;
use terra_voxel::MaterialId;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

#[derive(Component)]
pub(in crate::creatures) struct WaveGround {
    checked: f32,
    cells: [[i64; 2]; 5],
    anchor_y: i64,
    height: Option<(f32, bool)>,
}
impl Default for WaveGround {
    fn default() -> Self {
        Self {
            checked: f32::NEG_INFINITY,
            cells: [[i64::MIN; 2]; 5],
            anchor_y: i64::MIN,
            height: None,
        }
    }
}

pub(in crate::creatures) fn install(app: &mut App) {
    app.add_systems(Update, conform.after(CreatureAnimationSet));
}

type GroundPatch = (
    &'static ChildOf,
    &'static Limb,
    &'static mut Transform,
    &'static mut Visibility,
    &'static mut WaveGround,
    &'static mut MeshMaterial3d<StandardMaterial>,
);

fn conform(
    time: Res<Time>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    models: Option<Res<Models>>,
    roots: Query<(&Transform, &Motion), With<CreatureAvatar>>,
    mut patches: Query<GroundPatch, Without<CreatureAvatar>>,
) {
    for (parent, limb, mut local, mut visible, mut sample, mut material) in &mut patches {
        if *visible == Visibility::Hidden {
            continue;
        }
        let (Some(chunks), Some(journal), Ok((root, _))) =
            (&chunks, &journal, roots.get(parent.parent()))
        else {
            *visible = Visibility::Hidden;
            continue;
        };
        // Broad flat patches keep their full crest. At ordinary one-block
        // dune steps, fit a smaller crest onto its confirmed central column;
        // rejecting every broad patch made moving worms disappear on dunes.
        let footprint = if matches!(limb.0, Part::WormMound) {
            Vec2::new(0.97, 1.10)
        } else {
            Vec2::new(0.56, 0.32)
        };
        let probes = [
            Vec3::ZERO,
            Vec3::new(-footprint.x, 0.0, -footprint.y),
            Vec3::new(footprint.x, 0.0, -footprint.y),
            Vec3::new(-footprint.x, 0.0, footprint.y),
            Vec3::new(footprint.x, 0.0, footprint.y),
        ];
        let cells = probes.map(|p| {
            let world = root.transform_point(local.transform_point(p));
            [world.x.floor() as i64, world.z.floor() as i64]
        });
        let anchor_y = root.translation.y.floor() as i64;
        let now = time.elapsed_secs();
        if cells != sample.cells || anchor_y != sample.anchor_y || now - sample.checked >= 0.12 {
            sample.checked = now;
            sample.cells = cells;
            sample.anchor_y = anchor_y;
            sample.height =
                supported_surface(cells, |cell| surface(chunks, journal, cell, anchor_y));
            if let (Some(models), Some((height, _))) = (&models, sample.height) {
                let cell = [cells[0][0], height as i64 - 1, cells[0][1]];
                let block = journal.block(cell).or_else(|| chunks.block_at(cell));
                if let Some(block) = block {
                    apply_sand_material(&mut material, models, block);
                }
            }
        }
        if let Some((height, stepped)) = sample.height {
            if stepped {
                fit_to_cell(root, &mut local, footprint);
            }
            let lift = if matches!(limb.0, Part::WormMound) {
                local.translation.y
            } else {
                -0.018
            };
            local.translation.y = (height - root.translation.y) / root.scale.y.max(0.001) + lift;
        } else {
            *visible = Visibility::Hidden;
        }
    }
}

pub(super) fn apply_sand_material(
    material: &mut MeshMaterial3d<StandardMaterial>,
    models: &Models,
    block: u16,
) {
    let target = if matches!(
        MaterialId::from_raw(block),
        Some(MaterialId::RedDuneSand | MaterialId::RedSandstone)
    ) {
        &models.red_sand
    } else {
        &models.material
    };
    if &material.0 != target {
        material.0 = target.clone();
    }
}

/// Preserve a crest at small walkable dune steps while rejecting cliffs,
/// unloaded columns, water and all non-sand ground.
fn supported_surface(
    cells: [[i64; 2]; 5],
    mut sample: impl FnMut([i64; 2]) -> Option<f32>,
) -> Option<(f32, bool)> {
    let heights = cells.map(&mut sample);
    let center = heights[0]?;
    let mut low = center;
    let mut high = center;
    for height in heights {
        let height = height?;
        low = low.min(height);
        high = high.max(height);
    }
    (high - low <= 1.01).then_some((center, high - low > 0.05))
}

/// A projected crest must not bridge the unsupported side of a voxel step.
/// Shift at most8cm inside its centre column, then fit its whole footprint.
pub(super) fn fit_to_cell(root: &Transform, local: &mut Transform, footprint: Vec2) {
    let world = root.transform_point(local.translation);
    let center = Vec3::new(
        world
            .x
            .clamp(world.x.floor() + 0.08, world.x.floor() + 0.92),
        world.y,
        world
            .z
            .clamp(world.z.floor() + 0.08, world.z.floor() + 0.92),
    );
    let shifted = root.rotation.inverse() * (center - root.translation) / root.scale;
    local.translation.x = shifted.x;
    local.translation.z = shifted.z;
    let rotation = root.rotation * local.rotation;
    let x = rotation * Vec3::X * local.scale.x * footprint.x * root.scale.x;
    let z = rotation * Vec3::Z * local.scale.z * footprint.y * root.scale.z;
    let extent = Vec2::new(x.x.abs() + z.x.abs(), x.z.abs() + z.z.abs());
    let room = Vec2::new(
        (center.x - center.x.floor()).min(center.x.ceil() - center.x) - 0.015,
        (center.z - center.z.floor()).min(center.z.ceil() - center.z) - 0.015,
    );
    let factor = (room.x / extent.x.max(0.001))
        .min(room.y / extent.y.max(0.001))
        .clamp(0.001, 1.0);
    local.scale.x *= factor;
    local.scale.z *= factor;
}

pub(super) fn surface(
    chunks: &ChunkStore,
    journal: &EditJournal,
    [x, z]: [i64; 2],
    anchor_y: i64,
) -> Option<f32> {
    for y in ((anchor_y - 5)..=(anchor_y + 3)).rev() {
        // An edit is meaningful only when its underlying streamed voxel is known.
        let cell = [x, y, z];
        let edit = journal.block(cell);
        let block = edit.unwrap_or(chunks.block_at(cell)?);
        let material = MaterialId::from_raw(block)?;
        if matches!(material, MaterialId::SolidWater | MaterialId::FluidWater) {
            return None;
        }
        if material.is_solid() {
            return terra_core::creatures::sandworm::burrowable_ground(block, edit.is_some())
                .then_some((y + 1) as f32);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_uses_confirmed_voxels_and_rejects_a_removed_or_waterlogged_sand_floor() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let mut chunks = ChunkStore::default();
        let position = SectionPos::default();
        let mut data = BlockSection::empty(position).unwrap();
        data.set_block([4, 0, 4], MaterialId::Sand as u16).unwrap();
        chunks.loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        assert_eq!(surface(&chunks, &journal, [4, 4], 1), Some(1.0));
        for block in [MaterialId::DuneSand, MaterialId::RedDuneSand] {
            journal.record([4, 0, 4], block as u16).unwrap();
            assert_eq!(surface(&chunks, &journal, [4, 4], 1), Some(1.0));
        }
        journal
            .record([4, 0, 4], MaterialId::RedSandstone as u16)
            .unwrap();
        assert_eq!(surface(&chunks, &journal, [4, 4], 1), None);
        journal.record([4, 0, 4], MaterialId::Stone as u16).unwrap();
        assert_eq!(surface(&chunks, &journal, [4, 4], 1), None);
        journal.record([4, 0, 4], MaterialId::Sand as u16).unwrap();
        journal
            .record([4, 1, 4], MaterialId::FluidWater as u16)
            .unwrap();
        assert_eq!(surface(&chunks, &journal, [4, 4], 1), None);
        assert_eq!(surface(&ChunkStore::default(), &journal, [4, 4], 1), None);
    }

    #[test]
    fn crests_follow_small_dune_steps_but_reject_cliffs_or_unknown_ground() {
        let cells = [[0, 0], [0, 1], [1, 0], [1, 1], [0, 0]];
        assert_eq!(
            supported_surface(cells, |_| Some(32.0)),
            Some((32.0, false))
        );
        assert_eq!(
            supported_surface(cells, |[x, _]| Some(32.0 + x as f32)),
            Some((32.0, true))
        );
        assert!(supported_surface(cells, |[x, _]| Some(32.0 + x as f32 * 2.0)).is_none());
        assert!(supported_surface(cells, |cell| (cell != [1, 0]).then_some(32.0)).is_none());
    }

    #[test]
    fn fitted_crest_footprints_never_bridge_the_unsupported_side_of_a_step() {
        let footprint = Vec2::new(0.97, 1.10);
        for angle in [0.0, 0.4, 1.1, 2.6, 4.9] {
            for fraction in [0.0, 0.04, 0.5, 0.96] {
                let root = Transform::from_xyz(3.0 + fraction, 32.0, 4.04)
                    .with_rotation(Quat::from_rotation_y(angle));
                let mut local = Transform::from_scale(Vec3::new(2.7, 1.8, 2.4))
                    .with_rotation(Quat::from_rotation_y(0.7));
                fit_to_cell(&root, &mut local, footprint);
                for x in [-footprint.x, footprint.x] {
                    for z in [-footprint.y, footprint.y] {
                        let world =
                            root.transform_point(local.transform_point(Vec3::new(x, 0.0, z)));
                        assert!(world.x > 3.0 && world.x < 4.0);
                        assert!(world.z > 4.0 && world.z < 5.0);
                    }
                }
                assert_eq!(local.scale.y, 1.8);
            }
        }
    }
}
