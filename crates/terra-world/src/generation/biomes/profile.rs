#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiomeWeights {
    pub ocean: f64,
    pub plains: f64,
    pub forest: f64,
    pub desert: f64,
    pub highland: f64,
    pub mushroom_park: f64,
    pub cliffted_desert: f64,
    pub cherry_forest: f64,
    pub spirit_blossom: f64,
    pub blood_forest: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiomeSample {
    pub temperature: f64,
    pub humidity: f64,
    pub weights: BiomeWeights,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainProfile {
    pub elevation: f64,
    pub hills: f64,
    pub mountains: f64,
    pub soil_depth: f64,
    pub forest_cover: f64,
    pub terrace_scale: f64,
}

pub use terra_core::BiomeKind;

impl BiomeWeights {
    pub fn values(self) -> [f64; 10] {
        [
            self.ocean,
            self.plains,
            self.forest,
            self.desert,
            self.highland,
            self.mushroom_park,
            self.cliffted_desert,
            self.cherry_forest,
            self.spirit_blossom,
            self.blood_forest,
        ]
    }
    pub(super) fn from_values(values: [f64; 10]) -> Self {
        let [
            ocean,
            plains,
            forest,
            desert,
            highland,
            mushroom_park,
            cliffted_desert,
            cherry_forest,
            spirit_blossom,
            blood_forest,
        ] = values;
        Self {
            ocean,
            plains,
            forest,
            desert,
            highland,
            mushroom_park,
            cliffted_desert,
            cherry_forest,
            spirit_blossom,
            blood_forest,
        }
    }

    /// Only for diagnostics; generation always blends every weight.
    pub fn dominant(self) -> BiomeKind {
        let mut best = 0;
        let values = self.values();
        for i in 1..values.len() {
            if values[i] > values[best] {
                best = i;
            }
        }
        [
            BiomeKind::Ocean,
            BiomeKind::Plains,
            BiomeKind::Forest,
            BiomeKind::Desert,
            BiomeKind::Highland,
            BiomeKind::MushroomPark,
            BiomeKind::ClifftedDesert,
            BiomeKind::CherryForest,
            BiomeKind::SpiritBlossom,
            BiomeKind::BloodForest,
        ][best]
    }
}

impl BiomeSample {
    /// Reweight climate's existing coherent regions without changing continental
    /// coverage. Legacy snapshots with unit weights take the exact old path.
    pub(crate) fn with_distribution(mut self, definitions: &terra_core::BiomeDefinitions) -> Self {
        if definitions.entries[1..]
            .iter()
            .all(|definition| definition.selection_weight == 1.0)
        {
            return self;
        }
        let mut values = self.weights.values();
        let land: f64 = values[1..].iter().sum();
        for (value, definition) in values[1..].iter_mut().zip(&definitions.entries[1..]) {
            *value *= definition.selection_weight;
        }
        let selected: f64 = values[1..].iter().sum();
        if selected <= f64::EPSILON {
            return self;
        }
        for value in &mut values[1..] {
            *value *= land / selected;
        }
        self.weights = BiomeWeights::from_values(values);
        self
    }

    /// Disjoint thermal eligibility leaves a continuous temperate buffer. Lost
    /// mountain/desert cover becomes lowland rather than jumping to the opposite extreme.
    pub(crate) fn with_thermal_buffers(mut self) -> Self {
        let cold = 1.0 - super::smoothstep(0.28, 0.40, self.temperature);
        let hot = super::smoothstep(0.60, 0.72, self.temperature);
        let w = &mut self.weights;
        let lost = w.highland * (1.0 - cold) + (w.desert + w.cliffted_desert) * (1.0 - hot);
        w.highland *= cold;
        w.desert *= hot;
        w.cliffted_desert *= hot;
        let forest = super::smoothstep(0.30, 0.70, self.humidity);
        w.forest += lost * forest;
        w.plains += lost * (1.0 - forest);
        self
    }

    pub fn aridity(self) -> f64 {
        self.weights.desert + self.weights.cliffted_desert
    }
    /// Climate creates the mountain range; altitude decides its visible ecology.
    /// Redistribute suppressed highland cover without changing the ocean or desert.
    pub(crate) fn at_height(mut self, height: f64) -> Self {
        let highland = self.weights.highland;
        self.weights.highland *= super::smoothstep(80.0, 108.0, height);
        let lowland = highland - self.weights.highland;
        let forest = super::smoothstep(0.35, 0.72, self.humidity);
        self.weights.forest += lowland * forest;
        self.weights.plains += lowland * (1.0 - forest);
        self
    }

    /// All profiles participate: choosing a dominant biome would introduce borders.
    pub fn profile(self) -> TerrainProfile {
        let w = self.weights;
        let blend = |ocean, plains, forest, desert, highland, mushroom_park| {
            w.ocean * ocean
                + w.plains * plains
                + w.forest * forest
                + w.desert * desert
                + w.highland * highland
                + w.mushroom_park * mushroom_park
                + w.cliffted_desert * desert
                + w.cherry_forest * forest
                + w.spirit_blossom * forest
                + w.blood_forest * forest
        };
        TerrainProfile {
            elevation: blend(-40.0, 6.0, 12.0, 10.0, 18.0, 9.0) + w.cliffted_desert * 22.0,
            hills: blend(0.20, 0.50, 0.95, 0.60, 1.20, 0.65),
            mountains: blend(0.04, 0.15, 0.40, 0.18, 1.15, 0.18) + w.cliffted_desert * 0.35,
            soil_depth: blend(3.0, 4.0, 4.8, 6.0, 2.8, 5.2),
            forest_cover: blend(0.0, 0.22, 1.15, 0.0, 0.40, 0.70),
            terrace_scale: blend(0.80, 0.80, 1.0, 0.85, 1.25, 0.85),
        }
    }
}
