//! Realm-clock geothermal pulses shared by the server and the particle renderer.
pub const HEIGHT: f32 = 26.;
pub const BASE_RADIUS: f32 = 2.;
pub const CROWN_RADIUS: f32 = 4.;
pub const RAIN_RADIUS: f32 = 20.;
pub const DROPS: usize = 216;
pub(crate) const SUBMERGED_ECOLOGY_REVISION: u8 = 24;
pub(crate) const VENT_DEPTH: i64 = 4;

fn key([x, y, z]: [i64; 3]) -> u32 {
    let h = (x as u32).wrapping_mul(0x9e3779b1)
        ^ (y as u32).wrapping_mul(0x85ebca77)
        ^ (z as u32).wrapping_mul(0xc2b2ae3d);
    let h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
    h ^ (h >> 15)
}

fn cycle(seconds: f64, cell: [i64; 3]) -> (f64, f64) {
    let key = key(cell);
    let period = 10. + f64::from(key % 70) * 0.1;
    let phase = (seconds + f64::from(key >> 16) * 0.017).rem_euclid(period);
    (phase, period)
}

pub fn phase(seconds: f64, cell: [i64; 3]) -> f32 {
    cycle(seconds, cell).0 as f32
}

/// Fraction of the 2.4-second pressure warning immediately preceding each jet.
pub fn boil_progress(seconds: f64, cell: [i64; 3]) -> Option<f32> {
    const WARNING: f64 = 2.4;
    let (phase, period) = cycle(seconds, cell);
    let remaining = period - phase;
    (remaining < WARNING).then_some((1. - remaining / WARNING) as f32)
}

pub fn surge(seconds: f64, cell: [i64; 3]) -> f32 {
    let t = phase(seconds, cell);
    (t / 0.45).clamp(0., 1.) * ((3.2 - t) / 1.15).clamp(0., 1.) * (0.96 + 0.04 * (t * 11.).sin())
}

const GRAVITY: f32 = 12.;
const LAUNCH_LIFT: f32 = 1.5;
const DROP_HEIGHT: f32 = HEIGHT - 2.;
fn fall_time(y: f32) -> f32 {
    (LAUNCH_LIFT
        + (LAUNCH_LIFT.powi(2) + 2. * GRAVITY * (DROP_HEIGHT - y))
            .max(0.)
            .sqrt())
        / GRAVITY
}

/// Offsets from the vent mouth. Drops leave the crown over the full-pressure
/// plateau and accelerate until they hit the ground or a raised surface.
pub fn drop_position(cell: [i64; 3], phase: f32, index: usize) -> Option<[f32; 3]> {
    let h = key(cell).wrapping_add((index as u32).wrapping_mul(0x9e3779b1));
    let t = phase - (0.55 + 1.43 * (index as f32 / DROPS as f32));
    if t < 0. || t > fall_time(-5.) {
        return None;
    }
    let angle = (h >> 10) as f64 * (std::f64::consts::TAU / 4194304.);
    let reach = RAIN_RADIUS * ((h.rotate_left(11) >> 16) as f32 / 65535.).sqrt();
    let start = CROWN_RADIUS * reach / RAIN_RADIUS;
    let r = start + (reach - start) * t / fall_time(0.);
    Some([
        angle.cos() as f32 * r,
        DROP_HEIGHT + LAUNCH_LIFT * t - GRAVITY * 0.5 * t * t,
        angle.sin() as f32 * r,
    ])
}

/// The leading and trailing rain fronts pass the body's height at the same
/// instant as the rendered droplets. An early jet can only scald its own core.
pub fn heat(seconds: f64, cell: [i64; 3], feet: [f64; 3], body_height: f64) -> f32 {
    let radial = (feet[0] - cell[0] as f64 - 0.5).hypot(feet[2] - cell[2] as f64 - 0.5) as f32;
    let y = (feet[1] - cell[1] as f64 - 1.) as f32;
    let head = y + body_height as f32;
    if head < -5. || y > HEIGHT {
        return 0.;
    }
    let t = phase(seconds, cell);
    if y < HEIGHT * surge(seconds, cell)
        && radial <= BASE_RADIUS + (CROWN_RADIUS - BASE_RADIUS) * (y / HEIGHT).clamp(0., 1.)
        && t > 0.25
        && t < 3.
    {
        return 9.;
    }
    let arrival = fall_time(head.min(DROP_HEIGHT));
    let envelope = CROWN_RADIUS + (RAIN_RADIUS - CROWN_RADIUS) * fall_time(y) / fall_time(0.);
    if y <= DROP_HEIGHT
        && radial <= envelope.min(RAIN_RADIUS)
        && t >= 0.55 + arrival
        && t <= 1.98 + fall_time(y)
    {
        5.
    } else {
        0.
    }
}

