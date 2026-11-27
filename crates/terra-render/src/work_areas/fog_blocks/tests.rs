use super::*;
use field::{Position, radial_weight, source_height};
use terra_voxel::{BlockSection, MaterialId, SectionPos, global_to_section};
use terra_world::streaming::LoadedSection;

fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for z in -1..=1 {
            for y in 0..=1 {
                let p = SectionPos::new(x, y, z);
                let mut data = BlockSection::empty(p).unwrap();
                if y == 0 {
                    for x in 0..16 {
                        for z in 0..16 {
                            data.set_block([x, 0, z], 1).unwrap();
                        }
                    }
                }
                chunks.loaded.insert(
                    p,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
            }
        }
    }
    (chunks, EditJournal::default())
}
fn put(chunks: &mut ChunkStore, p: Position, raw: u16) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, raw)
        .unwrap();
}
#[test]
fn actual_placed_sources_are_found_removed_and_buried_under_real_floor_blocks() {
    let (mut chunks, journal) = fixture();
    let mut index = Index::default();
    index.refresh(Vec3::ZERO, &chunks, &journal);
    assert!(index.nearby(Vec3::ZERO).is_empty());
    put(&mut chunks, [-2, 1, -2], terra_core::fog_blocks::MATERIAL);
    put(&mut chunks, [-2, 2, -2], MaterialId::Grass as u16);
    index.refresh(Vec3::ZERO, &chunks, &journal);
    assert_eq!(index.nearby(Vec3::ZERO), vec![[-2, 1, -2]]);
    assert_eq!(source_height(&chunks, &journal, [-2, 1, -2]), Some(3.045));
    let field = Field::sample(Vec3::ZERO, &index.nearby(Vec3::ZERO), &chunks, &journal);
    assert!(field.occupied() > 0);
    assert_eq!(field.sources[0].y, 3.045);
    let i = 19 + 19 * SIZE;
    assert_eq!(field.heights[i], 3.045);
    put(&mut chunks, [-2, 1, -2], 0);
    index.refresh(Vec3::ZERO, &chunks, &journal);
    assert!(index.nearby(Vec3::ZERO).is_empty());
    // Unloaded source chunks cannot keep emitting a stale cached circle.
    put(&mut chunks, [1, 1, 1], terra_core::fog_blocks::MATERIAL);
    index.refresh(Vec3::ZERO, &chunks, &journal);
    chunks.loaded.remove(&SectionPos::new(0, 0, 0));
    index.refresh(Vec3::ZERO, &chunks, &journal);
    assert!(index.nearby(Vec3::ZERO).is_empty());
}
#[test]
fn circle_union_never_extends_past_eight_blocks_or_stacks_density() {
    let source = Vec4::new(-0.5, 4.0, -0.5, 1.0);
    for angle in 0..360 {
        let a = (angle as f32).to_radians();
        let p = Vec2::new(a.cos(), a.sin());
        assert_eq!(
            radial_weight(source.x + p.x * 8.001, source.z + p.y * 8.001, &[source]),
            0.0
        );
        let inner = radial_weight(source.x + p.x * 7.0, source.z + p.y * 7.0, &[source]);
        assert!(inner > 0.0 && inner < 1.0);
    }
    assert_eq!(radial_weight(-0.5, -0.5, &[source; MAX_SOURCES]), 1.0);
}
#[test]
fn source_budget_and_height_encoding_are_bounded_without_generating_chunks() {
    let (mut chunks, journal) = fixture();
    for x in 0..24 {
        put(&mut chunks, [x, 1, 0], terra_core::fog_blocks::MATERIAL);
    }
    let loaded = chunks.loaded.len();
    let mut index = Index::default();
    index.refresh(Vec3::ZERO, &chunks, &journal);
    let nearby = index.nearby(Vec3::ZERO);
    assert_eq!(nearby.len(), MAX_SOURCES);
    let field = Field::sample(Vec3::ZERO, &nearby, &chunks, &journal);
    let (pixels, base, span) = field.pixels();
    assert_eq!(pixels.len(), SIZE * SIZE * 4);
    for (i, p) in pixels.chunks_exact(4).enumerate() {
        let height = base + (f32::from(p[0]) * 256.0 + f32::from(p[1])) / 65535.0 * span;
        assert!((height - field.heights[i]).abs() < 0.001);
    }
    assert_eq!(chunks.loaded.len(), loaded);
    assert!(index.nearby(Vec3::splat(1000.0)).is_empty());
}
#[test]
fn local_worlds_render_one_volume_and_unready_replicas_hide_even_cached_fog() {
    let (mut chunks, journal) = fixture();
    put(&mut chunks, [0, 1, 0], terra_core::fog_blocks::MATERIAL);
    let mut app = App::new();
    app.init_resource::<Time<Real>>()
        .init_resource::<State>()
        .init_resource::<FogBlockStatus>()
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(WorldAccess::Local)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<FogMaterial>>()
        .add_systems(Update, update);
    app.world_mut()
        .spawn((TerraCamera, GlobalTransform::default()));
    let volume = app.world_mut().spawn((FogVolume, Visibility::Hidden)).id();
    app.update();
    let status = app.world().resource::<FogBlockStatus>();
    assert_eq!(status.sources, 1);
    assert_eq!(status.volumes, 1);
    assert_eq!(status.radius, 8.0);
    assert_eq!(
        app.world().get::<Visibility>(volume),
        Some(&Visibility::Inherited)
    );
    app.world_mut()
        .insert_resource(WorldAccess::Replica { ready: false });
    app.update();
    assert_eq!(app.world().resource::<FogBlockStatus>().volumes, 0);
    assert_eq!(
        app.world().get::<Visibility>(volume),
        Some(&Visibility::Hidden)
    );
}
#[test]
fn fog_shader_parses_with_and_without_depth_and_has_strict_circle_bound() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::TranslationUnit,
    };
    let shader = include_str!("../../../../../assets/shaders/terra_fog_block.wesl");
    for depth in [false, true] {
        let mut module = shader.parse::<TranslationUnit>().unwrap();
        let mut features = Features::new();
        features.set("DEPTH_PREPASS", depth);
        condcomp(&mut module, &features).unwrap();
    }
    assert!(shader.contains("smoothstep(6.2,8.0,distance)"));
    assert!(shader.contains("i < 6u"));
}

