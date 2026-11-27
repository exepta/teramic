//! Revision-four settlements reuse the same plots, foundations and bounded
//! street router. Homes are separate from their required civic buildings.
use super::*;
use crate::generation::structures::VillageSize;

fn roles(seed: u32) -> Vec<BuildingRole> {
    use BuildingRole::*;
    let size = VillageSize::from_seed(seed);
    let mut roles = vec![Church, Medic];
    if size != VillageSize::Small {
        if (seed >> 14) & 1 != 0 {
            roles.push(Medic);
        }
        roles.push(Smith);
    }
    if size == VillageSize::Large {
        roles.push(Police);
    }
    roles.extend(std::iter::repeat_n(House, size.houses(seed)));
    roles
}

fn envelope(plot: &Plot) -> [[i64; 2]; 2] {
    let corners = [
        [-1, -3],
        [plot.width + 1, -3],
        [-1, plot.depth + 1],
        [plot.width + 1, plot.depth + 1],
    ];
    corners.into_iter().map(|[x, z]| plot.point(x, 0, z)).fold(
        [[i64::MAX; 2], [i64::MIN; 2]],
        |mut b, p| {
            for (i, a) in [0, 2].into_iter().enumerate() {
                b[0][i] = b[0][i].min(p[a]);
                b[1][i] = b[1][i].max(p[a]);
            }
            b
        },
    )
}
fn overlaps(a: [[i64; 2]; 2], b: [[i64; 2]; 2]) -> bool {
    (0..2).all(|i| a[0][i] <= b[1][i] + 1 && a[1][i] + 1 >= b[0][i])
}

