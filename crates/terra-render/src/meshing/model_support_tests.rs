use super::*;
use crate::blocks::{BlockTemplate, PrimitiveTemplate, TemplateSet, definitions::BlockRenderKind};
use terra_voxel::{BlockSection, global_to_section};
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

const OWNER: [i64; 3] = [-16, 0, -16];
const BELOW: [i64; 3] = [-16, -1, -16];

fn loaded(position: [i64; 3], material: u16) -> LoadedSection {
    let (owner, local) = global_to_section(position);
    let mut data = BlockSection::empty(owner).unwrap();
    data.set_block(local, material).unwrap();
    LoadedSection {
        data,
        mesh: None,
        mesh_epoch: 0,
        water_epoch: 0,
        water: None,
        props: Vec::new(),
    }
}

fn templates(material: u16) -> TemplateSet {
    let mut templates = TemplateSet::default();
    templates.logistics_support_materials.extend([1, 2]);
    templates.blocks.insert(
        material,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::CustomMesh,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: (0..3)
                .map(|material| PrimitiveTemplate {
                    material,
                    positions: vec![
                        [0.08, -0.375, 0.08],
                        [0.20, -0.375, 0.08],
                        [0.08, -0.305, 0.08],
                    ],
                    normals: vec![[0.0, 0.0, 1.0]; 3],
                    tangents: Vec::new(),
                    uvs: vec![[0.0; 2]; 3],
                    indices: vec![0, 1, 2],
                })
                .collect(),
        },
    );
    templates
}