#[cfg(feature = "bevy")]
pub fn submerged(
    chunks: &crate::streaming::ChunkStore,
    journal: &crate::edits::EditJournal,
    terrain: &crate::TerraGenerator,
    cell: [i64; 3],
) -> bool {
    use terra_voxel::MaterialId;
    let block = |p| {
        chunks
            .block_at(p)
            .map(|raw| journal.block(p).unwrap_or(raw))
    };
    if block(cell) != Some(MaterialId::GeyserVent as u16) {
        return false;
    }
    let [x, y, z] = cell;
    let Some(y) = y.checked_add(1) else {
        return false;
    };
    let mouth = [x, y, z];
    block(mouth).is_some_and(|raw| {
        MaterialId::from_raw(raw).is_some_and(|m| !m.is_solid())
            && chunks.fluid_cell_at(terrain, journal, mouth, raw).amount > 0
    })
}

#[cfg(feature = "bevy")]
pub fn open(
    chunks: &crate::streaming::ChunkStore,
    journal: &crate::edits::EditJournal,
    p: [i64; 3],
) -> bool {
    journal.block(p).or_else(|| chunks.block_at(p)).map_or_else(
        || {
            chunks
                .prepared_column([p[0].div_euclid(16), p[2].div_euclid(16)])
                .is_some_and(|c| p[1] as f64 > c.maximum_height())
        },
        |m| m == 0 || terra_voxel::MaterialId::from_raw(m).is_some_and(|m| !m.is_solid()),
    )
}

/// One cache per damage pass, reused across all player and creature bodies.
#[cfg(feature = "bevy")]
#[derive(Default)]
pub struct Vents(std::collections::HashMap<terra_voxel::SectionPos, Vec<[i64; 3]>>);
#[cfg(feature = "bevy")]
impl Vents {
    pub fn exposure(
        &mut self,
        chunks: &crate::streaming::ChunkStore,
        journal: &crate::edits::EditJournal,
        terrain: &crate::TerraGenerator,
        feet: [f64; 3],
        body_height: f64,
        seconds: f64,
    ) -> f32 {
        use terra_voxel::{MaterialId, SectionPos, global_to_section};
        let vent = MaterialId::GeyserVent as u16;
        let [x, y, z] = feet.map(|v| v.floor() as i64);
        let lo = global_to_section([x - 21, y - 28, z - 21]).0;
        let hi = global_to_section([x + 21, y + 6, z + 21]).0;
        let mut damage = 0_f32;
        for sz in lo.z..=hi.z {
            for sy in lo.y..=hi.y {
                for sx in lo.x..=hi.x {
                    let owner = SectionPos::new(sx, sy, sz);
                    let Some(section) = chunks
                        .loaded
                        .get(&owner)
                        .filter(|s| s.data.may_contain(vent))
                    else {
                        continue;
                    };
                    let vents = self.0.entry(owner).or_insert_with(|| {
                        section
                            .data
                            .blocks()
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| **m == vent)
                            .filter_map(|(i, _)| {
                                owner.global_block([i % 16, i / 16 % 16, i / 256]).ok()
                            })
                            .collect()
                    });
                    for &p in vents.iter() {
                        let dose = heat(seconds, p, feet, body_height);
                        if dose <= damage || !submerged(chunks, journal, terrain, p) {
                            continue;
                        }
                        let top = p[1] + 1 + HEIGHT as i64;
                        let head = (feet[1] + body_height).floor() as i64;
                        let jet_top = if dose >= 9. {
                            head.min(top).max(p[1] + 1)
                        } else {
                            top
                        };
                        if (p[1] + 1..=jet_top).all(|cy| open(chunks, journal, [p[0], cy, p[2]]))
                            && (dose >= 9.
                                || (head..=top).all(|cy| open(chunks, journal, [x, cy, z])))
                        {
                            damage = dose;
                        }
                    }
                }
            }
        }
        damage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boiling_warns_before_the_jet_and_is_quiet_between_cycles() {
        for cell in [[20, 30, -40], [2389, 26, -4659], [0, -16, 0]] {
            let (phase, period) = cycle(0., cell);
            let jet = period - phase;
            assert!(boil_progress(jet - 2.5, cell).is_none());
            let early = boil_progress(jet - 2.2, cell).unwrap();
            let late = boil_progress(jet - 0.1, cell).unwrap();
            assert!(early > 0. && early < 0.1);
            assert!(late > 0.9 && late < 1.);
            assert_eq!(surge(jet - 0.1, cell), 0.);
            assert!(boil_progress(jet + 0.1, cell).is_none());
            assert!(surge(jet + 0.1, cell) > 0.);
            assert!(boil_progress(jet + 5., cell).is_none());
        }
    }

