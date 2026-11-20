//! Capture the same authored house used by settlements without spawning its loot or NPCs.
use super::*;

pub(super) fn cells() -> Result<Vec<StructureObject>, String> {
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        base_height: 0.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        sea_level: -64.0,
        biomes: false,
        trees: false,
        rivers: false,
        water: false,
        caves: false,
        landscape_revision: 3,
        structures_revision: 8,
        ..Default::default()
    })
    .map_err(|error| error.to_string())?;
    let mut builder = Builder {
        terrain: &terrain,
        summary: SiteSummary {
            id: 0,
            kind: StructureKind::Village,
            style: VillageStyle::Temperate,
            center: [0; 3],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed: 0,
    };
    buildings::build(
        &mut builder,
        &Plot {
            center: [0, 0],
            width: 6,
            depth: 6,
            height: 4,
            facing: 0,
            floor: 0,
            role: BuildingRole::House,
            variant: 0,
        },
    );
    if builder.failed {
        return Err("Default village house exceeds template bounds".into());
    }
    let mut result = Vec::new();
    for (section, raw) in &builder.patches {
        let origin = section.origin().map_err(|error| error.to_string())?;
        for (index, &material) in raw.iter().enumerate().filter(|(_, m)| **m != u16::MAX) {
            let p = [
                origin[0] + (index % 16) as i64,
                origin[1] + (index / 16 % 16) as i64,
                origin[2] + (index / 256) as i64,
            ];
            result.push(StructureObject {
                position: p,
                material,
                state: builder.objects.get(&p).and_then(|o| o.state),
                loot: None,
                crop: None,
            });
        }
    }
    Ok(result)
}
