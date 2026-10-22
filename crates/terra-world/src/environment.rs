use std::collections::HashMap;

/// Sparse authoritative surface state; block occupancy and material IDs stay unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SurfaceCondition {
    pub wetness: f32,
    pub charred: f32,
    pub snow: f32,
    pub fire: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SurfaceConditionError {
    #[error("wetness must be finite and within 0..=1")]
    InvalidWetness,
    #[error("charred must be finite and within 0..=1")]
    InvalidCharred,
    #[error("snow and fire must be finite and within 0..=1")]
    InvalidEnvironment,
    #[error("surface appearance revision overflow")]
    RevisionOverflow,
}

impl SurfaceCondition {
    pub fn new(wetness: f32, charred: f32) -> Result<Self, SurfaceConditionError> {
        let condition = Self {
            wetness,
            charred,
            ..Default::default()
        };
        condition.validate()?;
        Ok(condition)
    }

    pub fn validate(self) -> Result<(), SurfaceConditionError> {
        if [self.snow, self.fire]
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(SurfaceConditionError::InvalidEnvironment);
        }
        if !self.wetness.is_finite() || !(0.0..=1.0).contains(&self.wetness) {
            return Err(SurfaceConditionError::InvalidWetness);
        }
        if !self.charred.is_finite() || !(0.0..=1.0).contains(&self.charred) {
            return Err(SurfaceConditionError::InvalidCharred);
        }
        Ok(())
    }
}

/// Sparse conditions in local 16^3 cell coordinates, suitable for async render jobs.
pub type SurfaceSnapshot = HashMap<[usize; 3], SurfaceCondition>;

#[cfg(feature = "bevy")]
#[derive(bevy::prelude::Message, Clone, Copy, Debug)]
pub struct SurfaceConditionRequest {
    pub position: [i64; 3],
    pub condition: SurfaceCondition,
}

#[cfg(feature = "bevy")]
pub(crate) fn apply_surface_conditions(
    access: Option<bevy::prelude::Res<crate::streaming::WorldAccess>>,
    mut requests: bevy::prelude::MessageReader<SurfaceConditionRequest>,
    chunks: bevy::prelude::Res<crate::streaming::ChunkStore>,
    mut journal: bevy::prelude::ResMut<crate::edits::EditJournal>,
) {
    for request in requests.read() {
        if access
            .as_ref()
            .is_some_and(|access| !access.authoritative())
        {
            continue;
        }
        if !chunks
            .block_at(request.position)
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_some_and(terra_voxel::MaterialId::is_solid)
        {
            continue;
        }
        if request.condition.fire > 0.0
            && journal.surface_condition(request.position).fire == 0.0
            && journal.active_fires().take(256).count() >= 256
        {
            tracing::warn!("Active fire limit reached");
            continue;
        }
        if let Err(error) = journal.set_surface_condition(request.position, request.condition) {
            tracing::warn!(%error, position = ?request.position, "Rejected surface condition");
        }
    }
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
