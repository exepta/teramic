//! Advance the construction chain only after its own accepted, replicated edit.
use super::PlacementPreview;
use bevy::prelude::*;
use std::time::{Duration, Instant};
use terra_core::{
    WoodSpecies,
    rails::{FixedPoint, Piece},
};
use terra_network::{
    client::{ConfirmedEdit, RailPlacementAction, RailPlacementSent},
    protocol::session::EditOutcome,
};
use terra_world::{edits::EditJournal, placement::Layout, rails, streaming::ChunkStore};

#[derive(Debug)]
pub struct Pending {
    pub token: u64,
    pub action: RailPlacementAction,
    request: Option<u64>,
    accepted: bool,
    piece: Piece,
    wood: WoodSpecies,
    destination: FixedPoint,
    started: Instant,
    identity: Option<(Option<Entity>, Option<terra_core::PlayerUuid>)>,
}
impl Pending {
    pub fn new(
        token: u64,
        action: RailPlacementAction,
        plan: &Layout,
        wood: WoodSpecies,
        session: Option<&terra_network::client::ClientSession>,
    ) -> Option<Self> {
        let endpoint = rails::next_endpoint(plan)?;
        let state = plan
            .states
            .iter()
            .find(|(cell, _)| *cell == endpoint.anchor)?
            .1?;
        let material = plan
            .edits
            .iter()
            .find(|edit| edit.position == endpoint.anchor)?
            .material;
        let piece = rails::piece_from_state(endpoint.anchor, material, state)?;
        let destination = piece
            .ports()
            .into_iter()
            .find(|port| port.index == endpoint.port)?
            .position;
        Some(Self {
            token,
            action,
            request: None,
            accepted: false,
            piece,
            wood,
            destination,
            started: Instant::now(),
            identity: session.map(|session| (session.connection, session.uuid)),
        })
    }
}