pub(super) fn build(builder: &mut Builder<'_>, x: i64, z: i64) -> Option<()> {
    let seed = builder.seed;
    let rotation = ((seed >> 24) % 4) as u8;
    let layout = Layout::from_seed(seed);
    let mut candidates = Vec::with_capacity(44);
    for row in 0..6 {
        for col in 0..8 {
            let mut u: i64 = col * 18 - 63;
            let mut v: i64 = row * 18 - 45;
            if u.abs() < 18 && v.abs() < 18 {
                continue;
            }
            let salt = super::super::super::super::seed::layer_seed(
                u64::from(seed),
                2600 + (row * 8 + col) as u64,
            );
            match layout {
                Layout::Linear => u += (v / 20) * 2,
                Layout::Courtyard => v += (u.abs() / 18 - 2) * (if v >= 0 { 1 } else { -1 }),
                Layout::Plaza => {
                    u += i64::from(salt % 3) - 1;
                    v += i64::from((salt >> 4) % 3) - 1;
                }
            }
            candidates.push(([u, v], salt));
        }
    }
    // Different occupied plots, bend offsets and overall rotation avoid a fixed
    // six-building template. Civic buildings favor central, reachable streets.
    candidates
        .sort_unstable_by_key(|([u, v], salt)| (u.abs() + v.abs() + i64::from(salt % 17), *salt));
    let farm_offset = rotate([-63, -45], rotation);
    let farm = [x + farm_offset[0] - 6, z + farm_offset[1] - 4];
    let farm_y = if builder.allows_field(farm[0], farm[1], 12, 8) {
        let floor = builder.fit(farm[0], farm[1], 12, 8);
        // An alpine village can occupy a terrace even when its outlying field
        // falls on a steep or unsupported slope. Every occupied plot must fit.
        if floor.is_none()
            && !(builder.terrain.highland_overhaul()
                && builder.summary.style == VillageStyle::Highland)
        {
            return None;
        }
        floor
    } else {
        None
    };
    let farm_bounds = farm_y.map(|_| [[farm[0] - 1, farm[1] - 3], [farm[0] + 13, farm[1] + 9]]);
    let mut plots = Vec::new();
    let mut occupied: Vec<_> = farm_bounds.into_iter().collect();
    let mut foundations = std::collections::HashMap::new();
    for (i, role) in roles(seed).into_iter().enumerate() {
        let mut chosen = None;
        for (&[u, v], variant) in candidates.iter().map(|(p, v)| (p, *v)) {
            let offset = rotate([u, v], rotation);
            let facing = if u.abs() > v.abs() {
                if u > 0 { 1 } else { 3 }
            } else if v > 0 {
                0
            } else {
                2
            };
            let (width, depth, height) = match role {
                BuildingRole::Church => (11, 15, 6),
                BuildingRole::Medic => (9, 9, 5),
                BuildingRole::Smith => (11, 9, 5),
                BuildingRole::Police => (11, 11, 5),
                _ => (
                    7 + i64::from(variant % 3),
                    7 + i64::from((variant >> 8) % 3),
                    4 + i64::from((variant >> 10) % 2),
                ),
            };
            let mut plot = Plot {
                center: [x + offset[0], z + offset[1]],
                width,
                depth,
                height,
                facing: (facing + rotation) % 4,
                floor: 0,
                role,
                variant: variant ^ i as u32,
            };
            let bounds = envelope(&plot);
            if occupied.iter().any(|&old| overlaps(bounds, old)) {
                continue;
            }
            let [lo, hi] = plot.bounds();
            let Some(floor) = *foundations
                .entry([lo, hi])
                .or_insert_with(|| builder.fit(lo[0], lo[1], hi[0] - lo[0], hi[1] - lo[1]))
            else {
                continue;
            };
            plot.floor = floor;
            chosen = Some((plot, bounds));
            break;
        }
        let (plot, bounds) = chosen?;
        occupied.push(bounds);
        plots.push(plot);
    }
    let mut roads = paths::Routes::with_radius([x, z], &plots, farm_bounds, 82);
    for plot in &plots {
        roads.connect(builder, plot.portal())?;
    }
    if let Some(farm_y) = farm_y {
        roads.connect(builder, [farm[0] + 6, farm_y, farm[1] - 2])?;
    }
    for plot in &plots {
        buildings::build(builder, plot);
    }
    if farm_y.is_some() {
        builder.farm(farm[0], farm[1])?;
    }
    let old_layout = builder.terrain.config().ecology_revision < 14;
    let [wx, wz] = if old_layout { farm } else { [x, z] };
    if super::super::plains::active(builder, wx, wz) {
        let reserved = |bounds: [[i64; 2]; 2]| {
            occupied.iter().any(|&plot| overlaps(bounds, plot))
                || roads.walk.iter().any(|p| {
                    (0..2).all(|axis| (bounds[0][axis]..=bounds[1][axis]).contains(&p[axis]))
                })
        };
        if old_layout {
            super::super::plains::mill_by_field(builder, farm, reserved);
        }
        super::super::plains::well(builder, [x, z], reserved);
    }
    if let Some(y) = farm_y {
        super::farm_entrance(builder, farm, y);
    }
    // The reserved central clearing stays low and navigable instead of adding
    // another set of torch-lit structures over residential plots.
    let y = builder.ground(x, z);
    builder.fill([x - 2, y - 1, z - 2], [x + 2, y - 1, z + 2], 43);
    for dx in [-2, 2] {
        for dz in -2..=2 {
            if !roads.walk.contains(&[x + dx, z + dz]) {
                builder.state([x + dx, y, z + dz], 85, 0);
            }
        }
    }
    for dz in [-2, 2] {
        for dx in -1..=1 {
            if !roads.walk.contains(&[x + dx, z + dz]) {
                builder.state([x + dx, y, z + dz], 85, 0);
            }
        }
    }
    if builder.terrain.config().structures_revision >= 5 {
        super::lighting::build(builder, &roads, &occupied);
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{WorldConfig, WorldSeed};

    fn terrain(revision: u8) -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            structures_revision: revision,
            landscape_revision: 0,
            geology_revision: 0,
            ocean_revision: 0,
            base_height: 16.,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            terrace_strength: 0.,
            caves: false,
            trees: false,
            water: false,
            rivers: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap()
    }
    fn site(terrain: &TerraGenerator, seed: u32, style: VillageStyle) -> Site {
        let mut builder = Builder {
            terrain,
            summary: SiteSummary {
                id: u64::from(seed),
                kind: StructureKind::Village,
                style,
                center: [-192, 16, -576],
                bounds: [[0; 3]; 2],
                buildings: Vec::new(),
                containers: Vec::new(),
                grave: None,
            },
            patches: BTreeMap::new(),
            objects: BTreeMap::new(),
            writes: 0,
            failed: false,
            seed,
        };
        build(&mut builder, -192, -576).expect("flat bounded settlement must fit its quota");
        builder
            .finish()
            .expect("new settlements retain existing hard section/cell limits")
    }

    #[test]
    fn every_size_has_exact_civic_quotas_plus_residential_houses_and_distinct_seeded_streets() {
        use BuildingRole::*;
        let generator = terrain(4);
        let mut layouts = std::collections::BTreeSet::new();
        for size_index in 0..3 {
            for rotation in 0..4 {
                let size_bits = (size_index + 3 - (rotation * 16) % 3) % 3;
                let seed = (size_bits << 20) | (rotation << 24) | ((rotation % 2) << 14) | 1;
                let size = VillageSize::from_seed(seed);
                let style = [
                    VillageStyle::Temperate,
                    VillageStyle::Desert,
                    VillageStyle::Highland,
                ][size_index as usize];
                let site = site(&generator, seed, style);
                let count = |role| {
                    site.summary
                        .buildings
                        .iter()
                        .filter(|b| b.role == role)
                        .count()
                };
                assert_eq!(count(House), size.houses(seed));
                assert_eq!(count(Church), 1);
                assert_eq!(count(Smith), usize::from(size != VillageSize::Small));
                assert_eq!(count(Police), usize::from(size == VillageSize::Large));
                assert_eq!(
                    count(Medic),
                    1 + usize::from(size != VillageSize::Small && (seed >> 14) & 1 != 0)
                );
                assert_eq!(count(Farm), 1);
                let settlement = site.settlement.as_ref().unwrap();
                assert_eq!(settlement.homes.len(), count(House));
                assert!(settlement.homes.iter().all(|home| home.beds.len() == 2));
                assert!(settlement.residents.len() >= 2 * count(House));
                assert!(settlement.residents.len() < 64);
                assert!(site.sections.len() <= MAX_SITE_SECTIONS);
                assert!(
                    site.sections.values().map(|s| s.cells.len()).sum::<usize>() <= MAX_SITE_CELLS
                );
                layouts.insert(
                    site.summary
                        .buildings
                        .iter()
                        .map(|b| (b.role as u8, b.entrance))
                        .collect::<Vec<_>>(),
                );
                for axis in [0, 2] {
                    let region = site.summary.center[axis].div_euclid(REGION_SIZE) * REGION_SIZE;
                    assert!(site.summary.bounds[0][axis] >= region + 96);
                    assert!(site.summary.bounds[1][axis] <= region + 288);
                }
                for (i, resident) in settlement.residents.iter().enumerate() {
                    assert_eq!(resident.slot, i as u16);
                    assert!(terra_core::creatures::villagers::is_civilian(resident.role));
                    let object = site
                        .sections
                        .values()
                        .flat_map(|s| &s.objects)
                        .find(|o| o.position == resident.bed)
                        .unwrap();
                    let state = object.state.unwrap();
                    assert!(!state.secondary);
                    assert_eq!(state.facing, resident.bed_facing);
                    assert!(state.valid_for(object.position, object.material));
                    let door = site
                        .sections
                        .values()
                        .flat_map(|s| &s.objects)
                        .find(|o| o.position == resident.door)
                        .unwrap();
                    assert!((65..=67).contains(&door.material));
                    assert!(!door.state.unwrap().secondary);
                }
            }
        }
        assert_eq!(layouts.len(), 12);
    }

    #[test]
    fn street_lights_are_bounded_supported_and_do_not_retrofit_old_layouts() {
        let old = site(&terrain(4), 1, VillageStyle::Temperate);
        let generator = terrain(5);
        let new = site(&generator, 1, VillageStyle::Temperate);
        let repeated = site(&generator, 1, VillageStyle::Temperate);
        assert_eq!(old.settlement, new.settlement);
        let cells = |site: &Site| -> BTreeMap<[i64; 3], u16> {
            site.sections
                .iter()
                .flat_map(|(owner, patch)| {
                    let origin = owner.origin().unwrap();
                    patch.cells.iter().map(move |&(i, m)| {
                        let i = i64::from(i);
                        (
                            [
                                origin[0] + i % 16,
                                origin[1] + i / 16 % 16,
                                origin[2] + i / 256,
                            ],
                            m,
                        )
                    })
                })
                .collect()
        };
        let before = cells(&old);
        let after = cells(&new);
        assert_eq!(after, cells(&repeated));
        let added: Vec<_> = after
            .iter()
            .filter_map(|(p, &m)| (m == 40 && before.get(p) != Some(&m)).then_some(*p))
            .collect();
        assert!(
            (6..=20).contains(&added.len()),
            "street lamps: {}",
            added.len()
        );
        for lamp in &added {
            for dy in 1..=3 {
                let p = [lamp[0], lamp[1] - dy, lamp[2]];
                assert_eq!(after.get(&p), Some(&90));
                assert!(before.get(&p).is_none_or(|m| *m == 0));
            }
            assert_eq!(
                generator
                    .surface_height(lamp[0] as f64 + 0.5, lamp[2] as f64 + 0.5)
                    .ceil() as i64
                    + 3,
                lamp[1]
            );
        }
    }

    #[test]
    fn residents_are_cached_order_independent_and_old_settlement_voxels_are_not_retrofitted() {
        let generator = terrain(4);
        let region = [-1, -2];
        let first = super::super::plan(&generator, region, 1).unwrap();
        let repeat = super::super::plan(&generator, region, 1).unwrap();
        assert_eq!(first.summary, repeat.summary);
        assert_eq!(first.settlement, repeat.settlement);
        for (p, patch) in &first.sections {
            assert_eq!(patch.cells, repeat.sections[p].cells);
            assert_eq!(patch.objects, repeat.sections[p].objects);
        }
        let actual = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| [x, z]))
            .find_map(|region| {
                generator
                    .planned_site(region)
                    .filter(|s| s.summary.kind == StructureKind::Village)
            })
            .unwrap();
        let prepared = crate::SectionGenerator::new(generator.clone())
            .prepare_column(
                [
                    actual.summary.center[0].div_euclid(16),
                    actual.summary.center[2].div_euclid(16),
                ],
                false,
            )
            .unwrap();
        assert!(
            prepared
                .village_settlements()
                .any(|s| s.id == actual.summary.id)
        );
        let cached_regions = generator.structures.0.lock().unwrap().slots.len();
        for settlement in prepared.village_settlements() {
            assert!(std::sync::Arc::ptr_eq(
                settlement,
                prepared
                    .village_settlements()
                    .find(|s| s.id == settlement.id)
                    .unwrap()
            ));
            assert_eq!(
                prepared
                    .village_residents()
                    .filter(|r| r.village_id == settlement.id)
                    .count(),
                settlement.residents.len()
            );
        }
        assert_eq!(
            generator.structures.0.lock().unwrap().slots.len(),
            cached_regions
        );
        for revision in 1..=3 {
            let old = terrain(revision);
            let old = super::super::plan(&old, region, 1).unwrap();
            assert_eq!(
                old.summary
                    .buildings
                    .iter()
                    .filter(|b| b.role == BuildingRole::House)
                    .count(),
                2
            );
            assert!(
                !old.summary
                    .buildings
                    .iter()
                    .any(|b| b.role == BuildingRole::Police)
            );
            let old_beds = old
                .sections
                .values()
                .flat_map(|s| &s.objects)
                .filter(|o| matches!(o.material, 71..=73) && !o.state.unwrap().secondary)
                .count();
            assert_eq!(old_beds, 3, "saved layouts keep their original three beds");
        }
    }

    #[test]
    fn populated_villages_fit_real_default_terrain_for_three_seeds() {
        for (seed, landscape_revision) in [42042, 1337, 99173]
            .into_iter()
            .flat_map(|seed| [2, 3].map(|revision| (seed, revision)))
        {
            let generator = TerraGenerator::new(WorldConfig {
                seed: WorldSeed(seed),
                structures_revision: 4,
                // Keep the original river/ecology contracts of these bounded
                // historical candidates, before optional Spirit/Blood terrain.
                ecology_revision: 4,
                river_terrain_revision: 4,
                spirit_blossom_revision: 0,
                blood_forest_revision: 0,
                landscape_revision,
                rivers: landscape_revision >= 3,
                ..Default::default()
            })
            .unwrap();
            let mut regions = vec![[2, -2]];
            for radius in 0_i64..=3 {
                for z in -radius..=radius {
                    for x in -radius..=radius {
                        if (radius == 0 || x.abs() == radius || z.abs() == radius)
                            && [x, z] != [2, -2]
                        {
                            regions.push([x, z]);
                        }
                    }
                }
            }
            let mut attempted = 0;
            let village = regions.into_iter().find_map(|region| {
                let regional = crate::generation::seed::layer_seed(
                    seed ^ (region[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                        ^ (region[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
                    0x573731,
                );
                // Only actual settlement candidates: no unrelated mine/ruin
                // plans, and a hard cap on terrain-heavy construction attempts.
                if regional % 9 > 3 || regional % 10 == 9 || attempted >= 24 {
                    return None;
                }
                attempted += 1;
                generator.village_settlement(region).map(|v| (region, v))
            });
            let (region, village) = village.unwrap_or_else(|| {
                panic!("seed={seed}, landscape={landscape_revision}: no revision-four village in {attempted} bounded candidates")
            });
            let count = |role| village.services.iter().filter(|b| b.role == role).count();
            let range = match village.size {
                VillageSize::Small => 6..=8,
                VillageSize::Medium => 10..=16,
                VillageSize::Large => 18..=24,
            };
            assert!(range.contains(&village.homes.len()));
            assert!(village.homes.iter().all(|h| h.beds.len() == 2));
            assert_eq!(count(BuildingRole::Church), 1);
            assert_eq!(
                count(BuildingRole::Smith),
                usize::from(village.size != VillageSize::Small)
            );
            assert_eq!(
                count(BuildingRole::Police),
                usize::from(village.size == VillageSize::Large)
            );
            assert!((1..=2).contains(&count(BuildingRole::Medic)));
            assert!(village.residents.len() >= village.homes.len() * 2);
            assert_eq!(
                generator.village_settlement(region).as_deref(),
                Some(village.as_ref())
            );
            eprintln!(
                "populated village seed={seed} region={region:?} size={:?} houses={} beds={} attempts={attempted}",
                village.size,
                village.homes.len(),
                village.residents.len()
            );
        }
    }
}
