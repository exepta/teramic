use crate::{
    blocks::{BlockAssets, load_blocks, poll_blocks},
    meshing::{
        GeometryCache, RenderJobs, RenderStatus, RenderedSections, dispatch_meshes, upload_sections,
    },
    scene::setup_scene,
    selection::draw_selection,
};
use bevy::prelude::*;
use terra_player::PlayerSet;
use terra_world::streaming::StreamingSet;
pub struct TerraRenderPlugin;
impl Plugin for TerraRenderPlugin {
    fn build(&self, app: &mut App) {
        crate::preprocess_cache::install(app);
        crate::preprocess_buffers::install(app);
        crate::meshing::install_far_batches(app);
        crate::meshing::visibility::install(app);
        crate::diagnostics::install_cpu_schedules(app);
        crate::render_schedule::install(app);
        crate::mesh_buffers::install(app);
        crate::grouped_point_shadows::install(app);
        crate::draw_calls::install(app);
        crate::map_palette::install(app);
        crate::transitions::install(app);
        crate::scene::horizon::install(app);
        crate::scene::daylight::install(app);
        crate::scene::shadow_receivers::install(app);
        crate::scene::leaf_profiles::install(app);
        crate::scene::caves::install(app);
        crate::water::install(app);
        crate::liquid_motion::install(app);
        crate::particle_fluids::install(app);
        crate::props::install(app);
        crate::apple_fruit::install(app);
        crate::blood_drips::install(app);
        crate::players::install(app);
        crate::creatures::install(app);
        crate::weather::install(app);
        crate::foliage::install(app);
        crate::footprints::install(app);
        app.add_plugins(MaterialPlugin::<crate::PlacementOutlineMaterial>::default());
        crate::building::install(app);
        crate::furnaces::install(app);
        crate::meshing::lava::install_lights(app);
        crate::volcanoes::install(app);
        crate::plumbing::install(app);
        crate::crushers::install_materials(app);
        crate::crushers::install(app);
        crate::openings::install(app);
        crate::chests::install(app);
        crate::trains::install(app);
        crate::train_blueprint::install(app);
        crate::train_yards::install(app);
        crate::settlement_banners::install(app);
        crate::chunk_borders::install(app);
        crate::work_areas::install(app);
        crate::previews::install(app);
        app.insert_resource(ClearColor(Color::srgb(
            crate::scene::SURFACE_FOG.x,
            crate::scene::SURFACE_FOG.y,
            crate::scene::SURFACE_FOG.z,
        )))
        .insert_resource(GlobalAmbientLight {
            brightness: 240.0,
            ..default()
        })
        .init_resource::<RenderedSections>()
        .init_resource::<BlockAssets>()
        .init_resource::<RenderJobs>()
        .init_resource::<GeometryCache>()
        .init_resource::<RenderStatus>()
        .init_resource::<crate::MeshUploadStatus>()
        .add_systems(First, crate::MeshUploadStatus::reset)
        .add_systems(Startup, (load_blocks, setup_scene))
        .add_systems(Startup, crate::meshing::shadows::prepare_material)
        .add_systems(
            Update,
            (
                poll_blocks,
                crate::appearance::update_weather_materials,
                crate::foliage::prepare_materials,
                crate::meshing::refresh_stream_exposure,
                upload_sections,
                crate::meshing::shadows::release_disabled,
                dispatch_meshes,
            )
                .chain()
                .after(StreamingSet::Receive),
        )
        .add_systems(
            Update,
            draw_selection
                .after(PlayerSet::Targeting)
                .after(crate::chests::animate),
        )
        .add_systems(
            Update,
            crate::scene::update_view_distance.before(crate::water::environment::update),
        );
        app.add_systems(
            Update,
            crate::graphics::apply.after(crate::scene::update_view_distance),
        );
        crate::graphics::install(app);
        app.add_systems(PostUpdate, crate::graphics::reset_temporal_history);
    }
}
