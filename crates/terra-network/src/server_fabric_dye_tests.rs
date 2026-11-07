use super::{
    rgb_filter_tests::{apply, fixture, look, place},
    *,
};
use terra_core::{ItemKind, ItemStack, WoodSpecies, beds::DyeColor};
use terra_world::{conveyors, edits::BlockObjectState};

fn dye(app: &mut App, peer: &Peer, color: DyeColor, count: u16) {
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .get_mut(&peer.uuid)
        .unwrap()
        .creative_set(
            0,
            ItemStack {
                item: ItemKind::Dye(color),
                count,
            },
        );
}

fn aim(app: &mut App, peer: &mut Peer, position: [i64; 3], height: f32) {
    let center = Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, height, 0.5);
    look(app, peer, center + Vec3::new(0.0, 2.5, 0.4), center);
}

fn request(app: &App, peer: &Peer, position: [i64; 3], color: DyeColor, belt: bool) -> EditCommand {
    let journal = app.world().resource::<EditJournal>();
    EditCommand {
        request: 1,
        revision: journal.revision(terra_voxel::global_to_section(position).0),
        look: peer.look,
        source: Some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
        action: if belt {
            EditAction::DyeConveyor { position, color }
        } else {
            EditAction::DyeFabric { position, color }
        },
    }
}

#[test]
fn rgb_dye_authority_recolors_all_fabric_shapes_once_and_preserves_pose_and_mesh_epochs() {
    for material in [103, 104, 105, 106] {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        place(
            &mut app,
            position,
            material,
            matches!(material, 104 | 105).then_some(BlockObjectState::default()),
        );
        aim(
            &mut app,
            &mut peer,
            position,
            if material == 106 { 0.06 } else { 0.25 },
        );
        let original_pose = app.world().resource::<EditJournal>().object_state(position);
        let owner = terra_voxel::global_to_section(position).0;
        let mesh = app.world().resource::<EditJournal>().mesh_epoch(owner);
        for rgb in [[17, 95, 213], [221, 37, 103]] {
            let color = DyeColor::Rgb(rgb);
            dye(&mut app, &peer, color, 3);
            let command = request(&app, &peer, position, color, false);
            assert_eq!(
                apply(&mut app, &peer, command, false),
                EditOutcome::Applied,
                "material={material}"
            );
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.tint_at(position), Some(rgb));
            assert_eq!(journal.object_state(position), original_pose);
            assert_eq!(journal.mesh_epoch(owner), mesh);
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(position),
                Some(material)
            );
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid].count(ItemKind::Dye(color)),
                2
            );
            assert_eq!(
                apply(&mut app, &peer, command, false),
                EditOutcome::Conflict
            );
            let repeated = request(&app, &peer, position, color, false);
            assert_eq!(
                apply(&mut app, &peer, repeated, false),
                EditOutcome::Conflict
            );
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                    .count(ItemKind::Dye(color)),
                2
            );
        }
        assert!(app.world().resource::<EditJournal>().gameplay().valid());
    }
}

#[test]
fn fabric_dye_rejects_stale_false_unheld_missing_sources_and_blocked_rays_without_mutation() {
    for fault in [
        "stale_source",
        "stale_section",
        "wrong_color",
        "unheld",
        "missing",
        "wall",
    ] {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        place(&mut app, position, 103, None);
        let color = DyeColor::Rgb([17, 95, 213]);
        dye(&mut app, &peer, color, 3);
        aim(&mut app, &mut peer, position, 0.5);
        if fault == "wall" {
            place(&mut app, [-1, 5, -1], 3, None);
        }
        let mut command = request(&app, &peer, position, color, false);
        match fault {
            "stale_source" => command.source.as_mut().unwrap().revision += 1,
            "stale_section" => command.revision += 1,
            "wrong_color" => {
                command.action = EditAction::DyeFabric {
                    position,
                    color: DyeColor::Rgb([221, 37, 103]),
                }
            }
            "unheld" => peer.held_slot = Some(1),
            "missing" => command.source = None,
            _ => {}
        }
        let owner = terra_voxel::global_to_section(position).0;
        let journal = app.world().resource::<EditJournal>();
        let before = journal.section_delta(owner);
        let gameplay = journal.gameplay().clone();
        let block_revision = app.world().resource::<ChunkStore>().loaded[&owner]
            .data
            .revision();
        assert_ne!(
            apply(&mut app, &peer, command, false),
            EditOutcome::Applied,
            "{fault}"
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.section_delta(owner), before, "{fault}");
        assert_eq!(journal.gameplay(), &gameplay, "{fault}");
        assert_eq!(
            app.world().resource::<ChunkStore>().loaded[&owner]
                .data
                .revision(),
            block_revision,
            "{fault}"
        );
    }
}

