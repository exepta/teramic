use super::*;
use crate::{
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp},
    streaming::LoadedSection,
};
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_voxel::BlockSection;

fn scene() -> (ChunkStore, WorldProp) {
    let owner = SectionPos::default();
    let mut data = BlockSection::empty(owner).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 3, z], 2).unwrap();
        }
    }
    let grass = WorldProp {
        id: PropId(9001),
        kind: PropKind::Grass,
        position: [4.5, 4., 4.5],
        rotation: 0.,
        scale: 2.,
        variant: 0,
        section: owner,
        removed: false,
    };
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![grass],
        },
    );
    (chunks, grass)
}

fn mesh_rays(prop: WorldProp, origin: [f64; 3]) -> Vec<[f64; 3]> {
    crate::props::visual::template(
        prop.kind,
        prop.template_variant(),
        crate::props::visual::PropDetail::Near,
        &crate::props::visual::PropPalette::default(),
    )
    .unwrap()
    .iter()
    .flat_map(|mesh| {
        mesh.indices.chunks_exact(3).map(move |triangle| {
            let target: [f64; 3] = std::array::from_fn(|axis| {
                prop.position[axis]
                    + triangle
                        .iter()
                        .map(|i| {
                            f64::from(mesh.positions[*i as usize][axis])
                                * f64::from(prop.visual_scale())
                                / 3.
                        })
                        .sum::<f64>()
            });
            let delta: [f64; 3] = std::array::from_fn(|axis| target[axis] - origin[axis]);
            let length = delta.into_iter().map(|v| v * v).sum::<f64>().sqrt();
            delta.map(|v| v / length)
        })
    })
    .collect()
}

#[test]
fn flexible_wind_envelopes_do_not_reject_explicit_block_removal() {
    let (chunks, grass) = scene();
    let journal = EditJournal::default();
    let properties = BlockPropertiesCatalog::default();
    let origin = [4.5, 7., 7.5];
    let direction = mesh_rays(grass, origin)
        .into_iter()
        .find(|direction| {
            let (block, selected) = pick_world_for_mining(
                &chunks,
                &journal,
                &properties,
                origin,
                *direction,
                PropPose::Swept,
            );
            block.is_some()
                && matches!(selected, Some(WorldTarget::Prop(hit)) if hit.prop.id == grass.id)
        })
        .expect("an actual blade of grass must cover a supported block");
    for pose in [PropPose::Rest, PropPose::Swept] {
        let (block, selected) =
            pick_world_for_block_mining(&chunks, &journal, &properties, origin, direction, pose);
        assert_eq!(selected, block.map(WorldTarget::Block));
        assert!(block.is_some());
    }
    assert!(matches!(
        pick_world_for_mining(&chunks, &journal, &properties, origin, direction, PropPose::Swept).1,
        Some(WorldTarget::Prop(hit)) if hit.prop.id == grass.id
    ));
}

#[test]
fn ignoring_flexible_cover_still_checks_the_next_rigid_prop() {
    let (mut chunks, grass) = scene();
    let stones = WorldProp {
        id: PropId(9002),
        kind: PropKind::SmallStones,
        scale: 1.5,
        ..grass
    };
    chunks
        .loaded
        .get_mut(&grass.section)
        .unwrap()
        .props
        .push(stones);
    let journal = EditJournal::default();
    let properties = BlockPropertiesCatalog::default();
    let origin = [4.5, 7., 7.5];
    let direction = mesh_rays(stones, origin)
        .into_iter()
        .find(|direction| {
            matches!(pick_world_for_mining(&chunks, &journal, &properties, origin, *direction, PropPose::Swept).1,
                Some(WorldTarget::Prop(hit)) if hit.prop.id == grass.id)
                && matches!(pick_world_for_block_mining(&chunks, &journal, &properties, origin, *direction, PropPose::Swept).1,
                    Some(WorldTarget::Prop(hit)) if hit.prop.id == stones.id)
        })
        .expect("grass and actual rigid stones must overlap this ray before the soil");
    let (_, selected) = pick_world_for_block_mining(
        &chunks,
        &journal,
        &properties,
        origin,
        direction,
        PropPose::Swept,
    );
    assert!(matches!(selected, Some(WorldTarget::Prop(hit)) if hit.prop.id == stones.id));
}
