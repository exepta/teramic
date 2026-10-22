use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockDetailQuality {
    Low,
    Medium,
    #[default]
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BlockDetailConfig {
    pub detail_quality: BlockDetailQuality,
    pub micro_geometry: bool,
    pub micro_geometry_distance: f32,
    pub material_detail: bool,
    pub normal_mapping: bool,
    pub surface_variation: bool,
    pub leaf_geometry_quality: BlockDetailQuality,
}
impl Default for BlockDetailConfig {
    fn default() -> Self {
        Self {
            detail_quality: BlockDetailQuality::High,
            micro_geometry: true,
            micro_geometry_distance: 48.0,
            material_detail: true,
            normal_mapping: true,
            surface_variation: true,
            leaf_geometry_quality: BlockDetailQuality::High,
        }
    }
}
impl BlockDetailConfig {
    pub fn valid(self) -> bool {
        self.micro_geometry_distance.is_finite()
            && (16.0..=96.0).contains(&self.micro_geometry_distance)
    }
}
