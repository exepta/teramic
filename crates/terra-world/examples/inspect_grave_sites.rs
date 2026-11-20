//! Deterministic native-harness fixtures, discovered through the real planner.
use terra_core::{Config, WorldConfig};
use terra_world::{
    TerraGenerator,
    generation::structures::{GraveVariant, StructureKind},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WorldConfig::from_toml(include_str!("../../../configs/world.toml"))?;
    let seed = config.seed.0;
    let terrain = TerraGenerator::new(config)?;
    let mut found = [false; 4];
    for radius in 0_i64..=96 {
        for rz in -radius..=radius {
            for rx in -radius..=radius {
                if rx.abs().max(rz.abs()) != radius {
                    continue;
                }
                let region = [rx, rz];
                let Some(kind) = terrain.grave_site_candidate(region) else {
                    continue;
                };
                if kind == StructureKind::Cemetery && found[0] {
                    continue;
                }
                let Some(site) = terrain.structure_site(region) else {
                    continue;
                };
                let (slot, label, entrance, levels, route, treasures, name) =
                    match site.grave_dungeon() {
                        Some(info) => {
                            let (slot, label) = match info.variant {
                                GraveVariant::Knight => (1, "grave-knight"),
                                GraveVariant::Priestess => (2, "grave-priestess"),
                                GraveVariant::Scholar => (3, "grave-scholar"),
                            };
                            (
                                slot,
                                label,
                                info.entrance,
                                info.levels,
                                info.route,
                                info.treasures,
                                info.name,
                            )
                        }
                        None if site.kind == StructureKind::Cemetery => (
                            0,
                            "cemetery",
                            site.center,
                            Vec::new(),
                            Vec::new(),
                            Vec::new(),
                            String::new(),
                        ),
                        _ => continue,
                    };
                if found[slot] {
                    continue;
                }
                found[slot] = true;
                println!(
                    "{}",
                    serde_json::json!({"scenario":label,"seed":seed,"spirit_blossom_revision":2,"region":region,"center":site.center,"bounds":site.bounds,"entrance":entrance,"levels":levels,"route":route,"treasures":treasures,"name":name})
                );
                if found.into_iter().all(|v| v) {
                    return Ok(());
                }
            }
        }
    }
    Err(format!("Missing natural grave variants: {found:?}").into())
}