fn fixture(material: u16, floor: u16) -> (App, TemplateSet) {
    let owner = global_to_section(OWNER).0;
    let mut journal = EditJournal::default();
    for (cell, id) in [(OWNER, material), (BELOW, floor)] {
        journal.record(cell, id).unwrap();
        if cell == OWNER || id == 148 {
            journal
                .set_object_state(cell, Some(BlockObjectState::default()))
                .unwrap();
        }
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(owner, loaded(OWNER, material));
    let templates = templates(material);
    let models = capture_models(&chunks.loaded[&owner].data, &templates, &chunks, &journal)
        .unwrap()
        .unwrap();
    let mut cache = GeometryCache::default();
    cache.insert_model_test(
        owner,
        journal.mesh_epoch(owner),
        BlockMesh::default(),
        models.as_ref().clone(),
    );
    let mut app = App::new();
    app.insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(cache)
        .insert_resource(terra_core::GameConfig::default())
        .insert_resource(WorldTerrain(
            terra_world::TerraGenerator::new(Default::default()).unwrap(),
        ))
        .add_message::<SectionPublished>()
        .add_message::<SectionRemoved>();
    refresh(&mut app);
    (app, templates)
}

fn refresh(app: &mut App) {
    app.world_mut()
        .run_system_cached(refresh_stream_exposure)
        .unwrap();
}

fn snapshot(app: &App) -> Arc<super::super::custom::ModelSnapshot> {
    app.world().resource::<GeometryCache>().0[&global_to_section(OWNER).0]
        .models
        .clone()
        .unwrap()
}

fn epoch(app: &App) -> u64 {
    app.world().resource::<GeometryCache>().appearance_epoch(
        global_to_section(OWNER).0,
        app.world().resource::<EditJournal>(),
    )
}

fn assert_support(app: &App, templates: &TemplateSet, supported: bool) {
    let batches = super::super::custom::compose(
        &BlockMesh::default(),
        &snapshot(app),
        templates,
        global_to_section(OWNER).0,
        &RenderOptions {
            water_level: None,
            detail_level: DetailLevel::Near,
            terrain: None,
            exposure: None,
            conditions: Default::default(),
            tints: Default::default(),
        },
    )
    .unwrap();
    assert_eq!(batches.len(), if supported { 3 } else { 1 });
    let chunks = app.world().resource::<ChunkStore>();
    assert_eq!(
        terra_world::funnels::supported_with(
            OWNER,
            app.world().resource::<EditJournal>(),
            |cell| chunks.block_at(cell),
        ),
        supported
    );
}

#[test]
fn real_job_capture_rejects_persisted_but_unloaded_floor_and_belt_support() {
    for material in [153, 157] {
        for floor in [3, 148] {
            let (app, templates) = fixture(material, floor);
            assert_eq!(
                app.world().resource::<EditJournal>().block(BELOW),
                Some(floor)
            );
            assert_support(&app, &templates, false);
        }
    }
}

#[test]
fn real_stream_events_refresh_cached_and_hidden_funnel_owners_without_mutating_old_worker_snapshots()
 {
    let owner = global_to_section(OWNER).0;
    let below = global_to_section(BELOW).0;
    for material in [153, 157] {
        for floor in [3, 148] {
            let (mut app, templates) = fixture(material, floor);
            let old = snapshot(&app);
            let old_epoch = epoch(&app);
            let geometry = app.world().resource::<GeometryCache>().0[&owner]
                .geometry
                .clone();
            // No RenderedSection is present: portal-hidden owners need the same invalidation.
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .insert(below, loaded(BELOW, floor));
            app.world_mut()
                .write_message(SectionPublished { position: below });
            refresh(&mut app);
            assert_support(&app, &templates, true);
            assert_ne!(
                epoch(&app),
                old_epoch,
                "already-running workers must become stale"
            );
            assert!(!Arc::ptr_eq(&old, &snapshot(&app)));
            assert!(
                app.world()
                    .resource::<GeometryCache>()
                    .5
                    .changed
                    .contains(&owner)
            );
            assert!(Arc::ptr_eq(
                &geometry,
                &app.world().resource::<GeometryCache>().0[&owner].geometry
            ));

            let current = snapshot(&app);
            let current_epoch = epoch(&app);
            app.world_mut()
                .write_message(SectionPublished { position: below });
            refresh(&mut app);
            assert_eq!(
                epoch(&app),
                current_epoch,
                "unchanged support never rebuilds a model"
            );
            assert!(Arc::ptr_eq(&current, &snapshot(&app)));

            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .remove(&below);
            app.world_mut()
                .write_message(SectionRemoved { position: below });
            refresh(&mut app);
            assert_support(&app, &templates, false);
            assert_ne!(epoch(&app), current_epoch);

            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .insert(below, loaded(BELOW, floor));
            app.world_mut()
                .write_message(SectionPublished { position: below });
            refresh(&mut app);
            assert_support(&app, &templates, true);
            let current_epoch = epoch(&app);
            app.world_mut()
                .resource_mut::<EditJournal>()
                .record(BELOW, 0)
                .unwrap();
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .insert(below, loaded(BELOW, 0));
            app.world_mut()
                .write_message(SectionPublished { position: below });
            refresh(&mut app);
            assert_support(&app, &templates, false);
            assert_ne!(
                epoch(&app),
                current_epoch,
                "loaded support revisions also invalidate the owner"
            );

            app.world_mut()
                .resource_mut::<GeometryCache>()
                .forget(owner);
            assert!(
                app.world().resource::<GeometryCache>().7.is_empty(),
                "unloaded owners release support revisions"
            );
        }
    }
}

#[test]
fn real_job_capture_and_stream_events_keep_cauldron_piers_in_contact_across_vertical_section_seams()
{
    for source in [[0, 15, 0], [-16, -1, -16]] {
        let kettle = [source[0], source[1] + 1, source[2]];
        let owner = global_to_section(source).0;
        let upper = global_to_section(kettle).0;
        let mut journal = EditJournal::default();
        journal.record(source, 162).unwrap();
        journal.record(kettle, 161).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(owner, loaded(source, 162));
        let mut templates = templates(162);
        templates.campfire_support_material = Some(1);
        let template = templates.blocks.get_mut(&162).unwrap();
        template.model.truncate(1);
        template.model[0].positions = vec![[0.4, 0.2, 0.4], [0.6, 0.2, 0.4], [0.4, 0.2, 0.6]];
        let mut preview_neighbors = [Some(0); 6];
        preview_neighbors[3] = Some(161);
        let standalone = super::super::custom::model_geometry(
            162,
            Default::default(),
            Default::default(),
            preview_neighbors,
            [0; 6],
            &templates,
        )
        .unwrap();
        assert_eq!(
            standalone.len(),
            1,
            "the standalone campfire stays unchanged"
        );
        let models = capture_models(&chunks.loaded[&owner].data, &templates, &chunks, &journal)
            .unwrap()
            .unwrap();
        let mut cache = GeometryCache::default();
        cache.insert_model_test(
            owner,
            journal.mesh_epoch(owner),
            BlockMesh::default(),
            models.as_ref().clone(),
        );
        let mut app = App::new();
        app.insert_resource(chunks)
            .insert_resource(journal)
            .insert_resource(cache)
            .insert_resource(terra_core::GameConfig::default())
            .insert_resource(WorldTerrain(
                terra_world::TerraGenerator::new(Default::default()).unwrap(),
            ))
            .add_message::<SectionPublished>()
            .add_message::<SectionRemoved>();
        refresh(&mut app);
        let current = |app: &App| {
            app.world().resource::<GeometryCache>().0[&owner]
                .models
                .clone()
                .unwrap()
        };
        let current_epoch = |app: &App| {
            app.world()
                .resource::<GeometryCache>()
                .appearance_epoch(owner, app.world().resource::<EditJournal>())
        };
        let assert_geometry = |snapshot: &super::super::custom::ModelSnapshot, supported| {
            for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let batches = super::super::custom::compose(
                    &BlockMesh::default(),
                    snapshot,
                    &templates,
                    owner,
                    &RenderOptions {
                        water_level: None,
                        detail_level,
                        terrain: None,
                        exposure: None,
                        conditions: Default::default(),
                        tints: Default::default(),
                    },
                )
                .unwrap();
                assert_eq!(batches.len(), if supported { 2 } else { 1 });
                if supported {
                    let piers = batches.iter().find(|batch| batch.material == 1).unwrap();
                    assert_eq!(piers.positions.len(), 8 * 24);
                    assert_eq!(piers.indices.len(), 8 * 36);
                    assert_eq!(piers.tangents.len(), piers.positions.len());
                    assert_eq!(piers.uvs.len(), piers.positions.len());
                    let top = piers
                        .positions
                        .iter()
                        .map(|point| point[1])
                        .fold(f32::NEG_INFINITY, f32::max);
                    assert_eq!(top, 16.0, "the feet contact the next cell exactly");
                    assert!(piers.positions.iter().all(|point| point[1] >= 15.0));
                    for triangle in piers.indices.chunks_exact(3) {
                        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                            .map(|index| Vec3::from_array(piers.positions[index as usize]));
                        assert!(
                            (b - a)
                                .cross(c - a)
                                .dot(Vec3::from_array(piers.normals[triangle[0] as usize]))
                                > 0.0
                        );
                    }
                }
            }
        };
        // Journal metadata alone does not render unloaded support.
        assert_geometry(&current(&app), false);
        let before = current(&app);
        let old_epoch = current_epoch(&app);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(upper, loaded(kettle, 161));
        app.world_mut()
            .write_message(SectionPublished { position: upper });
        refresh(&mut app);
        assert_geometry(&current(&app), true);
        assert_geometry(&before, false);
        assert_ne!(current_epoch(&app), old_epoch);
        let unchanged = current(&app);
        let old_epoch = current_epoch(&app);
        app.world_mut()
            .write_message(SectionPublished { position: upper });
        refresh(&mut app);
        assert_eq!(current_epoch(&app), old_epoch);
        assert!(Arc::ptr_eq(&unchanged, &current(&app)));
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .remove(&upper);
        app.world_mut()
            .write_message(SectionRemoved { position: upper });
        refresh(&mut app);
        assert_geometry(&current(&app), false);
        assert_ne!(current_epoch(&app), old_epoch);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(upper, loaded(kettle, 161));
        app.world_mut()
            .write_message(SectionPublished { position: upper });
        refresh(&mut app);
        assert_geometry(&current(&app), true);
        let old_epoch = current_epoch(&app);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(kettle, 0)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(upper, loaded(kettle, 0));
        app.world_mut()
            .write_message(SectionPublished { position: upper });
        refresh(&mut app);
        assert_geometry(&current(&app), false);
        assert_ne!(current_epoch(&app), old_epoch);
    }
}
