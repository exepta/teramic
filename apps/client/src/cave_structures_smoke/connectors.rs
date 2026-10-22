use bevy::prelude::*;
use terra_world::{TerraGenerator, TerrainGenerator, generation::CaveBiome};

pub(super) struct View {
    pub name: String,
    pub eye: Vec3,
    pub target: Vec3,
}

pub(super) fn locate(terrain: &TerraGenerator, index: usize) -> Result<View, String> {
    let Some((a, b)) = [
        (CaveBiome::Salt, CaveBiome::Lush),
        (CaveBiome::Lush, CaveBiome::Cascade),
        (CaveBiome::Salt, CaveBiome::Cascade),
    ]
    .get(index)
    .copied() else {
        return Err("TERRA_CAVE_CONNECTOR must be 0, 1 or 2".into());
    };
    for z in (-2048..2048).step_by(16) {
        for x in (-2048..2048).step_by(16) {
            let x = f64::from(x) + 0.5;
            let z = f64::from(z) + 0.5;
            let column = terrain.surface_column(x, z);
            for depth in [24.0, 40.0, 64.0] {
                let y = (column.height - depth).floor() + 0.5;
                if y < -110.0 {
                    continue;
                }
                let p = [x, y, z];
                let sample = terrain.cave_ecology_sample(p, column);
                if sample.weights[a as usize] < 0.22
                    || sample.weights[b as usize] < 0.22
                    || !terrain.cave_connector_at(p)
                    || terrain.sample(p).density < 0.8
                {
                    continue;
                }
                for direction in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
                    if (1..=4).any(|d| {
                        terrain
                            .sample([
                                x + direction[0] * f64::from(d),
                                y,
                                z + direction[1] * f64::from(d),
                            ])
                            .density
                            < 0.4
                    }) {
                        continue;
                    }
                    let eye = Vec3::new(x as f32, y as f32, z as f32);
                    return Ok(View {
                        name: format!("{}-{}", a.label(), b.label()),
                        eye,
                        target: eye
                            + Vec3::new(direction[0] as f32 * 5.0, -0.3, direction[1] as f32 * 5.0),
                    });
                }
            }
        }
    }
    Err(format!(
        "No open {} / {} connector in the bounded fixture search",
        a.label(),
        b.label()
    ))
}
