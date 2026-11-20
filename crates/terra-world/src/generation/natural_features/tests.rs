use super::*;
use crate::{SectionGenerator, TerrainGenerator};
use terra_core::{Config, WorldConfig};
use terra_voxel::global_to_section;

fn terrain() -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        ecology_revision: 5,
        landscape_revision: 1,
        structures_revision: 0,
        trees: false,
        caves: false,
        water: false,
        rivers: false,
        base_height: 48.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        sea_level: -64.0,
        snow_height: 300.0,
        ..Default::default()
    })
    .unwrap()
}
fn habitat(terrain: &TerraGenerator, tile: Tile) -> Habitat<'_> {
    let mut base = terrain.surface_column(0.5, 0.5);
    base.height = 48.0;
    base.slope = 0.0;
    base.water_level = None;
    base.cave_cavity = None;
    base.river_cavity = None;
    base.surface_material = MaterialId::BloodGrass;
    base.biome.weights = super::super::biomes::BiomeWeights {
        blood_forest: 1.0,
        ocean: 0.0,
        plains: 0.0,
        forest: 0.0,
        desert: 0.0,
        highland: 0.0,
        mushroom_park: 0.0,
        cliffted_desert: 0.0,
        cherry_forest: 0.0,
        spirit_blossom: 0.0,
    };
    let mut columns = HashMap::new();
    for z in tile[1] * 32..tile[1] * 32 + 32 {
        for x in tile[0] * 32..tile[0] * 32 + 32 {
            columns.insert([x, z], base);
        }
    }
    Habitat {
        terrain,
        trees: Vec::new(),
        sites: Vec::new(),
        columns,
    }
}

#[test]
fn groups_leave_clearings_and_large_props_keep_twelve_metre_spacing_across_negative_tiles() {
    let terrain = terrain();
    let mut cells = BTreeMap::new();
    let mut large = Vec::new();
    let mut empty = 0;
    for z in -4..4 {
        for x in -4..4 {
            let mut context = habitat(&terrain, [x, z]);
            let mut local = BTreeMap::new();
            flora::build(&mut context, [x, z], &mut local);
            empty += usize::from(local.is_empty());
            for (&cell, &material) in &local {
                if matches!(material, 419..=421) {
                    large.push(cell);
                }
            }
            assert!(
                local.len() < 70,
                "one tile cannot become a decoration carpet"
            );
            cells.extend(local);
        }
    }
    assert!(
        cells.len() > 100 && cells.len() < 64 * 1024 / 15,
        "{} decorations",
        cells.len()
    );
    assert!(empty > 0, "real empty clearings must remain");
    assert!(large.len() > 3, "large props must actually occur");
    for (index, a) in large.iter().enumerate() {
        for b in &large[index + 1..] {
            assert!(
                (a[0] - b[0]).pow(2) + (a[2] - b[2]).pow(2) >= 144,
                "large props overlap {a:?}/{b:?}"
            );
        }
    }
    // Most small props have another member of their botanical group nearby.
    let clustered = cells
        .iter()
        .filter(|(p, m)| {
            !matches!(**m, 419..=421)
                && cells.iter().any(|(q, n)| {
                    *p != q && *m == n && (p[0] - q[0]).pow(2) + (p[2] - q[2]).pow(2) <= 8
                })
        })
        .count();
    assert!(clustered > cells.len() / 2);
}

#[test]
fn habitats_reject_water_portals_slopes_structures_and_tree_feet() {
    use super::super::structures::{SiteSummary, StructureKind, VillageStyle};
    let terrain = terrain();
    for mode in 0..4 {
        for tile in [[0, 0], [-1, -1], [1, -2]] {
            let mut context = habitat(&terrain, tile);
            for column in context.columns.values_mut() {
                match mode {
                    0 => column.water_level = Some(50.0),
                    1 => column.cave_cavity = Some([46.0, 53.0]),
                    2 => column.slope = 2.5,
                    _ => {}
                }
            }
            if mode == 3 {
                context.sites.push(Arc::new(Site {
                    summary: SiteSummary {
                        id: 1,
                        kind: StructureKind::Village,
                        style: VillageStyle::Temperate,
                        center: [tile[0] * 32, 48, tile[1] * 32],
                        bounds: [
                            [tile[0] * 32, 40, tile[1] * 32],
                            [tile[0] * 32 + 31, 70, tile[1] * 32 + 31],
                        ],
                        buildings: Vec::new(),
                        containers: Vec::new(),
                        grave: None,
                    },
                    sections: Default::default(),
                    settlement: None,
                }));
            }
            let mut cells = BTreeMap::new();
            assert!(rocks::build(&mut context, tile, &mut cells).is_none());
            flora::build(&mut context, tile, &mut cells);
            assert!(cells.is_empty(), "habitat exclusion failed for mode{mode}");
        }
    }
    let mut context = habitat(&terrain, [0, 0]);
    context.trees.push(super::super::vegetation::Tree::new(
        [16, 48, 16],
        42,
        [0.0; 4],
    ));
    let mut cells = BTreeMap::new();
    flora::build(&mut context, [0, 0], &mut cells);
    assert!(
        cells
            .keys()
            .all(|p| (p[0] - 16).pow(2) + (p[2] - 16).pow(2) >= 9 && !context.tree_occupied(*p))
    );
}

