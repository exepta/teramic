mod contact_shadows;
mod geometry;
mod growth;
mod jobs;
mod shapes;
mod static_layout;
#[cfg(test)]
mod tests;
pub(crate) mod wind;

pub(crate) use geometry::add_box;
pub use jobs::PropRenderStatus;
pub(crate) use shapes::template;
pub use wind::{PropGrowthStatus, WindProp};

pub(crate) fn install(app: &mut bevy::prelude::App) {
    wind::install(app);
    jobs::install(app);
    contact_shadows::install(app);
}
