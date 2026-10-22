//! Reproducible natural cave/biome locations for native inspection, never injected blocks.
use terra_core::WorldConfig;
use terra_world::{
    TerraGenerator,
    generation::{CaveKind, biomes::BiomeKind},
};

fn main() -> Result<(), String> {
    let terrain = TerraGenerator::new(WorldConfig::default()).map_err(|error| error.to_string())?;
    let mut salt = None;
    let mut cheese = None;
    let mut dripstone = None;
    let mut spaghetti = None;
    let mut eligible = 0;
    let mut salty = 0;
    for z in -96_i32..=96 {
        for x in -96_i32..=96 {
            let px = f64::from(x) * 192.0 + 96.0;
            let pz = f64::from(z) * 192.0 + 96.0;
            let biome = terrain.biome_at(px, pz).weights.dominant();
            if biome == BiomeKind::ClifftedDesert {
                eligible += 1;
                if let Some(center) = terrain.salt_chamber_center(px, pz) {
                    salty += 1;
                    salt.get_or_insert(center);
                }
            }
            if x.abs() <= 3 && z.abs() <= 3 {
                match terrain.cave_kind_at(px, pz) {
                    CaveKind::Cheese => {
                        cheese = cheese.or_else(|| terrain.cave_chamber_center(px, pz));
                    }
                    CaveKind::Dripstone => {
                        dripstone = dripstone.or_else(|| terrain.cave_chamber_center(px, pz));
                    }
                    CaveKind::Spaghetti => {
                        spaghetti.get_or_insert([px, -32.0, pz]);
                    }
                }
            }
        }
    }
    println!(
        "seed=42042 eligible_cliffted_regions={eligible} salt_regions={salty} fraction={:.5}",
        salty as f64 / f64::from(eligible.max(1))
    );
    println!("salt={salt:?}\ncheese={cheese:?}\ndripstone={dripstone:?}\nspaghetti={spaghetti:?}");
    if salt.is_none() || cheese.is_none() || dripstone.is_none() || spaghetti.is_none() {
        return Err("Missing natural cave type".into());
    }
    Ok(())
}
