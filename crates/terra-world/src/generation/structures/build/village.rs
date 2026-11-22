//! Revision two uses the same voxel builder with seeded street plans and rotated homes.
use super::*;

mod buildings;
mod house_template;
mod lighting;
mod paths;
pub(super) fn house_template() -> Result<Vec<StructureObject>, String> {
    house_template::cells()
}
mod populated;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    Plaza,
    Linear,
    Courtyard,
}
impl Layout {
    fn from_seed(seed: u32) -> Self {
        match (seed >> 16) % 3 {
            0 => Self::Plaza,
            1 => Self::Linear,
            _ => Self::Courtyard,
        }
    }
}

#[derive(Clone, Debug)]
struct Plot {
    center: [i64; 2],
    width: i64,
    depth: i64,
    height: i64,
    facing: u8,
    floor: i64,
    role: BuildingRole,
    variant: u32,
}
impl Plot {
    fn point(&self, x: i64, y: i64, z: i64) -> [i64; 3] {
        let [x, z] = rotate([x - self.width / 2, z - self.depth / 2], self.facing);
        [self.center[0] + x, self.floor + y, self.center[1] + z]
    }
    fn bounds(&self) -> [[i64; 2]; 2] {
        let corners = [
            [0, 0],
            [self.width, 0],
            [0, self.depth],
            [self.width, self.depth],
        ];
        let mut low = [i64::MAX; 2];
        let mut high = [i64::MIN; 2];
        for [x, z] in corners {
            let p = self.point(x, 0, z);
            for (i, a) in [0, 2].into_iter().enumerate() {
                low[i] = low[i].min(p[a]);
                high[i] = high[i].max(p[a]);
            }
        }
        [low, high]
    }
    fn portal(&self) -> [i64; 3] {
        self.point(self.width / 2, 0, -4)
    }
}
fn rotate([x, z]: [i64; 2], facing: u8) -> [i64; 2] {
    match facing % 4 {
        0 => [x, z],
        1 => [z, -x],
        2 => [-x, -z],
        _ => [-z, x],
    }
}

