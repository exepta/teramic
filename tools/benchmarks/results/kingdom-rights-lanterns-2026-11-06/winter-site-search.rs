use terra_core::{WorldConfig, WorldSeed};
use terra_world::generation::{TerraGenerator, TerrainGenerator};
use terra_world::generation::structures::{StructureKind, SettlementLayout, REGION_SIZE};
use std::time::{Instant, Duration};
fn main() {
    let started = Instant::now();
    let mut config = WorldConfig::default();
    config.seed = WorldSeed(42042);
    config.structures_revision = 8;
    let terrain = TerraGenerator::new(config).unwrap();
    let mut candidates = Vec::new();
    for ring in 0_i64..=64 {
        for z in -ring..=ring {
            for x in -ring..=ring {
                if x.abs().max(z.abs()) != ring || [x,z] == [-10,12] { continue; }
                let region = [x,z];
                if terrain.royal_site_candidate(region) != Some(StructureKind::Kingdom) { continue; }
                let cx = (x * REGION_SIZE + REGION_SIZE/2) as f64;
                let cz = (z * REGION_SIZE + REGION_SIZE/2) as f64;
                let b = terrain.biome_at(cx,cz).weights;
                if b.ocean > 0.25 || b.desert + b.cliffted_desert > 0.5 || b.highland <= 0.5 { continue; }
                let mut forest = 0.0;
                let mut highland = 0.0;
                for dz in [-160.0,-80.0,0.0,80.0,160.0] {
                    for dx in [-160.0,-80.0,0.0,80.0,160.0] {
                        let w = terrain.biome_at(cx+dx,cz+dz).weights;
                        forest += w.forest + w.cherry_forest;
                        highland += w.highland;
                    }
                }
                forest /= 25.0;
                highland /= 25.0;
                let h = terrain.surface_height(cx,cz);
                candidates.push((forest - highland*0.05, region, b.highland, b.forest, forest, highland, h));
            }
        }
    }
    candidates.sort_by(|a,b| a.0.total_cmp(&b.0).then_with(||a.1.cmp(&b.1)));
    eprintln!("biome_candidates={} scan={:?} config_revision={} landscape={} ecology={}", candidates.len(), started.elapsed(), terrain.config().structures_revision,terrain.config().landscape_revision,terrain.config().ecology_revision);
    let mut plans = 0;
    let mut matches = 0;
    for (_,region,highland,forest,meanforest,meanhighland,h) in candidates.into_iter().take(128) {
        if started.elapsed() > Duration::from_secs(55) { eprintln!("time bound"); break; }
        plans += 1;
        let Some(summary) = terrain.structure_site(region).filter(|s| s.layout() == Some(SettlementLayout::KingdomWinter)) else { continue; };
        let Some(settlement) = terrain.village_settlement(region) else { continue; };
        let civilians = settlement.residents.iter().filter(|r|r.role < terra_core::creatures::villagers::CIVILIAN_ROLE_COUNT).count();
        let soldiers = settlement.residents.len()-civilians;
        if !(50..=80).contains(&civilians) || !(26..=64).contains(&soldiers) { continue; }
        println!("region={region:?} center={:?} bounds={:?} highland={highland:.6} forest={forest:.6} mean_forest_320m={meanforest:.6} mean_highland_320m={meanhighland:.6} height={h:.3} civilians={civilians} soldiers={soldiers} buildings={} plans={plans} elapsed={:?}", summary.center, summary.bounds, summary.buildings.len(), started.elapsed());
        matches += 1;
        if matches >= 3 { break; }
    }
    eprintln!("done plans={plans} matches={matches} elapsed={:?}",started.elapsed());
}
