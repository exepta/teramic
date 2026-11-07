//! Roof and coastal payloads captured from streamed data, never generation jobs.
use std::{collections::VecDeque, sync::Arc};
use terra_voxel::{MaterialId, SectionPos};
use terra_world::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};

pub(crate) struct Exposure {
    pub natural: Option<Arc<terra_world::PreparedColumn>>,
    pub roofs: [i16; 256],
    /// Snow sees through unedited, naturally deciduous crowns after leaf fall.
    pub snow_roofs: [i16; 256],
    pub coast: [f32; 256],
    /// Shared grid-corner distances prevent square, whole-cell wash fronts.
    pub coast_vertices: [f32; 289],
    /// Highest open water above each roof. Negative infinity means dry.
    pub water: [f32; 256],
}

/// Fluid simulation changes water heights every tick, but terrain only needs a
/// new rain payload when its roof crosses the waterline, not for every ripple.
pub(crate) struct RoofWaterState {
    roofs: [i16; 256],
    mask: [i16; 256],
    fluid_revision: u64,
    pub epoch: u64,
}

impl RoofWaterState {
    pub fn captured(exposure: &Exposure, fluid_revision: u64, epoch: u64) -> Self {
        Self {
            roofs: exposure.roofs,
            mask: waterlines(&exposure.water),
            fluid_revision,
            epoch,
        }
    }

    pub fn refresh(
        &mut self,
        chunks: &ChunkStore,
        column: [i64; 2],
        terrain: &TerraGenerator,
        journal: &EditJournal,
        fluid_revision: u64,
    ) {
        if self.fluid_revision == fluid_revision {
            return;
        }
        self.fluid_revision = fluid_revision;
        let water = roof_water(chunks, column, &self.roofs, terrain, journal);
        let mask = waterlines(&water);
        if self.mask != mask {
            self.mask = mask;
            self.epoch = self.epoch.wrapping_add(1);
        }
    }
}

fn waterlines(water: &[f32; 256]) -> [i16; 256] {
    water.map(|height| {
        if height.is_finite() {
            (height - 0.001).ceil() as i16
        } else {
            i16::MIN
        }
    })
}

fn roof_water(
    chunks: &ChunkStore,
    column: [i64; 2],
    roofs: &[i16; 256],
    terrain: &TerraGenerator,
    journal: &EditJournal,
) -> [f32; 256] {
    let mut water = [f32::NEG_INFINITY; 256];
    let prepared = chunks.prepared_column(column);
    for z in 0..16 {
        for x in 0..16 {
            let i = x + z * 16;
            let global_x = column[0] * 16 + x as i64;
            let global_z = column[1] * 16 + z as i64;
            if terrain.config().water
                && let Some(surface) = prepared
                    .as_ref()
                    .and_then(|p| p.surface_at(global_x, global_z))
                && let Some(level) = surface
                    .water_level
                    .filter(|level| *level > surface.water_bed())
            {
                // The canopy may be above a lake: its highest solid roof must
                // not hide the actual water covering the ground below it.
                water[i] = level as f32;
            }
            if chunks.block_at([global_x, i64::from(roofs[i]), global_z])
                == Some(MaterialId::SolidWater as u16)
            {
                water[i] = water[i].max(f32::from(roofs[i]) + 1.0);
            }
        }
    }
    if !terrain.config().water {
        return water;
    }
    let mut include = |position: SectionPos| {
        for (p, fluid) in journal.fluid_cells_in(position) {
            if fluid.amount == 0 {
                continue;
            }
            let i = p[0].rem_euclid(16) as usize + p[2].rem_euclid(16) as usize * 16;
            water[i] = water[i].max(p[1] as f32 + fluid.height() as f32);
        }
    };
    if terrain.config().bounded_world {
        for y in (-128_i64).div_euclid(16)..=356_i64.div_euclid(16) {
            include(SectionPos::new(column[0], y, column[1]));
        }
    } else {
        for section in journal.changed_sections().filter(|p| [p.x, p.z] == column) {
            include(section);
        }
    }
    water
}

impl Exposure {
    pub fn coastal(&self, sea: f64) -> bool {
        self.roofs
            .iter()
            .any(|&roof| (f64::from(roof) + 1.0 - sea).abs() <= 0.65)
    }

