use super::*;

#[test]
fn local_worlds_sample_atmosphere_without_a_client_session_and_pending_replicas_hide_it() {
    let mut app = App::new();
    app.init_resource::<Time<Real>>()
        .init_resource::<Banks>()
        .init_resource::<SpiritMistStatus>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<BiomeMistMaterial>>()
        .insert_resource(WorldAccess::Local)
        .add_systems(Update, update);
    app.world_mut()
        .spawn((TerraCamera, GlobalTransform::default()));
    let mist = app
        .world_mut()
        .spawn((AmbientMist, Visibility::Hidden))
        .id();
    app.update();
    assert!(app.world().resource::<SpiritMistStatus>().sampled_columns > 0);
    assert!(
        !app.world()
            .contains_resource::<terra_network::client::ClientSession>()
    );
    // A replica awaiting its world must discard even cached banks immediately.
    {
        let mut banks = app.world_mut().resource_mut::<Banks>();
        banks.next_selection = 100.;
        banks.field.weights[0] = 1.;
    }
    app.world_mut()
        .insert_resource(WorldAccess::Replica { ready: false });
    app.update();
    let status = app.world().resource::<SpiritMistStatus>();
    assert_eq!(status.sampled_columns, 0);
    assert_eq!(status.banks, 0);
    assert!(app.world().resource::<Banks>().field.occupied() == 0);
    assert_eq!(
        app.world().get::<Visibility>(mist),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .insert_resource(WorldAccess::Replica { ready: true });
    app.world_mut().resource_mut::<Banks>().next_selection = 0.;
    app.update();
    assert!(app.world().resource::<SpiritMistStatus>().sampled_columns > 0);
}

#[test]
fn one_volume_covers_the_whole_loaded_grove_with_bounded_field_cost() {
    let field = Field::sample(Vec3::new(-1., 6., -1.), |_, _| Some((1.045, 1.)));
    assert_eq!(field.occupied(), 121);
    assert_eq!(field.origin, [-88, -88]);
    let (center, extent) = field.bounds();
    assert_eq!(center.xz(), Vec2::splat(-8.));
    assert_eq!(extent, Vec3::new(80., 4., 80.));
    let (pixels, base, span) = field.pixels();
    assert_eq!(pixels.len(), 484);
    assert_eq!(base, 1.045);
    assert_eq!(span, 1.);
}
#[test]
fn height_encoding_preserves_slopes_and_missing_cells_do_not_pull_ground_down() {
    let field = Field::sample(Vec3::ZERO, |x, z| {
        (x < 30).then_some((40. + x as f32 * 0.13 + z as f32 * 0.04, 0.9))
    });
    let (pixels, base, span) = field.pixels();
    for (i, pixel) in pixels.chunks_exact(4).enumerate() {
        let decoded = base + (f32::from(pixel[0]) * 256. + f32::from(pixel[1])) / 65535. * span;
        assert!((decoded - field.heights[i]).abs() < 0.001);
        assert!(decoded > 20.);
    }
    let empty = Field::sample(Vec3::ZERO, |_, _| None);
    assert_eq!(empty.occupied(), 0);
    assert_eq!(
        Field::sample(Vec3::NAN, |_, _| Some((1., 1.))).occupied(),
        0
    );
}
#[test]
fn world_fixed_field_and_density_are_continuous_without_lamp_sources() {
    let sample = |x, z| Some(((x + z) as f32 * 0.02, 0.4));
    assert_eq!(
        Field::sample(Vec3::new(-1., 2., -1.), sample),
        Field::sample(Vec3::new(-0.99, 2., -0.99), sample)
    );
    let a = Field::sample(Vec3::ZERO, |_, _| Some((1., 0.3)));
    let b = Field::sample(Vec3::ZERO, |_, _| Some((1., 0.301)));
    assert!(b.weights[0] > a.weights[0] && b.weights[0] - a.weights[0] < 0.003);
    assert_eq!(
        Field::sample(Vec3::ZERO, |_, _| Some((1., 0.))).occupied(),
        0
    );
    let (mut chunks, journal) = super::super::tests::terrain();
    chunks
        .loaded
        .get_mut(&terra_voxel::SectionPos::new(0, 0, 0))
        .unwrap()
        .data
        .set_block([2, 2, 2], 323)
        .unwrap();
    assert!(surface_sample(&chunks, &journal, 2, 2).is_none());
}
#[test]
fn single_volume_shader_parses_with_and_without_prepass() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::TranslationUnit,
    };
    for depth in [false, true] {
        let mut module = include_str!("../../../../../assets/shaders/terra_biome_mist.wesl")
            .parse::<TranslationUnit>()
            .unwrap();
        let mut features = Features::new();
        features.set("DEPTH_PREPASS", depth);
        condcomp(&mut module, &features).unwrap();
        let source = module.to_string();
        assert!(source.contains("i < 6u"));
        assert_eq!(source.contains("prepass_depth(input.position"), depth);
        assert!(source.contains("first_hit"));
    }
}
