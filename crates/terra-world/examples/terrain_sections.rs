//! CPU section-population comparison, not a renderer or FPS benchmark.
//!
//! Each pass generates 36 sections: three fixed X/Z anchors, two-by-two
//! columns per anchor, and three vertical sections around each local surface.
//! Revisions use identical settings and X/Z positions, but their surface Y,
//! generated geometry, filled-voxel count and props intentionally differ.
//! Cold and warm passes reuse the same generator and its regional caches.
use std::{hint::black_box, time::Instant};
use terra_core::{WorldConfig, WorldSeed};
use terra_voxel::SectionPos;
use terra_world::{TerraGenerator, generation::SectionGenerator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "CPU section population; no Bevy app, renderer, GPU or FPS measurement; \
         with_mesh=false includes voxel and prop generation, not meshing; \
         water/rivers/caves/trees enabled, structures disabled; \
         3 anchors, 2x2 columns per anchor, 3 vertical sections per column; \
         identical X/Z and settings, local ground Y and geometry differ by design; \
         timing includes surface-height queries; warm pass reuses regional caches"
    );
    for seed in [42042, 7, 99173] {
        for revision in [4, 5] {
            let terrain = TerraGenerator::new(WorldConfig {
                seed: WorldSeed(seed),
                landscape_revision: revision,
                water: true,
                rivers: true,
                caves: true,
                trees: true,
                structures_revision: 0,
                ..Default::default()
            })?;
            let sections = SectionGenerator::new(terrain.clone());
            for pass in ["cold", "warm"] {
                let start = Instant::now();
                let mut filled = 0usize;
                let mut props = 0usize;
                let mut count = 0usize;
                for [cx, cz] in [[256_i64, 256_i64], [-2048, 3072], [5376, -4864]] {
                    for dz in 0..2 {
                        for dx in 0..2 {
                            let x = cx.div_euclid(16) + dx;
                            let z = cz.div_euclid(16) + dz;
                            let height =
                                terrain.surface_height((x * 16 + 8) as f64, (z * 16 + 8) as f64);
                            let y = (height.floor() as i64).div_euclid(16);
                            for dy in [-1, 0, 1] {
                                let result = sections.build(
                                    SectionPos::new(x, y + dy, z),
                                    0,
                                    &Default::default(),
                                    false,
                                )?;
                                filled +=
                                    result.data.blocks().iter().filter(|&&id| id != 0).count();
                                props += result.props.len();
                                count += 1;
                                black_box(result);
                            }
                        }
                    }
                }
                println!(
                    "seed={seed} revision={revision} pass={pass} sections={count} \
                     ms={:.6} filled_voxels={filled} props={props}",
                    start.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
    }
    Ok(())
}
