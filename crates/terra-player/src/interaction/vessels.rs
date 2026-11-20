use bevy::prelude::*;
use terra_core::{ItemKind, ToolKind};

/// Local presentation only. A completed hold requests the existing server transaction.
#[derive(Resource, Default, Debug)]
pub struct VesselMotion {
    pub item: Option<ItemKind>,
    pub fill: f32,
    pub filling: bool,
    pub pouring: bool,
    pub progress: f32,
    pub target: Option<[i64; 3]>,
    held: Option<FillHold>,
}

#[derive(Debug, PartialEq)]
struct FillHold {
    item: ItemKind,
    position: [i64; 3],
    slot: u8,
    revision: u64,
    initial_fill: f32,
    elapsed: f32,
    sent: bool,
}

impl VesselMotion {
    pub(crate) fn begin_frame(
        &mut self,
        item: Option<ItemKind>,
        _water: (u8, u8),
        enabled: bool,
        held: bool,
    ) {
        self.filling = false;
        self.pouring = false;
        self.progress = 0.0;
        self.target = None;
        self.item = item.filter(|item| is_vessel(*item));
        self.fill =
            match self.item {
                Some(
                    ItemKind::WaterBottle
                    | ItemKind::Tool(
                        ToolKind::WaterBucket | ToolKind::TarBucket | ToolKind::BileBucket,
                    ),
                ) => 1.0,
                Some(item)
                    if item
                        .tool_kind()
                        .is_some_and(|tool| tool.watering_capacity() > 0) =>
                {
                    f32::from(item.watering_units())
                        / f32::from(item.tool_kind().unwrap().watering_capacity())
                }
                _ => 0.0,
            }
            .clamp(0.0, 1.0);
        if !enabled
            || !held
            || self
                .held
                .as_ref()
                .is_some_and(|h| Some(h.item) != self.item)
        {
            self.held = None;
        }
        if self.held.as_ref().is_some_and(|hold| {
            hold.sent
                && item
                    .and_then(ItemKind::tool_kind)
                    .is_some_and(|tool| tool.watering_capacity() > 0)
                && self.fill > hold.initial_fill
                && self.fill < 1.0
        }) {
            // A small faucet buffer can refill a large can in several checked portions.
            self.held = None;
        }
    }

    pub(crate) fn fill_from(
        &mut self,
        item: ItemKind,
        position: [i64; 3],
        slot: u8,
        revision: u64,
        delta: f32,
    ) -> bool {
        let duration = match item {
            ItemKind::GlassBottle => 1.0,
            ItemKind::Tool(ToolKind::Bucket) => 1.6,
            item if item
                .tool_kind()
                .is_some_and(|tool| tool.watering_capacity() > 0) =>
            {
                1.25
            }
            _ => return false,
        };
        if self
            .held
            .as_ref()
            .is_none_or(|h| h.item != item || h.position != position || h.slot != slot)
        {
            self.held = Some(FillHold {
                item,
                position,
                slot,
                revision,
                initial_fill: self.fill,
                elapsed: 0.0,
                sent: false,
            });
        }
        let hold = self.held.as_mut().expect("fill hold just initialized");
        if revision != hold.revision && hold.sent {
            // The next empty bottle in a stack must not display the already stored water.
            return false;
        }
        // An inventory update must not complete a hold against a different stack.
        if revision != hold.revision && !hold.sent {
            self.held = None;
            return false;
        }
        hold.elapsed += delta.clamp(0.0, 0.1);
        self.progress = (hold.elapsed / duration).min(1.0);
        self.fill = hold.initial_fill + (1.0 - hold.initial_fill) * self.progress;
        self.target = Some(position);
        self.filling = !hold.sent;
        if self.progress >= 1.0 && !hold.sent {
            hold.sent = true;
            self.filling = false;
            return true;
        }
        false
    }

    pub(crate) fn cancel_fill(&mut self) {
        self.held = None;
    }

    pub(crate) fn pour(&mut self, position: [i64; 3]) {
        self.cancel_fill();
        self.pouring = true;
        self.target = Some(position);
    }
}

fn is_vessel(item: ItemKind) -> bool {
    matches!(item, ItemKind::GlassBottle | ItemKind::WaterBottle)
        || matches!(
            item,
            ItemKind::Tool(
                ToolKind::Bucket
                    | ToolKind::WaterBucket
                    | ToolKind::TarBucket
                    | ToolKind::BileBucket
            )
        )
        || item
            .tool_kind()
            .is_some_and(|tool| tool.watering_capacity() > 0)
}

