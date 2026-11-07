use super::*;
use crate::{SectionGenerator, TerrainGenerator};
use terra_core::WorldConfig;
use terra_voxel::{MaterialId, SectionPos};

fn terrain() -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        water: false,
        trees: false,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn mineral_wall_cover_is_clustered_restrained_and_not_resource_ore() {
    let t = terrain();
    let mut counts = [0; 3];
    let mut neighbor_matches = 0;
    let mut previous = None;
    for x in -256..256 {
        let mut c = t.surface_column(x as f64 + 0.5, -31.5);
        c.cave = Default::default();
        c.cave_biome.surface = 40.0;
        c.cave_biome.sample = CaveBiomeSample::from_weights([0.0, 1.0, 0.0, 0.0], 1, 100.0);
        let material = t.cave_ecology_material([x as f64 + 0.5, -32.5, -31.5], c, -0.2);
        assert_ne!(material, Some(MaterialId::SaltOre));
        let index = match material {
            Some(MaterialId::SaltStone) => 1,
            Some(MaterialId::PinkSaltStone) => 2,
            _ => 0,
        };
        counts[index] += 1;
        neighbor_matches += usize::from(previous == Some(index));
        previous = Some(index);
    }
    assert!(counts[0] > 150, "quiet rock zones {counts:?}");
    assert!(
        counts[1] > 10 && counts[2] > 10,
        "both mineral strata {counts:?}"
    );
    assert!(
        neighbor_matches > 400,
        "coherent patches instead of checkerboard"
    );
}

#[test]
fn salt_cover_can_be_disabled_without_disabling_the_biome_or_changing_density() {
    let mut config = terra_core::WorldConfig::default();
    config.cave_biomes.salt_surfaces = [1.0, 0.0, 0.0, 0.0];
    let t = TerraGenerator::new(config).unwrap();
    for x in -64..64 {
        let mut c = t.surface_column(x as f64 + 0.5, -31.5);
        c.cave = Default::default();
        c.cave_biome.surface = 40.0;
        c.cave_biome.sample = CaveBiomeSample::from_weights([0.0, 1.0, 0.0, 0.0], 1, 100.0);
        assert!(!matches!(
            t.cave_ecology_material([x as f64 + 0.5, -32.5, -31.5], c, -0.2),
            Some(MaterialId::SaltStone | MaterialId::PinkSaltStone | MaterialId::SaltOre)
        ));
    }
}

#[test]
fn lush_connector_grass_matches_direct_samples_and_prop_prefilter_is_mesh_independent() {
    lush_connector_grass_consistency(1);
    lush_connector_grass_consistency(2);
}

fn lush_connector_grass_consistency(landscape_revision: u8) {
    let mut config = WorldConfig {
        landscape_revision,
        ..Default::default()
    };
    config.cave_biomes.distribution = terra_core::config::CaveDistribution {
        normal_weight: 0.0,
        salt_weight: 0.0,
        lush_weight: 1.0,
        cascade_weight: 0.0,
    };
    let t = TerraGenerator::new(config).unwrap();
    let generator = SectionGenerator::new(t.clone());
    let mut found = None;
    'search: for z in (-64..0).step_by(2) {
        for x in (-64..0).step_by(2) {
            let c = t.surface_column(x as f64 + 0.5, z as f64 + 0.5);
            // Lush suitability follows depth, not an absolute sea-level band.
            // Retain the historical revision-one floor search, and search the
            // corresponding shallow underground band in the new landscape.
            let range = if landscape_revision == 1 {
                -64..0
            } else {
                let top = (c.height - 12.0).floor() as i64;
                (top - 80).max(-118)..top.min(320)
            };
            for y in range {
                let p = [x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5];
                if c.height - p[1] > 8.0
                    && !c.cave.active()
                    && t.sample_with_column(p, c).material == MaterialId::Grass as u16
                {
                    found = Some([x, y, z]);
                    break 'search;
                }
            }
        }
    }
    let p = found.unwrap_or_else(|| {
        panic!("actual lush connector floor for landscape revision {landscape_revision}")
    });
    assert_eq!(
        t.sample(p.map(|v| v as f64 + 0.5)).material,
        MaterialId::Grass as u16
    );
    assert_eq!(generator.block_at(p).unwrap(), MaterialId::Grass as u16);
    let owner = terra_voxel::global_to_section(p).0;
    for offset in [[0, 0, 0], [0, 1, 0], [1, 0, 0]] {
        let section = owner.offset(offset).unwrap();
        let plain = generator
            .build(section, 0, &Default::default(), false)
            .unwrap();
        let meshed = generator
            .build(section, 0, &Default::default(), true)
            .unwrap();
        assert_eq!(plain.data, meshed.data);
        assert_eq!(
            plain.props, meshed.props,
            "padding must not change prop identities"
        );
    }
}