    /// Empty sky and deep interior publications cannot change a captured roof.
    /// Removing/replacing its actual section must still trigger a fresh mask.
    pub fn section_affects(
        &self,
        position: SectionPos,
        section: Option<&terra_voxel::BlockSection>,
        sea: f64,
    ) -> bool {
        let low = position.y.saturating_mul(16);
        let high = low.saturating_add(15);
        if self.roofs.iter().chain(&self.snow_roofs).any(|&roof| {
            let roof = i64::from(roof);
            (low..=high).contains(&roof)
        }) || self.coastal(sea) && (low..=high).contains(&water_cell_y(sea))
        {
            return true;
        }
        section.is_some_and(|section| {
            !section.is_empty()
                && self
                    .roofs
                    .iter()
                    .chain(&self.snow_roofs)
                    .any(|&roof| i64::from(roof) <= high)
                && section
                    .blocks()
                    .iter()
                    .any(|&block| MaterialId::from_raw(block).is_some_and(MaterialId::is_solid))
        })
    }

    /// Source Arcs may change without changing any vertex exposure payload.
    pub fn same_masks(&self, other: &Self) -> bool {
        self.roofs == other.roofs
            && self.snow_roofs == other.snow_roofs
            && waterlines(&self.water) == waterlines(&other.water)
            && self.coast == other.coast
            && self.coast_vertices == other.coast_vertices
    }

    pub fn capture(
        chunks: &ChunkStore,
        column: [i64; 2],
        sea: f64,
        bounded: bool,
        terrain: &TerraGenerator,
        journal: &EditJournal,
    ) -> Option<Arc<Self>> {
        let _scope = terra_core::profiling::ScopeTimer::new("terrain.exposure_snapshot");
        let prepared = chunks.prepared_column(column)?;
        let mut roofs = [i16::MIN; 256];
        let mut snow_roofs = [i16::MIN; 256];
        let sections: Vec<_> = if bounded {
            // Same vertical limits as bounded voxel generation, independent of
            // the total number of distant loaded sections.
            const MIN_Y: i64 = -128;
            const MAX_Y: i64 = 356;
            (MIN_Y.div_euclid(16)..=MAX_Y.div_euclid(16))
                .rev()
                .filter_map(|y| {
                    chunks
                        .loaded
                        .get_key_value(&SectionPos::new(column[0], y, column[1]))
                })
                .collect()
        } else {
            // Old worlds can explicitly retain unbounded generation.
            let mut sections: Vec<_> = chunks
                .loaded
                .iter()
                .filter(|(p, _)| [p.x, p.z] == column)
                .collect();
            sections.sort_unstable_by_key(|(p, _)| std::cmp::Reverse(p.y));
            sections
        };
        for (position, section) in sections {
            for z in 0..16 {
                for x in 0..16 {
                    let i = x + z * 16;
                    if roofs[i] != i16::MIN && snow_roofs[i] != i16::MIN {
                        continue;
                    }
                    for y in (0..16).rev() {
                        let material = section.data.block([x, y, z]).ok()?;
                        if !MaterialId::from_raw(material).is_some_and(MaterialId::is_solid) {
                            continue;
                        }
                        let global = [
                            column[0] * 16 + x as i64,
                            position.y * 16 + y as i64,
                            column[1] * 16 + z as i64,
                        ];
                        if roofs[i] == i16::MIN {
                            roofs[i] = global[1] as i16;
                        }
                        if snow_roofs[i] == i16::MIN {
                            let canopy = matches!(material, 6..=10 | 36);
                            let natural = canopy
                                .then(|| prepared.tree_appearance_at(global, material))
                                .flatten();
                            if !snow_transparent(
                                material,
                                natural,
                                canopy && journal.block(global).is_some(),
                            ) {
                                snow_roofs[i] = global[1] as i16;
                            }
                        }
                        if roofs[i] != i16::MIN && snow_roofs[i] != i16::MIN {
                            break;
                        }
                    }
                }
            }
        }
        for z in 0..16 {
            for x in 0..16 {
                if let Some(surface) = prepared.surface_at(column[0] * 16 + x, column[1] * 16 + z) {
                    let nominal = (surface.height - 0.5).floor() as i64;
                    if !chunks.loaded.contains_key(&SectionPos::new(
                        column[0],
                        nominal.div_euclid(16),
                        column[1],
                    )) {
                        roofs[x as usize + z as usize * 16] =
                            roofs[x as usize + z as usize * 16].max(nominal as i16);
                        snow_roofs[x as usize + z as usize * 16] =
                            snow_roofs[x as usize + z as usize * 16].max(nominal as i16);
                    }
                }
            }
        }
        let water = roof_water(chunks, column, &roofs, terrain, journal);
        let mut coast = [0.0; 256];
        let mut coast_vertices = [0.0; 289];
        if !roofs
            .iter()
            .any(|&roof| (f64::from(roof) + 1.0 - sea).abs() <= 0.65)
        {
            return Some(Arc::new(Self {
                natural: Some(prepared),
                roofs,
                snow_roofs,
                coast,
                coast_vertices,
                water,
            }));
        }
        // Only sea-level open water connected to an ocean seed receives surf.
        // A one-column halo keeps the useful 2.8m wash independent of mesh seams.
        const N: usize = 48;
        let neighbors: Vec<_> = (-1..=1)
            .flat_map(|z| (-1..=1).map(move |x| [column[0] + x, column[1] + z]))
            .map(|p| chunks.prepared_column(p))
            .collect();
        let sample = |x: usize, z: usize| {
            neighbors[x / 16 + (z / 16) * 3].as_ref()?.surface_at(
                column[0] * 16 + x as i64 - 16,
                column[1] * 16 + z as i64 - 16,
            )
        };
        let mut sea_water = vec![false; N * N];
        let mut connected = vec![false; N * N];
        let mut queue = VecDeque::new();
        for z in 0..N {
            for x in 0..N {
                if let Some(surface) = sample(x, z) {
                    let i = x + z * N;
                    let global = [
                        column[0] * 16 + x as i64 - 16,
                        water_cell_y(sea),
                        column[1] * 16 + z as i64 - 16,
                    ];
                    sea_water[i] = surface
                        .water_level
                        .is_some_and(|level| (level - sea).abs() < 0.01)
                        && (surface.height - 0.5).floor() + 1.0 < sea
                        && !chunks
                            .block_at(global)
                            .and_then(MaterialId::from_raw)
                            .is_some_and(MaterialId::is_solid);
                    if sea_water[i] && surface.biome.weights.ocean > 0.2 {
                        connected[i] = true;
                        queue.push_back(i);
                    }
                }
            }
        }
        connect_ocean(&sea_water, &mut connected, queue, N);
        for z in 0..=16 {
            for x in 0..=16 {
                coast_vertices[x + z * 17] =
                    coast_reach(&connected, N, x as f32 + 16.0, z as f32 + 16.0);
            }
        }
        for z in 0..16 {
            for x in 0..16 {
                if sample(x + 16, z + 16).is_none() {
                    continue;
                }
                let top = f64::from(roofs[x + z * 16]) + 1.0;
                if top < sea - 0.05 || top > sea + 0.65 {
                    continue;
                }
                coast[x + z * 16] = coast_reach(&connected, N, x as f32 + 16.5, z as f32 + 16.5);
            }
        }
        Some(Arc::new(Self {
            natural: Some(prepared),
            roofs,
            snow_roofs,
            coast,
            coast_vertices,
            water,
        }))
    }