#[test]
fn rock_voxels_are_embedded_irregular_and_match_streamed_boundaries_edits_and_reload() {
    let terrain = terrain();
    let (tile, formation) = (-8..0)
        .flat_map(|z| (-8..0).map(move |x| [x, z]))
        .find_map(|tile| {
            terrain
                .surface_rock_formations(tile)
                .into_iter()
                .next()
                .map(|r| (tile, r))
        })
        .expect("real rock group");
    assert!(formation.voxel_count > 24);
    let volume = (0..3)
        .map(|axis| (formation.bounds[1][axis] - formation.bounds[0][axis] + 1) as usize)
        .product::<usize>();
    assert!(
        formation.voxel_count * 5 < volume * 4,
        "formation must not be a cuboid"
    );
    assert!(formation.bounds[1][1] > formation.bounds[0][1] + 1);
    let generator = SectionGenerator::new(terrain.clone());
    let patch = terrain.natural_patch(tile);
    let restored = TerraGenerator::new(terrain.config().clone()).unwrap();
    assert_eq!(*patch, *restored.natural_patch(tile));
    let mut owners = std::collections::BTreeSet::new();
    for &(cell, material) in &patch.cells {
        assert_eq!(generator.block_at(cell).unwrap(), material);
        assert_eq!(
            terrain.sample(cell.map(|v| v as f64 + 0.5)).material,
            material
        );
        owners.insert(global_to_section(cell).0);
    }
    assert!(
        owners
            .iter()
            .map(|p| [p.x, p.z])
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1,
        "exercise a negative chunk edge"
    );
    for owner in owners.into_iter().rev() {
        let a = generator
            .build(owner, 0, &Default::default(), true)
            .unwrap();
        let b = generator
            .build(owner, 0, &Default::default(), false)
            .unwrap();
        assert_eq!(a.data, b.data);
        let start = owner.origin().unwrap();
        for z in 0..16 {
            for y in 0..16 {
                for x in 0..16 {
                    let cell = [
                        start[0] + x as i64,
                        start[1] + y as i64,
                        start[2] + z as i64,
                    ];
                    assert_eq!(
                        a.data.block([x, y, z]).unwrap(),
                        generator.block_at(cell).unwrap()
                    );
                }
            }
        }
    }
    let cell = patch.cells[0].0;
    let (owner, local) = global_to_section(cell);
    let changed = generator
        .build(
            owner,
            1,
            &std::collections::HashMap::from([(cell, 0)]),
            false,
        )
        .unwrap();
    assert_eq!(changed.data.block(local).unwrap(), 0);
    let top_ground = terrain.surface_height(
        formation.center[0] as f64 + 0.5,
        formation.center[2] as f64 + 0.5,
    );
    let mut old = terrain.config().clone();
    old.ecology_revision = 4;
    let old = TerraGenerator::new(old).unwrap();
    assert_eq!(
        old.surface_height(
            formation.center[0] as f64 + 0.5,
            formation.center[2] as f64 + 0.5
        ),
        top_ground
    );
    assert!(old.surface_rock_formations(tile).is_empty());
    assert_eq!(
        WorldConfig::from_toml("seed=42042")
            .unwrap()
            .ecology_revision,
        0
    );
    assert!(!terrain.config().same_terrain(old.config()));
    for revision in 0..=4 {
        let mut config = terrain.config().clone();
        config.ecology_revision = revision;
        assert!(
            TerraGenerator::new(config)
                .unwrap()
                .natural_patch([-1, -1])
                .cells
                .is_empty()
        );
    }
}