    #[test]
    fn droplets_and_scalding_arrive_together_at_the_outer_edge_after_the_jet_peaks() {
        let p = [20, 30, -40];
        let feet = [39., 31., -39.5];
        let mut seen = false;
        let mut highest = 0_f32;
        for i in 0..1800 {
            let t = f64::from(i) / 100.;
            highest = highest.max(HEIGHT * surge(t, p));
            if heat(t, p, feet, 1.8) > 0. {
                assert_eq!(heat(t, p, [41., 31., -39.5], 1.8), 0.);
                seen |= (0..DROPS)
                    .filter_map(|i| drop_position(p, phase(t, p), i))
                    .any(|[x, y, z]| x.hypot(z) >= 17. && (0.0..2.0).contains(&y));
            }
        }
        assert!(seen && highest > 25.);
    }
    #[cfg(feature = "bevy")]
    #[test]
    fn loaded_outer_rain_hurts_an_exposed_player_and_a_roof_protects_them() {
        use crate::{
            TerraGenerator,
            edits::EditJournal,
            streaming::{ChunkStore, LoadedSection},
            water::FluidCell,
        };
        use terra_voxel::{BlockSection, MaterialId, SectionPos};
        let cell = [4, 1, 4];
        let feet = [23., 2., 4.5];
        let mut chunks = ChunkStore::default();
        for sx in 0..=1 {
            for sy in 0..=1 {
                let owner = SectionPos::new(sx, sy, 0);
                let data = BlockSection::empty(owner).unwrap();
                chunks.loaded.insert(
                    owner,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
            }
        }
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([4, 1, 4], MaterialId::GeyserVent as u16)
            .unwrap();
        let t = (0..1800)
            .map(|i| f64::from(i) / 100.)
            .find(|t| heat(*t, cell, feet, 1.8) > 0.)
            .unwrap();
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut journal = EditJournal::default();
        let mouth = [4, 2, 4];
        journal.record(mouth, MaterialId::Air as u16).unwrap();
        journal.set_fluid(mouth, FluidCell::default()).unwrap();
        assert_eq!(
            Vents::default().exposure(&chunks, &journal, &terrain, feet, 1.8, t),
            0.
        );
        journal
            .set_fluid(
                mouth,
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0; 2],
                },
            )
            .unwrap();
        assert!(submerged(&chunks, &journal, &terrain, cell));
        assert_eq!(
            Vents::default().exposure(&chunks, &journal, &terrain, feet, 1.8, t),
            5.
        );
        chunks
            .loaded
            .get_mut(&SectionPos::new(1, 0, 0))
            .unwrap()
            .data
            .set_block([7, 5, 4], MaterialId::Stone as u16)
            .unwrap();
        assert_eq!(
            Vents::default().exposure(&chunks, &journal, &terrain, feet, 1.8, t),
            0.
        );
    }
}
