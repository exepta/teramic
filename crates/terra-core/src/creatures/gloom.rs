//! Append-only Gloom palettes and habitat choices shared by authority and renderer.
// Saved blue/purple spirits retain their original indices and appearance.
pub const STANDARD: u8 = 0;
pub const SHADOW: u8 = 1;
pub const BLOOD: u8 = 2;
pub const LIGHT: u8 = 3;
pub const FOREST: u8 = 4;
pub const ICE: u8 = 5;
pub const SAKURA: u8 = 6;
pub const COUNT: u8 = 7;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub shell: [f32; 3],
    pub petal: [f32; 3],
    pub glow: [f32; 3],
}
pub const PALETTES: [Palette; COUNT as usize] = [
    Palette {
        shell: [0.66, 0.79, 0.92],
        petal: [0.78, 0.68, 0.96],
        glow: [0.39, 0.86, 1.0],
    },
    Palette {
        shell: [0.29, 0.15, 0.48],
        petal: [0.66, 0.35, 0.88],
        glow: [0.76, 0.32, 1.0],
    },
    Palette {
        shell: [0.48, 0.12, 0.24],
        petal: [0.95, 0.22, 0.46],
        glow: [1.0, 0.18, 0.40],
    },
    Palette {
        shell: [0.91, 0.68, 0.23],
        petal: [1.0, 0.86, 0.42],
        glow: [1.0, 0.80, 0.30],
    },
    Palette {
        shell: [0.43, 0.66, 0.29],
        petal: [0.68, 0.86, 0.37],
        glow: [0.71, 1.0, 0.40],
    },
    Palette {
        shell: [0.67, 0.80, 0.97],
        petal: [0.82, 0.90, 1.0],
        glow: [0.52, 0.78, 1.0],
    },
    Palette {
        shell: [0.94, 0.63, 0.76],
        petal: [1.0, 0.79, 0.87],
        glow: [1.0, 0.44, 0.69],
    },
];
pub fn palette(variant: u8) -> Palette {
    PALETTES[usize::from(variant.min(COUNT - 1))]
}

pub fn surface_variant(biome: &str, temperature: f64, height: f64, seed: u64) -> u8 {
    match biome {
        "blood_forest" => BLOOD,
        "cherry_forest" => SAKURA,
        "forest" => FOREST,
        "spirit_blossom" if seed.is_multiple_of(4) => STANDARD,
        "spirit_blossom" => FOREST,
        "highland" if temperature < 0.35 || height >= 100.0 => ICE,
        _ => STANDARD,
    }
}
pub fn cave_variant(depth: f64, seed: u64) -> u8 {
    if depth >= 32.0 && seed.is_multiple_of(16) {
        SHADOW
    } else {
        LIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seven_palettes_are_distinct_and_habitats_keep_rare_shadows_underground() {
        let mut signatures = std::collections::BTreeSet::new();
        for palette in PALETTES {
            assert!(
                palette
                    .shell
                    .into_iter()
                    .chain(palette.petal)
                    .chain(palette.glow)
                    .all(|v| (0.0..=1.0).contains(&v))
            );
            signatures.insert(palette.glow.map(f32::to_bits));
        }
        assert_eq!(signatures.len(), 7);
        assert_eq!((STANDARD, SHADOW), (0, 1));
        assert_eq!(surface_variant("blood_forest", 0.5, 50.0, 0), BLOOD);
        assert_eq!(surface_variant("cherry_forest", 0.5, 50.0, 0), SAKURA);
        assert_eq!(surface_variant("highland", 0.1, 110.0, 0), ICE);
        assert_eq!(surface_variant("forest", 0.5, 50.0, 0), FOREST);
        for seed in 0..64 {
            assert_eq!(cave_variant(10.0, seed), LIGHT);
            assert_ne!(surface_variant("spirit_blossom", 0.5, 60.0, seed), SHADOW);
        }
        assert_eq!(
            (0..64).filter(|s| cave_variant(48.0, *s) == SHADOW).count(),
            4
        );
    }
}
