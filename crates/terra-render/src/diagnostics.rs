//! Optional native GPU profiling. Values are published in Bevy's DiagnosticsStore.
use bevy::{
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        diagnostic::RenderDiagnosticsPlugin,
        renderer::{RenderAdapterInfo, RenderDevice},
        settings::WgpuFeatures,
    },
};

pub struct TerraRenderDiagnosticsPlugin;

impl Plugin for TerraRenderDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenderDiagnosticsPlugin);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(RenderStartup, report_capabilities);
        }
    }
}

fn report_capabilities(device: Res<RenderDevice>, adapter: Res<RenderAdapterInfo>) {
    let features = device.features();
    info!(
        adapter = %adapter.name,
        backend = ?adapter.backend,
        timestamps = features.contains(WgpuFeatures::TIMESTAMP_QUERY),
        pass_timestamps = features.contains(WgpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES),
        encoder_timestamps = features.contains(WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS),
        pipeline_statistics = features.contains(WgpuFeatures::PIPELINE_STATISTICS_QUERY),
        "Render diagnostics enabled; GPU pass timings are milliseconds, not total frame latency; TERRA_DRAW_DIAGNOSTICS enables separate PBR draw API counters"
    );
}