    pub fn payload(&self, local: [u32; 3], origin_y: i64) -> f32 {
        let i = local[0] as usize + local[2] as usize * 16;
        let y = origin_y + i64::from(local[1]);
        let submerged = self
            .natural
            .as_ref()
            .and_then(|p| p.local_surface(local[0] as usize, local[2] as usize))
            .map_or(y as f32 + 1.0 < self.water[i] - 0.001, |column| {
                column
                    .water_columns()
                    .intervals
                    .into_iter()
                    .flatten()
                    .any(|w| y as f64 >= w.bed - 1.0 && (y as f64 + 1.0) < w.level - 0.001)
                    || (self.water[i].is_finite()
                        && column
                            .water_level
                            .is_none_or(|level| self.water[i] > level as f32 + 0.01)
                        && y >= i64::from(self.roofs[i]) - 1
                        && y as f32 + 1.0 < self.water[i] - 0.001)
            });
        if submerged {
            // This also suppresses an old saved rain condition while submerged.
            -1.0
        } else if y >= i64::from(self.roofs[i]) {
            1.0 + self.coast[i]
        } else {
            0.0
        }
    }

    pub fn snow_exposed(&self, local: [u32; 3], origin_y: i64) -> bool {
        let i = local[0] as usize + local[2] as usize * 16;
        origin_y + i64::from(local[1]) >= i64::from(self.snow_roofs[i])
            && self.payload(local, origin_y) >= 0.0
    }
}

fn snow_transparent(
    material: u16,
    natural: Option<terra_world::generation::TreeAppearance>,
    edited: bool,
) -> bool {
    matches!(material, 6..=10 | 36) && !edited && natural.is_some()
}

