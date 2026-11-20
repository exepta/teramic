//! Client-only selection of saved work areas; no authority or persistence changes.
use crate::{building_jobs::BuildArea, kingdom::KingdomId};
use bevy::prelude::Resource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkAreaTarget {
    Worker(u64),
    Settlement(KingdomId),
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WorkAreaDisplay {
    pub target: Option<WorkAreaTarget>,
}

/// Session-local construction overlays. Manual area selection is kept separate
/// so leaving construction restores the previous gameplay display.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConstructionAreaDisplay {
    pub kingdom: Option<KingdomId>,
    pub visible: bool,
}

impl WorkAreaDisplay {
    pub fn toggle(&mut self, target: WorkAreaTarget) {
        self.target = (self.target != Some(target)).then_some(target);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkAreaKind {
    Work,
    Storage,
    Residential,
    BuildingOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkAreaMark {
    pub kind: WorkAreaKind,
    pub bounds: BuildArea,
}