#[test]
fn a_roof_above_the_emitter_does_not_steal_the_indoor_ground_layer() {
    let (mut chunks, journal) = fixture();
    put(&mut chunks, [0, 1, 0], terra_core::fog_blocks::MATERIAL);
    for x in -8..=8 {
        for z in -8..=8 {
            put(&mut chunks, [x, 2, z], 1);
            put(&mut chunks, [x, 9, z], 3);
        }
    }
    let field = Field::sample(Vec3::new(0.0, 4.0, 0.0), &[[0, 1, 0]], &chunks, &journal);
    assert!(field.occupied() > 0);
    for (i, &weight) in field.weights.iter().enumerate() {
        if weight > 0.0 {
            assert!(
                (field.heights[i] - 3.045).abs() < 0.001,
                "fog jumped onto the roof"
            );
        }
    }
}

#[test]
fn moving_source_window_recovers_distant_additions_and_removals_on_return() {
    let (mut chunks, mut journal) = fixture();
    let left = [-32, 1, 0];
    let right = [192, 1, 0];
    let east = Vec3::new(160.5, 1.0, 0.0);
    // Exceed the source-window volume, as in a streamed world with distant terrain.
    let occupied = [left, right].map(|cell| global_to_section(cell).0);
    for section in occupied
        .into_iter()
        .chain((1000..1300).map(|x| SectionPos::new(x, 0, 0)))
    {
        chunks
            .loaded
            .entry(section)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            });
    }
    put(&mut chunks, left, terra_core::fog_blocks::MATERIAL);
    put(&mut chunks, right, 0);
    let mut index = Index::default();
    index.refresh(Vec3::ZERO, &chunks, &journal);
    assert_eq!(index.nearby(Vec3::ZERO), [left]);
    journal
        .record(right, terra_core::fog_blocks::MATERIAL)
        .unwrap();
    index.refresh(east, &chunks, &journal);
    assert_eq!(index.nearby(east), [right]);
    journal.record(left, 0).unwrap();
    index.refresh(Vec3::ZERO, &chunks, &journal);
    assert!(index.nearby(Vec3::ZERO).is_empty());
    index.refresh(east, &chunks, &journal);
    assert_eq!(index.nearby(east), [right]);
}
