use super::*;
use terra_core::trees::SaplingPlacementError;
use terra_world::props::{PropId, WorldProp};

fn command(app: &App, peer: &Peer, support: [i64; 3]) -> EditCommand {
    EditCommand {
        request: 1,
        revision: app
            .world()
            .resource::<EditJournal>()
            .prop_epoch(terra_voxel::global_to_section(support).0),
        look: aim(
            app,
            peer,
            [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.,
                support[2] as f64 + 0.5,
            ],
        ),
        source: None,
        action: EditAction::PlaceProp {
            support,
            kind: ResourcePropKind::OakSapling,
        },
    }
}
fn apply_test(app: &mut App, peer: &Peer, command: EditCommand) -> EditOutcome {
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = state.get_mut(app.world_mut()).unwrap();
    apply(command, peer, &mut world, false)
}
fn ground_cover(app: &mut App, support: [i64; 3], saved: bool) -> WorldProp {
    let prop = WorldProp {
        id: PropId(0xf202_0020),
        kind: PropKind::Grass,
        position: [
            support[0] as f64 + 0.5,
            support[1] as f64 + 1.,
            support[2] as f64 + 0.5,
        ],
        rotation: 0.,
        scale: 1.,
        variant: 0,
        section: terra_voxel::global_to_section(support).0,
        removed: false,
    };
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&prop.section)
        .unwrap()
        .props
        .push(prop);
    if saved {
        assert!(
            app.world_mut()
                .resource_mut::<EditJournal>()
                .place_prop(prop)
                .unwrap()
        );
    }
    prop
}

#[test]
fn actual_survival_placement_accepts_natural_grass_shade_near_trunks_and_missing_far_sections() {
    let (mut app, peer, support) = fixture(ResourcePropKind::OakSapling, false);
    let grass = ground_cover(&mut app, support, false);
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        chunks
            .loaded
            .remove(&grass.section.offset([0, 1, 0]).unwrap());
        let section = chunks.loaded.get_mut(&grass.section).unwrap();
        section.data.set_block([4, 8, 4], 3).unwrap();
        section.data.set_block([7, 4, 4], 6).unwrap();
    }
    assert!(!terra_world::saplings::light(
        app.world().resource::<ChunkStore>(),
        [4, 4, 4],
        true
    ));
    let command = command(&app, &peer, support);
    assert_eq!(apply_test(&mut app, &peer, command), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    assert!(journal.prop_removed(grass.section, grass.id));
    assert_eq!(
        journal.gameplay().inventories[&peer.uuid]
            .count(ItemKind::Prop(ResourcePropKind::OakSapling)),
        1
    );
    assert_eq!(journal.gameplay().trees.trees.len(), 1);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([7, 4, 4]),
        Some(6)
    );
}

#[test]
fn rejected_sapling_requests_report_specific_reasons_and_preserve_inventory_and_plants() {
    for error in [
        SaplingPlacementError::Soil,
        SaplingPlacementError::Water,
        SaplingPlacementError::Occupied,
        SaplingPlacementError::Limit,
        SaplingPlacementError::MissingItem,
    ] {
        let (mut app, peer, support) = fixture(ResourcePropKind::OakSapling, false);
        let owner = terra_voxel::global_to_section(support).0;
        let grass = ground_cover(&mut app, support, error == SaplingPlacementError::Occupied);
        match error {
            SaplingPlacementError::Soil => {
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block([4, 3, 4], 3)
                    .unwrap();
            }
            SaplingPlacementError::Water => {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_fluid(
                        [4, 4, 4],
                        terra_world::water::FluidCell {
                            amount: 8,
                            source: true,
                            flow: [0; 2],
                        },
                    )
                    .unwrap();
            }
            SaplingPlacementError::Limit => {
                let delta = terra_world::edits::SectionDelta {
                    position: owner,
                    prop_epoch: u64::MAX,
                    ..Default::default()
                };
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .import_delta(delta)
                    .unwrap();
            }
            SaplingPlacementError::MissingItem => {
                assert_eq!(
                    peer.game_mode(app.world().resource::<GameConfig>()),
                    GameMode::Survival
                );
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                let bag = journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap();
                let item = ItemKind::Prop(ResourcePropKind::OakSapling);
                assert!(bag.take_from(0, item));
                assert!(bag.take_from(0, item));
                assert_eq!(bag.count(item), 0);
                assert!(bag.slot(0).is_none());
            }
            _ => {}
        }
        let inventory =
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
        let before = app.world().resource::<EditJournal>().section_delta(owner);
        let command = command(&app, &peer, support);
        assert_eq!(
            apply_test(&mut app, &peer, command),
            EditOutcome::Planting(error)
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.gameplay().inventories[&peer.uuid], inventory);
        assert_eq!(journal.section_delta(owner), before);
        assert!(!journal.prop_removed(owner, grass.id));
        assert!(journal.gameplay().trees.trees.is_empty());
    }
}