fn coast_reach(connected: &[bool], n: usize, x: f32, z: f32) -> f32 {
    let mut distance_squared = f32::INFINITY;
    for dz in -3..=3 {
        for dx in -3..=3 {
            let nx = x.floor() as i32 + dx;
            let nz = z.floor() as i32 + dz;
            if nx < 0 || nz < 0 || nx >= n as i32 || nz >= n as i32 {
                continue;
            }
            if connected[nx as usize + nz as usize * n] {
                distance_squared = distance_squared
                    .min((x - nx as f32 - 0.5).powi(2) + (z - nz as f32 - 0.5).powi(2));
            }
        }
    }
    (1.0 - distance_squared.sqrt() / 2.8).clamp(0.0, 1.0)
}

fn water_cell_y(level: f64) -> i64 {
    (level.ceil() as i64).saturating_sub(1)
}

fn connect_ocean(water: &[bool], connected: &mut [bool], mut queue: VecDeque<usize>, n: usize) {
    while let Some(i) = queue.pop_front() {
        for next in neighbors4(i, n).into_iter().flatten() {
            if water[next] && !connected[next] {
                connected[next] = true;
                queue.push_back(next);
            }
        }
    }
}

fn neighbors4(i: usize, n: usize) -> [Option<usize>; 4] {
    [
        (!i.is_multiple_of(n)).then(|| i - 1),
        (i % n + 1 < n).then_some(i + 1),
        (i >= n).then(|| i - n),
        (i + n < n * n).then_some(i + n),
    ]
}