#[test]
fn rgb_belt_dye_with_unchanged_default_facing_commits_for_legacy_and_owned_spans() {
    for linked in [false, true] {
        let (mut app, mut peer) = fixture();
        let target = [-1, 4, -1];
        let end = [-1, 4, 1];
        let cells = if linked {
            let start = [-1, 4, -3];
            for position in [start, end] {
                place(&mut app, position, 144, Some(BlockObjectState::default()));
            }
            app.world_mut()
                .resource_scope(|world, mut journal: Mut<EditJournal>| {
                    let mut chunks = world.resource_mut::<ChunkStore>();
                    let layout = conveyors::layout(
                        &chunks,
                        &journal,
                        start,
                        end,
                        ItemKind::for_wood(148, WoodSpecies::Cherry),
                    )
                    .unwrap();
                    assert!(conveyors::commit(&mut chunks, &mut journal, &layout).unwrap());
                    conveyors::span_at(&journal, target)
                        .unwrap()
                        .cells()
                        .filter(|position| journal.block(*position) == Some(148))
                        .collect::<Vec<_>>()
                })
        } else {
            place(
                &mut app,
                target,
                148,
                Some(BlockObjectState {
                    wood: WoodSpecies::Cherry,
                    ..Default::default()
                }),
            );
            vec![target]
        };
        let poses: Vec<_> = cells
            .iter()
            .map(|position| {
                (
                    *position,
                    app.world()
                        .resource::<EditJournal>()
                        .object_state(*position),
                )
            })
            .collect();
        let first_color = DyeColor::Rgb([17, 95, 213]);
        dye(&mut app, &peer, first_color, 3);
        aim(&mut app, &mut peer, target, 0.625);
        for fault in ["stale_source", "stale_section", "wrong_color", "wall"] {
            let blocker = [target[0], target[1] + 1, target[2]];
            if fault == "wall" {
                place(&mut app, blocker, 3, None);
            }
            let mut command = request(&app, &peer, target, first_color, true);
            match fault {
                "stale_source" => command.source.as_mut().unwrap().revision += 1,
                "stale_section" => command.revision += 1,
                "wrong_color" => {
                    command.action = EditAction::DyeConveyor {
                        position: target,
                        color: DyeColor::Rgb([221, 37, 103]),
                    }
                }
                _ => {}
            }
            let journal = app.world().resource::<EditJournal>();
            let before: Vec<_> = app
                .world()
                .resource::<ChunkStore>()
                .loaded
                .iter()
                .map(|(owner, section)| {
                    (
                        *owner,
                        journal.section_delta(*owner),
                        section.data.revision(),
                    )
                })
                .collect();
            let gameplay = journal.gameplay().clone();
            assert_ne!(
                apply(&mut app, &peer, command, false),
                EditOutcome::Applied,
                "linked={linked} {fault}"
            );
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.gameplay(), &gameplay);
            for (owner, delta, blocks) in before {
                assert_eq!(journal.section_delta(owner), delta);
                assert_eq!(
                    app.world().resource::<ChunkStore>().loaded[&owner]
                        .data
                        .revision(),
                    blocks
                );
            }
            if fault == "wall" {
                place(&mut app, blocker, 0, None);
            }
        }
        for (phase, rgb) in [[17, 95, 213], [221, 37, 103]].into_iter().enumerate() {
            let color = DyeColor::Rgb(rgb);
            let hit = if linked && phase == 1 { end } else { target };
            aim(
                &mut app,
                &mut peer,
                hit,
                if hit == end { 0.5 } else { 0.625 },
            );
            dye(&mut app, &peer, color, 3);
            let command = request(&app, &peer, hit, color, true);
            assert_eq!(
                apply(&mut app, &peer, command, false),
                EditOutcome::Applied,
                "linked={linked} phase={phase}"
            );
            let journal = app.world().resource::<EditJournal>();
            for (position, pose) in &poses {
                assert_eq!(journal.object_state(*position), *pose);
                assert_eq!(journal.tint_at(*position), Some(rgb));
                assert_eq!(terra_core::kinetic::belt_facing(pose.unwrap().facing), 0);
            }
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid].count(ItemKind::Dye(color)),
                2
            );
            assert_ne!(apply(&mut app, &peer, command, false), EditOutcome::Applied);
            let repeat = request(&app, &peer, hit, color, true);
            assert_ne!(apply(&mut app, &peer, repeat, false), EditOutcome::Applied);
            assert_eq!(
                app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
                    .count(ItemKind::Dye(color)),
                2
            );
        }
        assert!(app.world().resource::<EditJournal>().gameplay().valid());
    }
}