pub(super) fn plan(terrain: &TerraGenerator, region: [i64; 2], seed: u32) -> Option<Site> {
    if seed % 10 == 9 {
        return None;
    }
    let (jitter, half) = if terrain.config().structures_revision >= 4 {
        (17, 8)
    } else {
        (97, 48)
    };
    let x = region[0] * REGION_SIZE + 192 + i64::from((seed >> 4) % jitter) - half;
    let z = region[1] * REGION_SIZE + 192 + i64::from((seed >> 12) % jitter) - half;
    let biome = terrain.biome_at(x as f64, z as f64);
    if biome.weights.ocean > 0.3 {
        return None;
    }
    let style = if biome.weights.desert + biome.weights.cliffted_desert > 0.4 {
        VillageStyle::Desert
    } else if biome.weights.highland > 0.2 {
        VillageStyle::Highland
    } else {
        VillageStyle::Temperate
    };
    let kind = match seed % 9 {
        0..=3 => StructureKind::Village,
        4..=6 => StructureKind::Ruin,
        _ => StructureKind::Mine,
    };
    // Keep the original seeded category: disabling one kind must not reroll
    // its regions into different structures or alter enabled neighbors.
    if !kind.enabled(terrain.config().structures) {
        return None;
    }
    if kind == StructureKind::Village
        && style == VillageStyle::Temperate
        && !matches!(
            biome.weights.dominant(),
            super::super::super::biomes::BiomeKind::Plains
                | super::super::super::biomes::BiomeKind::Forest
        )
    {
        return None;
    }
    let y = (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64 + 1;
    let mut builder = Builder {
        terrain,
        summary: SiteSummary {
            id: (u64::from(seed) << 32) ^ region[0] as u64 ^ (region[1] as u64).rotate_left(17),
            kind,
            style,
            center: [x, y, z],
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
    match kind {
        StructureKind::Village => build(&mut builder, x, z)?,
        StructureKind::Ruin => builder.ruin(x - 7, z - 6)?,
        StructureKind::Mine => super::super::mine::build(&mut builder, x, z)?,
        StructureKind::Kingdom
        | StructureKind::Outpost
        | StructureKind::Cemetery
        | StructureKind::GraveDungeon
        | StructureKind::DarkDragonLair
        | StructureKind::EarthDragonLair
        | StructureKind::WaterDragonLair => return None,
    }
    builder.finish()
}

fn build(builder: &mut Builder<'_>, x: i64, z: i64) -> Option<()> {
    if builder.terrain.config().structures_revision >= 4 {
        return populated::build(builder, x, z);
    }
    use BuildingRole::*;
    let layout = Layout::from_seed(builder.seed);
    let rotation = ((builder.seed >> 24) % 4) as u8;
    let positions = match layout {
        Layout::Plaza => [
            (0, -25, 2, Church),
            (-24, -4, 3, Smith),
            (25, -19, 1, Watch),
            (25, 14, 1, Medic),
            (-24, 18, 3, House),
            (0, 27, 0, House),
        ],
        Layout::Linear => [
            (0, -26, 2, Church),
            (-18, -7, 3, Smith),
            (0, 31, 0, Watch),
            (18, -6, 1, Medic),
            (-18, 16, 3, House),
            (18, 17, 1, House),
        ],
        Layout::Courtyard => [
            (0, -27, 2, Church),
            (-27, -4, 3, Smith),
            (0, 28, 0, Watch),
            (27, -3, 1, Medic),
            (-26, 23, 3, House),
            (26, 24, 1, House),
        ],
    };
    let mut plots = Vec::with_capacity(positions.len());
    for (i, (u, v, facing, role)) in positions.into_iter().enumerate() {
        let variant =
            super::super::super::seed::layer_seed(u64::from(builder.seed), 800 + i as u64);
        let offset = rotate(
            [
                u + i64::from(variant % 3) - 1,
                v + i64::from((variant >> 4) % 3) - 1,
            ],
            rotation,
        );
        let (width, depth, height) = match role {
            Church => (11, 15, 6),
            Smith => (11, 9, 5),
            Watch => (7, 7, 5),
            Medic => (9, 9, 5),
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
            variant,
        };
        let [low, high] = plot.bounds();
        let floor = builder.fit(low[0], low[1], high[0] - low[0], high[1] - low[1]);
        #[cfg(test)]
        if floor.is_none() && std::env::var_os("TERRA_STRUCTURE_TRACE").is_some() {
            eprintln!(
                "village seed={} stage=plot-fit role={role:?} bounds={low:?}..{high:?}",
                builder.seed
            );
        }
        plot.floor = floor?;
        plots.push(plot);
    }
    let farm_offset = rotate([-29, -29], rotation);
    let farm = [x + farm_offset[0] - 6, z + farm_offset[1] - 4];
    let farm_floor = builder.fit(farm[0], farm[1], 12, 8);
    #[cfg(test)]
    if farm_floor.is_none() && std::env::var_os("TERRA_STRUCTURE_TRACE").is_some() {
        eprintln!("village seed={} stage=farm-fit", builder.seed);
    }
    let farm_y = farm_floor?;
    let farm_bounds = [[farm[0] - 1, farm[1] - 1], [farm[0] + 13, farm[1] + 9]];
    let mut roads = paths::Routes::new([x, z], &plots, farm_bounds);
    for plot in &plots {
        let connection = roads.connect(builder, plot.portal());
        #[cfg(test)]
        if connection.is_none() && std::env::var_os("TERRA_STRUCTURE_TRACE").is_some() {
            eprintln!(
                "village seed={} stage=street role={:?} portal={:?}",
                builder.seed,
                plot.role,
                plot.portal()
            );
        }
        connection?;
    }
    let connection = roads.connect(builder, [farm[0] + 6, farm_y, farm[1] - 2]);
    #[cfg(test)]
    if connection.is_none() && std::env::var_os("TERRA_STRUCTURE_TRACE").is_some() {
        eprintln!("village seed={} stage=farm-street", builder.seed);
    }
    connection?;
    for plot in &plots {
        buildings::build(builder, plot);
    }
    builder.farm(farm[0], farm[1])?;
    for dz in -2..=0 {
        let px = farm[0] + 6;
        let pz = farm[1] + dz;
        let ground = builder.ground(px, pz);
        builder.fill(
            [px, ground.min(farm_y - 1) - 1, pz],
            [px, farm_y - 1, pz],
            2,
        );
        builder.set([px, farm_y - 1, pz], PATH);
        builder.fill([px, farm_y, pz], [px, farm_y + 2, pz], 0);
    }
    buildings::plaza(builder, [x, z], layout, &roads);
    Some(())
}