#[test]
fn distribution_is_seeded_order_independent_negative_and_seamless() {
    let a = terrain();
    let b = terrain();
    let points: Vec<_> = (-12..=12)
        .flat_map(|z| (-12..=12).map(move |x| [f64::from(x) * 32.0, -30.5, f64::from(z) * 32.0]))
        .collect();
    let samples: Vec<_> = points.iter().map(|&p| a.cave_biome_at(p)).collect();
    for (&p, &sample) in points.iter().zip(&samples).rev() {
        assert_eq!(sample, b.cave_biome_at(p));
        assert!((sample.weights.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        for axis in [0, 2] {
            let mut left = p;
            let mut right = p;
            left[axis] -= 1e-5;
            right[axis] += 1e-5;
            let l = a.cave_biome_at(left);
            let r = a.cave_biome_at(right);
            for i in 0..4 {
                assert!(
                    (l.weights[i] - r.weights[i]).abs() < 0.001,
                    "boundary {p:?}"
                );
            }
        }
    }
    a.cave_biomes.0.lock().unwrap().clear();
    for (&p, &s) in points.iter().zip(&samples).step_by(11) {
        assert_eq!(s, a.cave_biome_at(p));
    }
    let changed = TerraGenerator::new(WorldConfig {
        seed: terra_core::WorldSeed(19),
        water: false,
        ..Default::default()
    })
    .unwrap();
    assert!(
        points
            .iter()
            .zip(samples)
            .any(|(&p, s)| changed.cave_biome_at(p).region_id != s.region_id)
    );
}

#[test]
fn all_biomes_and_all_cross_biome_pairs_occur_on_long_spatial_paths() {
    let t = terrain();
    let mut found = [false; 4];
    let mut pairs = [[false; 4]; 4];
    let mut changes = 0;
    let mut previous = None;
    for z in (-4096..4096).step_by(64) {
        for x in (-4096..4096).step_by(32) {
            let c = t.cave_biome_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
            let s = c.at(c.surface - 40.0);
            let i = s.primary as usize;
            found[i] = true;
            if previous != Some(i) {
                changes += 1;
            }
            previous = Some(i);
            for (a, row) in pairs.iter_mut().enumerate() {
                for (b, exists) in row.iter_mut().enumerate().skip(a + 1) {
                    if s.weights[a] > 0.08 && s.weights[b] > 0.08 {
                        *exists = true;
                    }
                }
            }
        }
    }
    assert!(found.into_iter().all(|v| v), "{found:?}");
    for (a, row) in pairs.iter().enumerate() {
        for (b, &exists) in row.iter().enumerate().skip(a + 1) {
            assert!(
                exists,
                "missing {:?} -> {:?}",
                CaveBiome::ALL[a],
                CaveBiome::ALL[b]
            );
        }
    }
    assert!(changes > 30);
}

#[test]
fn all_nine_connector_transitions_use_the_same_field_as_chamber_surfaces() {
    let t = terrain();
    let mut c = t.surface_column(-320.5, -287.5);
    let p = [-320.5, -32.5, -287.5];
    c.cave = Default::default();
    c.cave_biome.surface = p[1] + 40.0;
    for (a, b) in [
        (0, 1),
        (0, 2),
        (0, 3),
        (1, 2),
        (2, 3),
        (1, 3),
        (1, 1),
        (2, 2),
        (3, 3),
    ] {
        let mut last: Option<[f32; 4]> = None;
        for step in 0..=100 {
            let f = step as f32 / 100.0;
            let mut w = [0.0; 4];
            w[a] += 1.0 - f;
            w[b] += f;
            c.cave_biome.sample = CaveBiomeSample::from_weights(w, 73, 24.0);
            let connector = t.cave_ecology_sample(p, c);
            c.cave.kind = super::super::CaveEcology::Stone;
            c.cave.radial = 0.0;
            c.cave.floor = -40.0;
            c.cave.ceiling = -20.0;
            assert_eq!(connector, t.cave_ecology_sample(p, c));
            if let Some(last) = last {
                for (a, b) in last.into_iter().zip(connector.weights) {
                    assert!((a - b).abs() < 0.012);
                }
            }
            // Compare the same subsurface rock layer. The synthetic chamber
            // changes the local floor normal, so grass on an exposed top is not
            // expected to match the connector's differently oriented surface.
            let chamber = t.cave_ecology_material(p, c, -1.5);
            c.cave = Default::default();
            assert_eq!(chamber, t.cave_ecology_material(p, c, -1.5));
            last = Some(connector.weights);
        }
    }
}

#[test]
fn connector_decorations_are_biome_specific_anchored_and_leave_headroom() {
    let t = terrain();
    let mut minerals = 0;
    let mut vegetation = 0;
    let mut fern_props = 0;
    for z in (-80..16).step_by(2) {
        for x in -96..0 {
            let mut c = t.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
            c.cave = Default::default();
            c.water_level = None;
            c.cave_biome.surface = 8.0;
            for y in -55..-16 {
                let p = [f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5];
                let original = t.cave_density_with_column(p, c).0;
                if original <= 0.0 {
                    continue;
                }
                c.cave_biome.sample = CaveBiomeSample::from_weights([0.0, 1.0, 0.0, 0.0], 1, 100.0);
                let salt = t.cave_ecology_material(p, c, original);
                assert!(!matches!(
                    salt,
                    Some(MaterialId::CaveRoot | MaterialId::CaveVine | MaterialId::CaveMushroom)
                ));
                if salt == Some(MaterialId::SaltCrystal) {
                    minerals += 1;
                    assert!(original <= 0.75);
                }
                c.cave_biome.sample = CaveBiomeSample::from_weights([0.0, 0.0, 1.0, 0.0], 2, 100.0);
                let lush = t.cave_ecology_material(p, c, original);
                assert_ne!(lush, Some(MaterialId::SaltCrystal));
                if matches!(
                    lush,
                    Some(MaterialId::CaveRoot | MaterialId::CaveVine | MaterialId::CaveMushroom)
                ) {
                    vegetation += 1;
                }
                let support = [i64::from(x), i64::from(y - 1), i64::from(z)];
                if let Some(prop) = t.cave_prop(support, c, SectionPos::new(-1, -2, -1)) {
                    assert!(t.cave_density_with_column([p[0], p[1] - 1.0, p[2]], c).0 <= 0.0);
                    if prop.kind == crate::props::PropKind::Fern {
                        fern_props += 1;
                    }
                }
            }
        }
    }
    assert!(
        minerals > 0 && vegetation > 0 && fern_props > 0,
        "salt={minerals} plants={vegetation} ferns={fern_props}"
    );
}

#[test]
fn new_rooms_vary_in_shape_height_position_and_do_not_close_whole_network_bands() {
    let t = terrain();
    let mut radii = std::collections::HashSet::new();
    let mut heights = std::collections::HashSet::new();
    for z in -8..8 {
        for x in -8..8 {
            let room = t.cave_region([x, z]);
            if room.radius > 0.0 {
                radii.insert(room.radius as i32);
                heights.insert(room.height as i32);
            }
            for dx in [0.0, 32.0, 80.0] {
                let px = x as f64 * 192.0 + dx;
                let pz = z as f64 * 192.0;
                assert!(t.cave_shape(px, pz).tunnel_width > 0.08);
                let a = t.sample([px, -45.0, pz - 1e-5]);
                let b = t.sample([px, -45.0, pz + 1e-5]);
                assert!((a.density - b.density).abs() < 0.01);
            }
        }
    }
    assert!(radii.len() > 20 && heights.len() > 20);
}

#[test]
fn prepared_sections_direct_samples_and_unload_reload_agree() {
    let t = terrain();
    let generator = SectionGenerator::new(t.clone());
    let p = SectionPos::new(-19, -3, -17);
    let col = generator.prepare_column([p.x, p.z], true).unwrap();
    let section = generator
        .build_prepared(p, 0, &Default::default(), &col)
        .unwrap();
    let origin = p.origin().unwrap();
    for z in 0..16 {
        for y in 0..16 {
            for x in [0, 15] {
                let global = [
                    origin[0] + x as i64,
                    origin[1] + y as i64,
                    origin[2] + z as i64,
                ];
                assert_eq!(
                    section.data.block([x, y, z]).unwrap(),
                    generator.block_at(global).unwrap()
                );
            }
        }
    }
    let other = SectionGenerator::new(terrain())
        .build(p, 0, &Default::default(), true)
        .unwrap();
    assert_eq!(section.data, other.data);
    assert_eq!(section.props, other.props);
}

#[test]
fn connector_switches_and_zero_weight_biomes_are_respected() {
    let mut cfg = WorldConfig {
        water: false,
        ..Default::default()
    };
    cfg.cave_biomes.distribution = terra_core::config::CaveDistribution {
        normal_weight: 1.0,
        salt_weight: 0.0,
        lush_weight: 0.0,
        cascade_weight: 0.0,
    };
    let t = TerraGenerator::new(cfg.clone()).unwrap();
    for x in (-500..500).step_by(17) {
        assert_eq!(
            t.cave_biome_at([f64::from(x), -20.0, -14.0]).weights,
            [1.0, 0.0, 0.0, 0.0]
        );
    }
    cfg.cave_biomes.connectors.inherit_biomes = false;
    let t = TerraGenerator::new(cfg).unwrap();
    let mut c = t.surface_column(-1.5, -1.5);
    c.cave = Default::default();
    c.cave_biome.sample = CaveBiomeSample::from_weights([0.0, 1.0, 0.0, 0.0], 1, 0.0);
    assert_eq!(
        t.cave_ecology_sample([-1.5, -40.5, -1.5], c).primary,
        CaveBiome::Normal
    );
}
