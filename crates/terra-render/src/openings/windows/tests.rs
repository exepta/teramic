use super::*;
use std::sync::Arc;
use terra_voxel::{BlockSection, SectionPos, global_to_section};
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

#[test]
fn connected_openings_reuse_templates_oppose_hinges_and_survive_owner_unload() {
    for material in [68, 65, 66, 67, 139] {
        let group = Group {
            anchor: [-1, 15, 8],
            material,
            facing: 0,
            width: 2,
            height: 2,
        };
        let primitive = |material| crate::blocks::PrimitiveTemplate {
            tangents: Vec::new(),
            positions: vec![[0.0; 3]; 3],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            material,
        };
        let mut templates = crate::blocks::TemplateSet::default();
        templates.blocks.insert(
            material,
            crate::blocks::BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1), primitive(2)],
            },
        );
        templates.hinge_materials.insert(1);
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = BlockAssets::default();
        assets.materials = vec![
            materials.add(StandardMaterial::default()),
            materials.add(StandardMaterial::default()),
            materials.add(StandardMaterial {
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        ];
        assets.templates = Some(Arc::new(templates));
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for p in group.cells() {
            let (owner, local) = global_to_section(p);
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
                .set_block(local, material)
                .unwrap();
            journal.record(p, material).unwrap();
            journal
                .set_object_state(
                    p,
                    Some(if terra_world::objects::is_door(material) {
                        let upper = p[1] != group.anchor[1];
                        BlockObjectState {
                            peer: Some([p[0], p[1] + if upper { -1 } else { 1 }, p[2]]),
                            secondary: upper,
                            ..default()
                        }
                    } else {
                        default()
                    }),
                )
                .unwrap();
        }
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(assets)
            .insert_resource(materials)
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(chunks)
            .insert_resource(journal);
        super::super::install(&mut app);
        app.update();
        let placed = app.world().resource::<Openings>();
        assert_eq!(placed.placed.len(), 1);
        assert_eq!(placed.placed[&group.anchor].leaves.len(), 2);
        let mesh_count = app.world().resource::<Assets<Mesh>>().len();
        assert!(mesh_count < 16);
        for (_, mesh) in app.world().resource::<Assets<Mesh>>().iter() {
            let count = mesh.count_vertices();
            assert!(mesh.indices().unwrap().iter().all(|i| i < count));
            let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
            else {
                panic!("positions");
            };
            assert!(positions.iter().flatten().all(|v| v.is_finite()));
        }
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .remove(&SectionPos::new(-1, 0, 0));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        assert_eq!(app.world().resource::<Openings>().placed.len(), 1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
        for p in group.cells() {
            let previous = app
                .world()
                .resource::<EditJournal>()
                .object_state(p)
                .unwrap();
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_object_state(
                    p,
                    Some(BlockObjectState {
                        facing: 4,
                        ..previous
                    }),
                )
                .unwrap();
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        let angles: Vec<_> = app
            .world_mut()
            .query::<&Leaf>()
            .iter(app.world())
            .map(|l| l.angle * l.fraction)
            .collect();
        assert_eq!(angles.len(), 2);
        assert!(angles[0] * angles[1] < 0.0);
        assert!(
            angles
                .iter()
                .all(|v| v.abs() > 0.0 && v.abs() < std::f32::consts::FRAC_PI_2)
        );
        app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        assert!(app.world().resource::<Openings>().placed.is_empty());
        assert_eq!(
            app.world_mut().query::<&Leaf>().iter(app.world()).count(),
            0
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
    }
}
