//! Bounded authoritative projectiles. The replicated attack phase draws their arc.
use terra_core::creatures::sandworm;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

const MAX_PROJECTILES: usize = 32;
#[derive(Clone, Copy)]
struct Projectile {
    origin: [f64; 3],
    velocity: [f64; 3],
    elapsed: f32,
}
impl Projectile {
    fn at(self, time: f32) -> [f64; 3] {
        std::array::from_fn(|axis| {
            self.origin[axis] + self.velocity[axis] * f64::from(time)
                - if axis == 1 {
                    0.5 * f64::from(sandworm::SPIT_GRAVITY) * f64::from(time).powi(2)
                } else {
                    0.0
                }
        })
    }
}
#[derive(Default)]
pub(super) struct Projectiles(Vec<Projectile>);
impl Projectiles {
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
    pub(super) fn launch(&mut self, origin: [f64; 3], velocity: [f64; 3]) {
        if self.0.len() < MAX_PROJECTILES && origin.iter().chain(&velocity).all(|v| v.is_finite()) {
            self.0.push(Projectile {
                origin,
                velocity,
                elapsed: 0.0,
            });
        }
    }
    pub(super) fn advance(&mut self, chunks: &mut ChunkStore, journal: &mut EditJournal, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.0.retain_mut(|projectile| {
            let from = projectile.at(projectile.elapsed);
            projectile.elapsed += dt.clamp(0.0, 0.25);
            let to = projectile.at(projectile.elapsed);
            let delta: [f64; 3] = std::array::from_fn(|i| to[i] - from[i]);
            let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
            if distance > 0.0001 {
                let direction = delta.map(|v| v / distance);
                let hit = terra_voxel::blocks::raycast_blocks(from, direction, distance, |cell| {
                    let raw = chunks
                        .block_at(cell)
                        .map(|raw| journal.block(cell).unwrap_or(raw));
                    // Unknown space terminates a projectile; it never generates terrain.
                    Some(raw.map_or(terra_voxel::MaterialId::Stone as u16, |raw| {
                        if terra_voxel::MaterialId::from_raw(raw).is_none_or(|m| m.is_solid()) {
                            raw
                        } else {
                            0
                        }
                    }))
                });
                if let Some(hit) = hit {
                    if chunks.block_at(hit.position).is_none() {
                        return false;
                    }
                    let cell = std::array::from_fn(|i| hit.position[i] + i64::from(hit.normal[i]));
                    if let Err(error) = terra_world::bile::deposit_from_spit(chunks, journal, cell)
                    {
                        tracing::warn!(%error, "Sandworm bile impact rejected");
                    }
                    return false;
                }
            }
            projectile.elapsed < sandworm::SPIT_SECONDS + 0.3
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_projectile_arc_matches_the_visible_cached_droplets_and_has_a_fixed_budget() {
        let p = Projectile {
            origin: [0.0, 2.3, 0.0],
            velocity: [
                0.0,
                f64::from(sandworm::SPIT_UPWARD),
                -f64::from(sandworm::SPIT_SPEED),
            ],
            elapsed: 0.0,
        };
        for tick in 0..=8 {
            let t = tick as f32 * 0.1;
            let offset = sandworm::spit_offset(t);
            let position = p.at(t);
            assert!((position[1] - 2.3 - f64::from(offset[1])).abs() < 1e-6);
            assert!((position[2] - f64::from(offset[2])).abs() < 1e-6);
        }
        let mut queue = Projectiles::default();
        for _ in 0..10_000 {
            queue.launch(p.origin, p.velocity);
        }
        assert_eq!(queue.0.len(), MAX_PROJECTILES);
        queue.clear();
        assert!(queue.0.is_empty());
    }
}

#[cfg(test)]
mod impact_tests {
    use super::*;
    use terra_voxel::{BlockSection, MaterialId, SectionPos};
    use terra_world::streaming::LoadedSection;
    #[test]
    fn a_real_spit_hits_confirmed_ground_and_deposits_a_temporary_scoopable_acid_pool() {
        let mut chunks = ChunkStore::default();
        // The faster real spit crosses z=0 before touching the floor. Keep the
        // actual landing chunk loaded; unknown-space cancellation is checked
        // independently below and must not stand in for a ground impact.
        for sz in -1..=0 {
            let position = SectionPos::new(0, 0, sz);
            let mut data = BlockSection::empty(position).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 0, z], MaterialId::Sand as u16).unwrap();
                }
            }
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
        }
        let mut journal = EditJournal::default();
        let mut projectiles = Projectiles::default();
        projectiles.launch(
            [8.5, 3.3, 12.5],
            [
                0.0,
                f64::from(sandworm::SPIT_UPWARD),
                -f64::from(sandworm::SPIT_SPEED),
            ],
        );
        for _ in 0..12 {
            projectiles.advance(&mut chunks, &mut journal, 0.1);
        }
        assert!(
            projectiles.0.is_empty(),
            "terrain contact consumes the projectile"
        );
        let pools: Vec<_> = journal
            .object_states()
            .filter(|(_, id, _)| *id == terra_core::bile::FLUID)
            .collect();
        assert_eq!(
            pools.len(),
            1,
            "the real transaction must create one acid source"
        );
        assert_eq!(pools[0].0[1], 1);
        assert!(pools[0].2.liquid.is_some_and(|state| state.source
            && state.volume == terra_core::bile::CAPACITY
            && state.age & terra_core::bile::LIFETIME_MASK == terra_core::bile::SPIT_STEPS));
        assert_eq!(chunks.block_at(pools[0].0), Some(terra_core::bile::FLUID));
        let mut empty = ChunkStore::default();
        let mut untouched = EditJournal::default();
        projectiles.launch([0.0, 3.0, 0.0], [0.0, 0.0, -8.0]);
        projectiles.advance(&mut empty, &mut untouched, 0.1);
        assert!(projectiles.0.is_empty());
        assert_eq!(
            untouched.edited_blocks(),
            0,
            "unknown chunks never receive acid edits"
        );
    }
}
