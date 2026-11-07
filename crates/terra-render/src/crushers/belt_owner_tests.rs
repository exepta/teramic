use super::*;
use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
use terra_core::{ItemKind, WoodSpecies};
use terra_world::{edits::BeltOwner, streaming::LoadedSection};

#[test]
fn shaft_controllers_keep_the_real_rotor_and_independent_belt_wood_rgb_direction_and_phase() {
    let start = [-17, 2, -1];
    let position = [-16, 2, -1];
    let end = [-15, 2, -1];
    let mut journal = EditJournal::default();
    let mut chunks = ChunkStore::default();
    for (cell, facing, wood, peer) in [
        (start, 1, WoodSpecies::Birch, position),
        (position, 3, WoodSpecies::Spruce, end),
        (end, 1, WoodSpecies::Oak, start),
    ] {
        journal.record(cell, 144).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    facing,
                    wood,
                    peer: Some(peer),
                    ..default()
                }),
            )
            .unwrap();
        let (section, local) = terra_voxel::global_to_section(cell);
        chunks
            .loaded
            .entry(section)
            .or_insert_with(|| LoadedSection {
                data: terra_voxel::BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, 144)
            .unwrap();
    }
    let item = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Cherry,
        color: [71, 93, 184],
    };
    journal
        .set_belt_owner(position, Some(BeltOwner { start, end, item }))
        .unwrap();
    let span = terra_world::conveyors::span_at(&journal, position).unwrap();
    assert_eq!(span.wood, WoodSpecies::Cherry);
    let object = journal.object_state(position).unwrap();
    let primitive = |material, y| PrimitiveTemplate {
        material,
        positions: vec![[0.0, y, 0.4], [1.0, y, 0.4], [1.0, y, 0.6]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        uvs: vec![[0.0; 2]; 3],
        tangents: Vec::new(),
        indices: vec![0, 2, 1],
    };
    let mut templates = TemplateSet::default();
    templates.wood_materials.insert(0);
    templates.conveyor_ribbon_materials.insert(1);
    templates.crusher_moving_materials.extend([(0, 5), (1, 7)]);
    for (id, slot) in [(144, 0), (148, 1)] {
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [slot; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(slot, if id == 144 { 0.5 } else { 0.625 })],
            },
        );
    }
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = BlockAssets::default();
    assets.materials = (0..2)
        .map(|_| materials.add(StandardMaterial::default()))
        .collect();
    assets.templates = Some(std::sync::Arc::new(templates));
    let mut app = App::new();
    app.insert_resource(Time::<()>::default())
        .insert_resource(journal)
        .insert_resource(chunks)
        .insert_resource(assets)
        .insert_resource(materials)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ribbon_tint::Material>>()
        .init_resource::<Machines>()
        .init_resource::<CrusherRenderStatus>()
        .add_systems(Update, synchronize);
    app.update();
    let root = app.world().resource::<Machines>().placed[&position].0;
    assert_eq!(app.world().resource::<CrusherRenderStatus>().belt_spans, 1);
    assert_eq!(
        app.world_mut()
            .query::<&belts::Surface>()
            .iter(app.world())
            .count(),
        1
    );
    let surface = app.world().get::<belts::Surface>(root).unwrap();
    assert_eq!(
        surface.flow_sign,
        -span.flow_sign(),
        "shaft RPM is expressed on its own opposite axis"
    );
    let rotor = app
        .world_mut()
        .query::<(Entity, &Rotor)>()
        .iter(app.world())
        .find(|(_, rotor)| rotor.root == root && rotor.index == 5)
        .map(|(entity, _)| entity)
        .unwrap();
    let frame = app.world().get::<ChildOf>(rotor).unwrap().parent();
    assert_ne!(frame, root);
    let pose = app.world().get::<Transform>(root).unwrap().compute_affine()
        * app
            .world()
            .get::<Transform>(frame)
            .unwrap()
            .compute_affine();
    let expected = assembly_pose(position, 144, object).compute_affine();
    assert!(pose.translation.abs_diff_eq(expected.translation, 1e-6));
    assert!(
        pose.transform_vector3(Vec3::X)
            .abs_diff_eq(expected.transform_vector3(Vec3::X), 1e-6)
    );
    let expected_tint = ribbon_tint::tag([71, 93, 184]).value;
    assert!(
        app.world_mut()
            .query::<&bevy::mesh::MeshTag>()
            .iter(app.world())
            .any(|tag| tag.value == expected_tint)
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .object_state(position)
            .unwrap()
            .wood,
        WoodSpecies::Spruce
    );
    let meshes = app.world().resource::<Assets<Mesh>>().len();
    {
        let mut drive = app.world_mut().get_mut::<Drive>(root).unwrap();
        drive.phase = 2.3;
        drive.speed = -1.7;
    }
    app.world_mut()
        .get_mut::<belts::Surface>(root)
        .unwrap()
        .offset = 0.37;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_belt_owner(
            position,
            Some(BeltOwner {
                start,
                end,
                item: ItemKind::TintedBlock {
                    material: 148,
                    wood: WoodSpecies::Cherry,
                    color: [230, 90, 31],
                },
            }),
        )
        .unwrap();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(110));
    app.update();
    let root = app.world().resource::<Machines>().placed[&position].0;
    let drive = app.world().get::<Drive>(root).unwrap();
    assert_eq!((drive.phase, drive.speed), (2.3, -1.7));
    assert_eq!(
        app.world().get::<belts::Surface>(root).unwrap().offset,
        0.37
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
}