pub(crate) fn water_target(
    chunks: &terra_world::streaming::ChunkStore,
    journal: &terra_world::edits::EditJournal,
    session: Option<&terra_network::client::ClientSession>,
    origin: [f64; 3],
    direction: [f64; 3],
    units: u8,
) -> Option<[i64; 3]> {
    // Pressure proofs stay on the authority. The client only previews its checked snapshot.
    if let Some(hit) = terra_voxel::blocks::raycast_blocks(origin, direction, 8.0, |position| {
        chunks.block_at(position)
    }) && chunks.block_at(hit.position) == Some(terra_core::plumbing::FAUCET)
    {
        return session
            .is_some_and(|session| {
                session.plumbing.iter().any(|state| {
                    state.position == hit.position
                        && state.open
                        && state.supplied
                        && state.amount >= u16::from(units)
                })
            })
            .then_some(hit.position);
    }
    terra_world::farming::water_target(chunks, journal, origin, direction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_faucet_preview_uses_checked_supply_and_never_fabricates_private_pressure() {
        use terra_network::{client::ClientSession, protocol::session::PlumbingVisual};
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::{
            edits::EditJournal,
            streaming::{ChunkStore, LoadedSection},
        };
        let owner = SectionPos::default();
        let position = [4, 4, 4];
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block([4, 4, 4], terra_core::plumbing::FAUCET)
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let journal = EditJournal::default();
        let mut session = ClientSession::default();
        session.plumbing.push(PlumbingVisual {
            intake: None,
            position,
            amount: 8,
            capacity: 8,
            open: true,
            flowing: false,
            supplied: true,
            pump_rpm: 0.0,
            basin: None,
        });
        let origin = [4.5, 4.5, 8.5];
        let direction = [0.0, 0.0, -1.0];
        assert_eq!(
            water_target(&chunks, &journal, Some(&session), origin, direction, 8),
            Some(position)
        );
        assert!(journal.plumbing_at(position).is_none());
        session.plumbing[0].amount = 7;
        assert_eq!(
            water_target(&chunks, &journal, Some(&session), origin, direction, 8),
            None
        );
        assert_eq!(
            water_target(&chunks, &journal, Some(&session), origin, direction, 1),
            Some(position)
        );
        session.plumbing[0].supplied = false;
        assert_eq!(
            water_target(&chunks, &journal, Some(&session), origin, direction, 1),
            None
        );
        session.plumbing[0].supplied = true;
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([4, 4, 6], 3)
            .unwrap();
        assert_eq!(
            water_target(&chunks, &journal, Some(&session), origin, direction, 1),
            None
        );
        assert_eq!(
            water_target(&chunks, &journal, None, origin, direction, 1),
            None
        );
    }

    #[test]
    fn fill_is_progressive_and_requests_only_once_per_hold() {
        let item = ItemKind::GlassBottle;
        let mut motion = VesselMotion::default();
        let mut requests = 0;
        for step in 0..40 {
            motion.begin_frame(Some(item), (0, 0), true, true);
            requests += usize::from(motion.fill_from(item, [1, 2, 3], 0, 7, 0.05));
            if step < 10 {
                assert!(motion.filling);
                assert!(motion.fill > 0.0 && motion.fill < 0.6);
                assert_eq!(requests, 0);
            }
        }
        assert_eq!(requests, 1);
        assert_eq!(motion.fill, 1.0);
        motion.begin_frame(Some(item), (0, 0), true, false);
        assert_eq!(motion.fill, 0.0);
        assert!(motion.held.is_none());
    }

    #[test]
    fn release_target_change_ui_and_inventory_change_cancel_unfinished_fill() {
        let item = ItemKind::Tool(ToolKind::Bucket);
        for interrupt in 0..4 {
            let mut motion = VesselMotion::default();
            motion.begin_frame(Some(item), (0, 0), true, true);
            for _ in 0..8 {
                assert!(!motion.fill_from(item, [1, 2, 3], 0, 7, 0.1));
            }
            match interrupt {
                0 => motion.begin_frame(Some(item), (0, 0), true, false),
                1 => motion.begin_frame(Some(item), (0, 0), false, true),
                2 => motion.cancel_fill(),
                _ => {
                    assert!(!motion.fill_from(item, [1, 2, 3], 0, 8, 0.1));
                }
            }
            assert!(motion.held.is_none());
            assert!(!motion.fill_from(item, [-17, 2, 3], 0, 8, 0.1));
            assert!(motion.progress < 0.1);
        }
    }

    #[test]
    fn can_uses_real_reserve_and_only_valid_pouring_sets_the_stream() {
        let mut motion = VesselMotion::default();
        let item = ItemKind::Tool(ToolKind::IronWateringCan)
            .with_watering_units(6)
            .unwrap();
        motion.begin_frame(Some(item), (6, 12), true, true);
        assert_eq!(motion.fill, 0.5);
        assert!(!motion.pouring);
        motion.pour([3, 0, 4]);
        assert!(motion.pouring);
        assert_eq!(motion.target, Some([3, 0, 4]));
        motion.begin_frame(
            Some(item.with_watering_units(5).unwrap()),
            (5, 12),
            true,
            false,
        );
        assert!(!motion.pouring);
        assert!(motion.fill < 0.5);
    }

    #[test]
    fn holding_a_can_continues_after_a_confirmed_partial_faucet_refill() {
        let item = ItemKind::Tool(ToolKind::IronWateringCan);
        let mut motion = VesselMotion::default();
        for _ in 0..20 {
            motion.begin_frame(Some(item), (0, 12), true, true);
            motion.fill_from(item, [1, 2, 3], 0, 7, 0.1);
        }
        assert!(motion.held.as_ref().unwrap().sent);
        let partial = item.with_watering_units(8).unwrap();
        motion.begin_frame(Some(partial), (8, 12), true, true);
        assert!(motion.held.is_none());
        assert!(!motion.fill_from(partial, [1, 2, 3], 0, 8, 0.1));
        assert!(motion.filling);
        assert!(motion.fill > 8.0 / 12.0);
    }
}
