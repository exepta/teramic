use bevy::prelude::*;
use terra_player::{TargetWorld, WorldTarget};
use terra_world::edits::EditJournal;

fn block_bounds(position: [i64; 3], material: Option<u16>, journal: &EditJournal) -> (Vec3, Vec3) {
    let mut minimum = position;
    let mut maximum = position;
    // Paired machines own one footprint. Connected chests remain two objects.
    if matches!(material, Some(28 | 99))
        && let Some(peer) = journal.object_state(position).and_then(|state| state.peer)
    {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(peer[axis]);
            maximum[axis] = maximum[axis].max(peer[axis]);
        }
    }
    let (mut center, mut scale) = (
        Vec3::from_array(std::array::from_fn(|axis| {
            (minimum[axis] as f64 + maximum[axis] as f64 + 1.0) as f32 * 0.5
        })),
        Vec3::from_array(std::array::from_fn(|axis| {
            (maximum[axis] - minimum[axis] + 1) as f32 + 0.005
        })),
    );
    if material == Some(terra_core::crusher::MATERIAL) {
        center.y += 0.115;
        scale.y += 0.23;
    }
    (center, scale)
}
fn show_outline(material: u16) -> bool {
    !matches!(
        terra_voxel::MaterialId::from_raw(material),
        Some(
            terra_voxel::MaterialId::SolidWater
                | terra_voxel::MaterialId::FluidWater
                | terra_voxel::MaterialId::Lava
        )
    )
}
fn show_prop_outline(kind: terra_world::props::PropKind, growth: Option<f32>) -> bool {
    kind.crop().is_none() || growth.is_none_or(|progress| progress >= 1.0)
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_selection(
    mut gizmos: Gizmos,
    target: Res<TargetWorld>,
    time: Res<Time>,
    chunks: Res<terra_world::streaming::ChunkStore>,
    journal: Res<EditJournal>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    mining: Option<Res<terra_player::interaction::MiningVisual>>,
    weather: Option<Res<crate::weather::WeatherVisual>>,
    growth: Option<Res<crate::props::PropGrowthStatus>>,
    chests: Option<Res<crate::chests::Chests>>,
    lids: Query<&crate::chests::Lid>,
) {
    if let Some(target) = target.0 {
        if let WorldTarget::Block(hit) = target
            && chunks
                .block_at(hit.position)
                .is_some_and(|id| !show_outline(id))
        {
            return;
        }
        let (center, scale) = match target {
            WorldTarget::Block(hit) => {
                if chunks.block_at(hit.position) == Some(29)
                    && chests
                        .as_ref()
                        .is_some_and(|chests| chests.hides_outline(hit.position, &journal, &lids))
                {
                    return;
                }
                // The isolated model already loses real surface pieces. Its
                // intact selection cage must not redraw those missing parts.
                if chunks
                    .block_at(hit.position)
                    .is_some_and(|material| !properties.shape(material).is_empty())
                    && mining.as_ref().is_some_and(|mining| {
                        mining.progress > 0.0
                            && matches!(
                                mining.target,
                                Some(terra_network::protocol::session::MiningTarget::Block { position, .. })
                                    if position == hit.position
                                        || journal.object_state(hit.position)
                                            .and_then(|state| state.peer) == Some(position)
                            )
                    })
                {
                    return;
                }
                if chunks.block_at(hit.position) == Some(terra_core::crusher::MATERIAL) {
                    let (center, scale) =
                        block_bounds(hit.position, Some(terra_core::crusher::MATERIAL), &journal);
                    gizmos.cube(
                        Transform::from_translation(center).with_scale(scale),
                        Color::srgb(0.08, 0.10, 0.08),
                    );
                    return;
                }
                if terra_world::interaction_shapes::visit_selection_boxes(
                    &chunks,
                    &journal,
                    &properties,
                    hit.position,
                    |mut bounds| {
                        if chunks.block_at(hit.position) == Some(29) {
                            for axis in 0..3 {
                                bounds.minimum[axis] =
                                    bounds.minimum[axis].max(hit.position[axis] as f64);
                                bounds.maximum[axis] =
                                    bounds.maximum[axis].min(hit.position[axis] as f64 + 1.0);
                            }
                        }
                        if (0..3).any(|axis| bounds.minimum[axis] >= bounds.maximum[axis]) {
                            return;
                        }
                        let low = Vec3::from_array(bounds.minimum.map(|v| v as f32));
                        let high = Vec3::from_array(bounds.maximum.map(|v| v as f32));
                        gizmos.cube(
                            Transform::from_translation((low + high) * 0.5)
                                .with_scale(high - low + Vec3::splat(0.002)),
                            Color::srgb(0.08, 0.10, 0.08),
                        );
                    },
                ) {
                    return;
                }
                block_bounds(hit.position, chunks.block_at(hit.position), &journal)
            }
            WorldTarget::Prop(hit) => {
                // A full-stage wire cage would expose geometry still dissolving in.
                if !show_prop_outline(
                    hit.prop.kind,
                    growth.as_ref().and_then(|g| g.progress(hit.prop.id)),
                ) {
                    return;
                }
                hit.prop.visit_weather_edges(
                    time.elapsed_secs_wrapped(),
                    crate::weather::foliage_wind(weather.as_deref()).to_array(),
                    |[a, b]| {
                        gizmos.line(
                            Vec3::from_array(a.map(|v| v as f32)),
                            Vec3::from_array(b.map(|v| v as f32)),
                            Color::srgb(0.08, 0.10, 0.08),
                        )
                    },
                );
                return;
            }
        };
        gizmos.cube(
            Transform::from_translation(center).with_scale(scale),
            Color::srgb(0.08, 0.10, 0.08),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_stage_fades_do_not_draw_a_wire_cage_of_the_future_geometry() {
        use terra_world::props::PropKind;
        for kind in [PropKind::Wheat, PropKind::Corn, PropKind::Lettuce] {
            for progress in [0.0, 0.25, 0.5, 0.99] {
                assert!(!show_prop_outline(kind, Some(progress)));
            }
            assert!(show_prop_outline(kind, None));
            assert!(show_prop_outline(kind, Some(1.0)));
        }
        for kind in [PropKind::Grass, PropKind::Bush, PropKind::SpiderWeb] {
            assert!(show_prop_outline(kind, Some(0.5)));
        }
    }

    #[test]
    fn crusher_outline_is_one_pair_with_its_hopper_height_from_either_half() {
        for facing in 0..4 {
            let anchor = [-16, 15, -1];
            let state = terra_world::edits::BlockObjectState {
                facing,
                ..default()
            };
            let peer = std::array::from_fn(|axis| anchor[axis] + state.right()[axis]);
            let mut journal = EditJournal::default();
            for (cell, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
                journal.record(cell, terra_core::crusher::MATERIAL).unwrap();
                journal
                    .set_object_state(
                        cell,
                        Some(terra_world::edits::BlockObjectState {
                            peer: Some(other),
                            secondary,
                            ..state
                        }),
                    )
                    .unwrap();
            }
            let a = block_bounds(anchor, Some(terra_core::crusher::MATERIAL), &journal);
            let b = block_bounds(peer, Some(terra_core::crusher::MATERIAL), &journal);
            assert_eq!(a, b);
            assert!((a.1.y - 1.235).abs() < 0.00001);
            assert!((a.0.y - (anchor[1] as f32 + 0.615)).abs() < 0.00001);
            assert_eq!(
                a.1[state.right().iter().position(|&v| v != 0).unwrap()],
                2.005
            );
        }
    }

    #[test]
    fn workbench_footprint_is_identical_from_either_cell_but_chests_stay_separate() {
        for facing in 0..4 {
            let position = [-16, 5, -1];
            let state = terra_world::edits::BlockObjectState {
                facing,
                ..default()
            };
            let peer = std::array::from_fn(|axis| position[axis] + state.right()[axis]);
            let mut journal = EditJournal::default();
            for (cell, other, secondary) in [(position, peer, false), (peer, position, true)] {
                journal.record(cell, 28).unwrap();
                journal
                    .set_object_state(
                        cell,
                        Some(terra_world::edits::BlockObjectState {
                            peer: Some(other),
                            secondary,
                            ..state
                        }),
                    )
                    .unwrap();
            }
            let (center, scale) = block_bounds(position, Some(28), &journal);
            assert_eq!((center, scale), block_bounds(peer, Some(28), &journal));
            assert_eq!(
                scale[state.right().iter().position(|&v| v != 0).unwrap()],
                2.005
            );
            assert_eq!(
                block_bounds(position, Some(29), &journal).1,
                Vec3::splat(1.005)
            );
            assert_ne!(
                block_bounds(position, Some(29), &journal).0,
                block_bounds(peer, Some(29), &journal).0
            );
        }
    }
    #[test]
    fn water_and_lava_hide_the_outline_but_ordinary_blocks_show_it() {
        for id in 1..=terra_voxel::MaterialId::LAST {
            assert_eq!(super::show_outline(id), !matches!(id, 14 | 27 | 613));
        }
    }
}
