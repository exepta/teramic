//! Freeze the four-field object payload used by REG3 through RG12.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use terra_world::edits::BlockObjectState;

#[derive(Serialize, Deserialize)]
struct StoredObjectState {
    facing: u8,
    peer: Option<[i64; 3]>,
    secondary: bool,
    wood: terra_core::WoodSpecies,
}

pub(super) fn serialize<S: Serializer>(
    objects: &[([u8; 3], BlockObjectState)],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    objects
        .iter()
        .map(|(cell, state)| {
            (
                *cell,
                StoredObjectState {
                    facing: state.facing,
                    peer: state.peer,
                    secondary: state.secondary,
                    wood: state.wood,
                },
            )
        })
        .collect::<Vec<_>>()
        .serialize(serializer)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<([u8; 3], BlockObjectState)>, D::Error> {
    Vec::<([u8; 3], StoredObjectState)>::deserialize(deserializer).map(|objects| {
        objects
            .into_iter()
            .map(|(cell, state)| {
                (
                    cell,
                    BlockObjectState {
                        facing: state.facing,
                        peer: state.peer,
                        secondary: state.secondary,
                        wood: state.wood,
                        rail_curve: None,
                        liquid: None,
                    },
                )
            })
            .collect()
    })
}
