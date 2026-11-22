//! A carried egg is an ordinary inventory block. Only a placed egg on straw incubates.
use super::CreatureKind;
use crate::PlayerUuid;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use terra_voxel::MaterialId;

pub const MAX_INCUBATING: usize = 512;

pub fn kind(material: u16) -> Option<CreatureKind> {
    match MaterialId::from_raw(material)? {
        MaterialId::DarkDragonEgg => Some(CreatureKind::DarkDragon),
        MaterialId::EarthDragonEgg => Some(CreatureKind::EarthDragon),
        MaterialId::WaterDragonEgg => Some(CreatureKind::WaterDragon),
        _ => None,
    }
}

pub fn material(kind: CreatureKind) -> Option<u16> {
    Some(match kind {
        CreatureKind::DarkDragon => MaterialId::DarkDragonEgg,
        CreatureKind::EarthDragon => MaterialId::EarthDragonEgg,
        CreatureKind::WaterDragon => MaterialId::WaterDragonEgg,
        _ => return None,
    } as u16)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct IncubatingEgg {
    pub position: [i64; 3],
    pub shell: u16,
    pub owner: PlayerUuid,
    pub elapsed_days: f64,
}

pub fn valid(eggs: &[IncubatingEgg]) -> bool {
    let mut positions = BTreeSet::new();
    eggs.len() <= MAX_INCUBATING
        && eggs.iter().all(|egg| {
            kind(egg.shell).is_some()
                && !egg.owner.is_nil()
                && (0.0..=7.0).contains(&egg.elapsed_days)
                && egg.position.iter().all(|v| v.unsigned_abs() <= 999_000)
                && positions.insert(egg.position)
        })
}
