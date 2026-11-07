//! Scenery photographs use smooth sampling independently of pixel-art UI atlases.
use bevy::{image::ImageSampler, prelude::*};
pub(crate) const SOURCES: [&str; 8] = [
    "tilt-ui://assets/biome-choice-ocean.png",
    "tilt-ui://assets/biome-choice-plains.png",
    "tilt-ui://assets/biome-choice-forest.png",
    "tilt-ui://assets/biome-choice-desert.png",
    "tilt-ui://assets/biome-choice-highlands.png",
    "tilt-ui://assets/biome-choice-mushroom-park.png",
    "tilt-ui://assets/biome-choice-cliffted-desert.png",
    "tilt-ui://assets/biome-choice-cherry-forest.png",
];
pub(crate) fn install(app: &mut App) {
    app.add_systems(Update, synchronize);
}
fn synchronize(server: Option<Res<AssetServer>>, mut images: Option<ResMut<Assets<Image>>>) {
    let (Some(server), Some(images)) = (server, images.as_mut()) else {
        return;
    };
    let linear = ImageSampler::linear();
    for source in SOURCES {
        let Some(handle) = server.get_handle::<Image>(source) else {
            continue;
        };
        if images
            .get(&handle)
            .is_some_and(|image| image.sampler != linear)
            && let Some(mut image) = images.get_mut(&handle)
        {
            image.sampler = linear.clone();
        }
    }
}