pub(crate) fn confirm(
    mut preview: ResMut<PlacementPreview>,
    mut sent: MessageReader<RailPlacementSent>,
    mut confirmed: MessageReader<ConfirmedEdit>,
    chunks: Res<ChunkStore>,
    journal: Option<Res<EditJournal>>,
    session: Option<Res<terra_network::client::ClientSession>>,
) {
    for receipt in sent.read() {
        if let Some(pending) = &mut preview.rail_pending
            && receipt.token == pending.token
            && receipt.action == pending.action.edit_action()
        {
            pending.request = Some(receipt.request);
        }
    }
    for receipt in confirmed.read() {
        if let Some(pending) = &mut preview.rail_pending
            && pending.request == Some(receipt.request)
            && receipt.action == pending.action.edit_action()
        {
            if receipt.outcome == EditOutcome::Applied {
                pending.accepted = true;
            } else {
                // The previously confirmed start remains available for a retry.
                preview.cancel_rail_pending();
            }
        }
    }
    if let Some(journal) = &journal
        && let Some(endpoint) = preview.rail_start
    {
        let actual = rails::owner_at(journal, endpoint.anchor);
        if preview.rail_origin.is_none() {
            preview.rail_origin =
                actual.and_then(|piece| Some((piece, journal.object_state(piece.anchor)?.wood)));
        }
        let source_valid = preview.rail_origin.is_some_and(|(piece, wood)| {
            actual == Some(piece)
                && rails::complete(journal, piece)
                && journal
                    .object_state(piece.anchor)
                    .is_some_and(|state| state.wood == wood)
                && rails::cells(piece)
                    .iter()
                    .all(|cell| chunks.block_at(*cell) == Some(piece.material))
                && (preview.rail_pending.is_some()
                    || piece
                        .ports()
                        .into_iter()
                        .find(|port| port.index == endpoint.port)
                        .is_some_and(|port| {
                            rails::continuation_endpoint(journal, piece, port.position)
                                == Some(endpoint)
                        }))
        });
        if !source_valid {
            preview.cancel_rail_chain();
            return;
        }
    }
    let Some(pending) = &preview.rail_pending else {
        return;
    };
    if pending.identity
        != session
            .as_deref()
            .map(|session| (session.connection, session.uuid))
    {
        preview.cancel_rail_chain();
        return;
    }
    if pending.started.elapsed() >= Duration::from_secs(30) {
        preview.cancel_rail_pending();
        return;
    }
    let Some(journal) = journal else { return };
    if !pending.accepted {
        return;
    }
    // A complete but different replacement is definitive. A missing/partial
    // section can still be in flight on the reliable replication stream.
    if let Some(actual) = rails::owner_at(&journal, pending.piece.anchor)
        && rails::complete(&journal, actual)
        && (actual != pending.piece
            || journal
                .object_state(actual.anchor)
                .is_none_or(|state| state.wood != pending.wood))
    {
        preview.cancel_rail_pending();
        return;
    }
    if !rails::complete(&journal, pending.piece)
        || rails::owner_at(&journal, pending.piece.anchor) != Some(pending.piece)
        || journal
            .object_state(pending.piece.anchor)
            .is_none_or(|state| state.wood != pending.wood)
        || rails::cells(pending.piece)
            .iter()
            .any(|cell| chunks.block_at(*cell) != Some(pending.piece.material))
    {
        return;
    }
    let endpoint = rails::continuation_endpoint(&journal, pending.piece, pending.destination);
    let origin = (pending.piece, pending.wood);
    preview.rail_start = endpoint;
    preview.rail_origin = endpoint.map(|_| origin);
    preview.cancel_rail_pending();
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn an_applied_but_missing_replica_has_a_finite_wait_and_cannot_rearm_after_timeout() {
        let mut chunks = ChunkStore::default();
        let piece = Piece::seed([4, 1, 4], 0).unwrap();
        for cell in rails::cells(piece)
            .into_iter()
            .chain(piece.supports().iter().map(|point| {
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]))
            }))
        {
            let (owner, local) = global_to_section(cell);
            chunks
                .loaded
                .entry(owner)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                })
                .data
                .set_block(local, if cell[1] == 0 { 3 } else { 0 })
                .unwrap();
        }
        let journal = EditJournal::default();
        let hit = terra_voxel::blocks::BlockHit {
            position: [4, 0, 4],
            normal: [0, 1, 0],
            distance: 2.0,
        };
        let plan = rails::seed(
            &chunks,
            &journal,
            hit,
            ([4.5, 3.0, 4.5], [0.0, -1.0, 0.0]),
            [std::f32::consts::PI, 0.0],
            WoodSpecies::Oak,
        )
        .unwrap();
        let action = RailPlacementAction::Seed {
            position: hit.position,
            expected: 3,
            signature: rails::seed_signature(&plan),
        };
        let mut pending = Pending::new(1, action, &plan, WoodSpecies::Oak, None).unwrap();
        pending.request = Some(41);
        pending.accepted = true;
        pending.started = Instant::now() - Duration::from_secs(31);
        let mut app = App::new();
        app.insert_resource(chunks)
            .insert_resource(journal)
            .insert_resource(PlacementPreview {
                rail_pending: Some(pending),
                rail_waiting: true,
                ..Default::default()
            })
            .add_message::<RailPlacementSent>()
            .add_message::<ConfirmedEdit>()
            .add_systems(Update, confirm);
        app.update();
        assert!(!app.world().resource::<PlacementPreview>().rail_waiting);
        assert!(
            app.world()
                .resource::<PlacementPreview>()
                .rail_start
                .is_none()
        );
        app.world_mut().write_message(ConfirmedEdit {
            request: 41,
            action: action.edit_action(),
            outcome: EditOutcome::Applied,
        });
        app.update();
        assert!(
            app.world()
                .resource::<PlacementPreview>()
                .rail_start
                .is_none()
        );
    }
}
