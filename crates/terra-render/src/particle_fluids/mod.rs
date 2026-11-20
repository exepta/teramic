//! Near-field GPU position-based fluids; coarse world volumes retain authority.
mod api;
mod far;
mod flows;
mod gpu;
mod surface;
// The experimental camera-relative particle adapter is kept as a tested model.
// World presentation follows the authoritative flow meshes, not a second solver.
#[cfg(test)]
mod world;
pub use api::*;
use bevy::prelude::*;
pub(crate) use surface::{FluidMaterial, HydraulicMaterials, setup as setup_surface};

pub(crate) fn install(app: &mut App) {
    app.init_resource::<ParticleFluidInput>()
        .init_resource::<ParticleFluidStatus>()
        .init_resource::<FarFluidSections>()
        .init_resource::<RestFluidGrid>();
    flows::install(app);
    gpu::install(app);
    surface::install(app);
    far::install(app);
}

/// Both old-surface shaders and the reconstructed surface use this exact domain.
pub(crate) fn coverage_uniform(
    input: Option<&ParticleFluidInput>,
    status: Option<&ParticleFluidStatus>,
) -> Vec4 {
    match (input, status) {
        (Some(input), Some(status))
            if input.enabled
                && status.enabled
                && status.gpu_ready
                && status.error.is_none()
                && input.particles.iter().any(|s| s.kind < 4) =>
        {
            Vec3::from_array(input.origin).extend(1.0)
        }
        _ => Vec4::ZERO,
    }
}