#[cfg(test)]
#[path = "exposure_removal_tests.rs"]
mod removal_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_sky_publications_do_not_recapture_roofs_but_real_roofs_can_never_be_skipped() {
        let exposure = Exposure {
            natural: None,
            roofs: [8; 256],
            snow_roofs: [3; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        let mut recaptures = 0;
        for y in 1..=20 {
            let position = SectionPos::new(-2, y, -3);
            let empty = terra_voxel::BlockSection::empty(position).unwrap();
            recaptures += usize::from(exposure.section_affects(position, Some(&empty), 0.0));
        }
        assert_eq!(
            recaptures, 0,
            "empty streamed sky cannot invalidate the same loaded roof twenty times"
        );
        let roof = SectionPos::new(-2, 5, -3);
        let mut data = terra_voxel::BlockSection::empty(roof).unwrap();
        data.set_block([7, 3, 7], 3).unwrap();
        assert!(exposure.section_affects(roof, Some(&data), 0.0));
        let removed = Exposure {
            roofs: [83; 256],
            ..exposure
        };
        assert!(
            removed.section_affects(roof, None, 0.0),
            "unloading an actual ceiling must recapture lower exposure"
        );
        let canopy = SectionPos::new(-2, 0, -3);
        assert!(
            removed.section_affects(canopy, None, 0.0),
            "snow sees a different roof below the crown"
        );
    }
    #[test]
    fn snow_ignores_natural_crowns_but_not_player_roofs() {
        use terra_world::generation::TreeAppearance;
        for kind in [
            TreeAppearance::Oak,
            TreeAppearance::Autumn,
            TreeAppearance::Birch,
            TreeAppearance::Spruce,
            TreeAppearance::Palm,
            TreeAppearance::Mushroom,
        ] {
            for material in 7..=9 {
                assert!(snow_transparent(material, Some(kind), false));
                assert!(!snow_transparent(material, Some(kind), true));
            }
            assert!(snow_transparent(6, Some(kind), false));
            assert!(!snow_transparent(6, Some(kind), true));
            assert!(!snow_transparent(3, Some(kind), false));
        }
        assert!(!snow_transparent(7, None, false));
    }

    #[test]
    fn snow_reaches_ground_under_shed_crowns_but_respects_roofs_and_water() {
        let mut exposure = Exposure {
            natural: None,
            roofs: [20; 256],
            snow_roofs: [8; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        assert_eq!(exposure.payload([0, 8, 0], 0), 0.0);
        assert!(exposure.snow_exposed([0, 8, 0], 0));
        assert!(!exposure.snow_exposed([0, 7, 0], 0));
        exposure.snow_roofs[0] = 20;
        assert!(!exposure.snow_exposed([0, 8, 0], 0));
        exposure.snow_roofs[0] = 8;
        exposure.water[0] = 10.0;
        assert!(!exposure.snow_exposed([0, 8, 0], 0));
        assert_eq!(exposure.payload([0, 8, 0], 0), -1.0);
    }

    #[test]
    fn water_occupancy_uses_the_cell_immediately_below_integer_or_fractional_planes() {
        for (level, expected) in [
            (8.0, 7),
            (8.25, 8),
            (8.999, 8),
            (0.0, -1),
            (-2.0, -3),
            (-2.3, -3),
        ] {
            assert_eq!(water_cell_y(level), expected, "water plane {level}");
        }
    }
    #[test]
    fn exposure_does_not_wet_sheltered_faces_and_coast_is_subblock_payload() {
        let mut exposure = Exposure {
            natural: None,
            roofs: [20; 256],
            snow_roofs: [20; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        exposure.coast[0] = 0.6;
        assert_eq!(exposure.payload([0, 4, 0], 16), 1.6);
        assert_eq!(exposure.payload([0, 3, 0], 16), 0.0);
        assert_eq!(exposure.payload([1, 4, 0], 16), 1.0);
    }
    #[test]
    fn submerged_roofs_override_rain_and_saved_wetness_at_negative_coordinates() {
        let mut exposure = Exposure {
            natural: None,
            roofs: [-20; 256],
            snow_roofs: [-20; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        exposure.water[0] = -18.5;
        assert_eq!(exposure.payload([0, 12, 0], -32), -1.0);
        assert_eq!(exposure.payload([0, 11, 0], -32), -1.0);
        assert_eq!(exposure.payload([0, 13, 0], -32), 1.0);
        assert_eq!(exposure.payload([1, 12, 0], -32), 1.0);
        // A dry bank exactly at water height is not an underwater floor.
        exposure.water[0] = -19.0;
        assert_eq!(exposure.payload([0, 12, 0], -32), 1.0);
        exposure.roofs[0] = 60;
        exposure.water[0] = -18.5;
        assert_eq!(
            exposure.payload([0, 12, 0], -32),
            -1.0,
            "overhanging canopy must not expose a submerged bed to rain"
        );
    }
    #[test]
    fn moving_fluids_refresh_the_rain_mask_only_when_submersion_changes() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            base_height: 100.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            biomes: false,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let exposure = Exposure {
            natural: None,
            roofs: [-20; 256],
            snow_roofs: [-20; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        let mut state = RoofWaterState::captured(&exposure, 0, 0);
        let cell = [-1, -19, -1];
        let owner = SectionPos::new(-1, -2, -1);
        let mut fluid = terra_world::water::FluidCell {
            amount: 4,
            source: false,
            flow: [1, 0],
        };
        journal.set_fluid(cell, fluid).unwrap();
        state.refresh(
            &chunks,
            [-1, -1],
            &terrain,
            &journal,
            journal.fluid_epoch(owner),
        );
        assert_eq!(state.epoch, 1);
        assert_eq!(state.mask[255], -18);
        fluid.amount = 8;
        fluid.flow = [-1, 0];
        journal.set_fluid(cell, fluid).unwrap();
        state.refresh(
            &chunks,
            [-1, -1],
            &terrain,
            &journal,
            journal.fluid_epoch(owner),
        );
        assert_eq!(
            state.epoch, 1,
            "changing water amount/flow must not rebuild terrain"
        );
        journal.set_fluid(cell, Default::default()).unwrap();
        state.refresh(
            &chunks,
            [-1, -1],
            &terrain,
            &journal,
            journal.fluid_epoch(owner),
        );
        assert_eq!(state.epoch, 2);
        assert_eq!(state.mask, [i16::MIN; 256]);
    }
    #[test]
    fn wash_distances_match_at_adjacent_chunk_grid_corners() {
        let mut ocean = vec![false; 48 * 48];
        for z in 0..20 {
            for x in 0..48 {
                ocean[x + z * 48] = true;
            }
        }
        for z in 20..23 {
            assert_eq!(
                coast_reach(&ocean, 48, 32.0, z as f32),
                coast_reach(&ocean, 48, 16.0, z as f32)
            );
        }
        assert!(coast_reach(&ocean, 48, 16.0, 20.0) > coast_reach(&ocean, 48, 16.0, 20.5));
    }
    #[test]
    fn flood_neighbors_never_wrap_row_edges() {
        assert_eq!(neighbors4(0, 3), [None, Some(1), None, Some(3)]);
        assert_eq!(neighbors4(8, 3), [Some(7), None, Some(5), None]);
    }

    #[test]
    fn same_height_lake_is_not_an_ocean_without_a_connected_water_path() {
        let mut water = [false; 25];
        for i in [0, 1, 6, 11, 18, 19, 24] {
            water[i] = true;
        }
        let mut connected = [false; 25];
        connected[0] = true;
        connect_ocean(&water, &mut connected, VecDeque::from([0]), 5);
        assert!(connected[11]);
        assert!(!connected[18] && !connected[24]);
    }
}
