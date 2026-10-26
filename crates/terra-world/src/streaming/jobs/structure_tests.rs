use super::*;
use crate::{
    PreparedColumn, TerraGenerator,
    generation::structures::{BuildingRole, StructureKind},
};
use std::time::{Duration, Instant};
use terra_core::{WorldConfig, WorldSeed};
use terra_voxel::{BlockSection, global_to_section};

fn mine_boundary() -> (
    SectionGenerator,
    Arc<PreparedColumn>,
    SectionPos,
    SectionPos,
) {
    let terrain = TerraGenerator::new(WorldConfig {
        seed: WorldSeed(1),
        structures_revision: 1,
        landscape_revision: 0,
        geology_revision: 0,
        ocean_revision: 0,
        base_height: 16.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        caves: false,
        trees: false,
        water: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mine = (-3..=2)
        .flat_map(|z| (-3..=2).map(move |x| [x, z]))
        .find_map(|region| {
            terrain
                .structure_site(region)
                .filter(|site| site.kind == StructureKind::Mine)
        })
        .expect("deterministic legacy mine fixture");
    let entrance = mine
        .buildings
        .iter()
        .find(|building| building.role == BuildingRole::Mine)
        .unwrap()
        .entrance;
    let stair = [entrance[0], entrance[1] - 1, entrance[2] + 1];
    let (owner, local) = global_to_section(stair);
    assert_eq!(local[1], 15);
    let sibling = owner.offset([0, 1, 0]).unwrap();
    let generator = SectionGenerator::new(terrain);
    let prepared = Arc::new(generator.prepare_column([owner.x, owner.z], false).unwrap());
    assert!(
        prepared.structure_objects(owner).iter().any(|object| {
            object.position == stair
                && terra_core::construction::is_stair(object.material)
                && object.state.is_some()
        }),
        "fixture must contain real oriented joinery in the sibling's one-cell halo"
    );
    (generator, prepared, owner, sibling)
}

fn receiver() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<PendingSections>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<WorldGeneration>()
        .add_message::<SectionPublished>()
        .add_message::<PreparedColumnChanged>()
        .insert_resource(WorldAccess::Server)
        .add_systems(Update, receive_jobs);
    app
}

fn publish(
    app: &mut App,
    generator: &SectionGenerator,
    prepared: Arc<PreparedColumn>,
    positions: &[SectionPos],
) {
    let first = positions[0];
    let output = ColumnOutput {
        coordinate: [first.x, first.z],
        remesh: false,
        sections: positions
            .iter()
            .map(|&position| {
                (
                    position,
                    0,
                    0,
                    generator
                        .build_prepared(position, 0, &Default::default(), &prepared)
                        .map_err(|e| e.to_string()),
                )
            })
            .collect(),
        prepared: Some(prepared),
    };
    app.world_mut().resource_mut::<PendingSections>().0.insert(
        first,
        SectionJob {
            positions: positions.to_vec(),
            task: AsyncComputeTaskPool::get().spawn(async move { output }),
        },
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.world().resource::<PendingSections>().0.is_empty() {
        app.update();
        assert!(
            Instant::now() < deadline,
            "completed structure output did not publish"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn generated_structure_siblings_publish_once_without_false_stale_jobs_in_either_order() {
    let (generator, prepared, owner, sibling) = mine_boundary();
    for positions in [[owner, sibling], [sibling, owner]] {
        let mut app = receiver();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .wanted
            .extend(positions);
        publish(&mut app, &generator, prepared.clone(), &positions);
        let chunks = app.world().resource::<ChunkStore>();
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            chunks.loaded.len(),
            2,
            "initial metadata cannot discard a completed sibling worker"
        );
        assert!(
            chunks.dirty.is_empty(),
            "fresh sibling results do not need a redundant remesh"
        );
        for position in positions {
            assert_eq!(journal.mesh_epoch(position), 0);
            assert_eq!(chunks.loaded[&position].mesh_epoch, 0);
            assert_eq!(
                chunks.loaded[&position].data.revision(),
                0,
                "data revisions retain their existing generation-snapshot contract"
            );
            for object in prepared.structure_objects(position) {
                assert_eq!(journal.block(object.position), Some(object.material));
                assert_eq!(journal.object_state(object.position), object.state);
                assert_eq!(chunks.block_at(object.position), Some(object.material));
            }
        }
        assert!(
            journal.revision(owner) > 0,
            "canonical markers still enter public terrain deltas"
        );
    }
}

#[test]
fn generated_joinery_coalesces_refreshes_of_existing_neighbors_and_republished_owners() {
    let (generator, prepared, owner, sibling) = mine_boundary();
    for republish_owner in [false, true] {
        let mut app = receiver();
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            chunks.wanted.extend([owner, sibling]);
            for position in [Some(sibling), republish_owner.then_some(owner)]
                .into_iter()
                .flatten()
            {
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: BlockSection::empty(position).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
        publish(&mut app, &generator, prepared.clone(), &[owner]);
        let chunks = app.world().resource::<ChunkStore>();
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            journal.mesh_epoch(sibling),
            1,
            "hundreds of generated objects share one loaded-neighbor refresh"
        );
        assert!(chunks.dirty.contains(&sibling));
        assert_eq!(journal.mesh_epoch(owner), u64::from(republish_owner));
        assert_eq!(
            chunks.dirty.contains(&owner),
            republish_owner,
            "an already-rendered owner must refresh too"
        );
        assert_eq!(chunks.loaded[&owner].mesh_epoch, 0);
    }
}
