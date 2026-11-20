//! Cosmetic canopy droplets: a fixed opaque instancing pool, no fluid simulation.
mod motion;
mod sources;
#[cfg(test)]
mod tests;

use bevy::{light::NotShadowCaster, prelude::*};
use terra_player::TerraCamera;
use terra_voxel::SectionPos;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

const MAX_DROPS: usize = 192;
const RANGE: f32 = 42.0;
const MAX_SECTIONS: usize = 32;
const LEAVES: u16 = 406;

#[derive(Resource, Default, Debug)]
pub struct BloodDripStatus {
    pub candidates: usize,
    pub falling: usize,
    pub splashes: usize,
    pub pool_capacity: usize,
    pub scanned_sections: usize,
    pub refreshes: u64,
    pub animation_time: f32,
}

#[derive(Clone, Copy, Debug)]
struct Source {
    cell: [i64; 3],
    origin: Vec3,
    impact: f32,
    seed: u64,
    distance: f32,
}

#[derive(Resource)]
struct Cache {
    next: f32,
    sources: Vec<Source>,
    sections: Vec<(f32, SectionPos)>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            next: 0.0,
            sources: Vec::with_capacity(MAX_DROPS),
            sections: Vec::with_capacity(MAX_SECTIONS),
        }
    }
}

#[derive(Component)]
struct Drop(usize);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Cache>()
        .init_resource::<BloodDripStatus>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (sources::refresh, animate)
                .chain()
                .after(terra_player::PlayerSet::Movement),
        );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut status: ResMut<BloodDripStatus>,
) {
    // Identical mesh/material permits Bevy's ordinary automatic instancing.
    // Opaque depth testing avoids alpha sorting and canopy-sized overdraw.
    let mesh = meshes.add(Sphere::new(1.0).mesh().uv(12, 8));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.13, 0.23),
        perceptual_roughness: 0.24,
        reflectance: 0.45,
        ..default()
    });
    for index in 0..MAX_DROPS {
        commands.spawn((
            Name::new("Blood canopy droplet"),
            Drop(index),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }
    status.pool_capacity = MAX_DROPS;
}

#[allow(clippy::too_many_arguments)]
fn animate(
    time: Res<Time>,
    access: Res<WorldAccess>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    cache: Res<Cache>,
    cameras: Query<&Transform, (With<TerraCamera>, Without<Drop>)>,
    mut drops: Query<(&Drop, &mut Transform, &mut Visibility), Without<TerraCamera>>,
    mut status: ResMut<BloodDripStatus>,
) {
    status.falling = 0;
    status.splashes = 0;
    status.animation_time = time.elapsed_secs();
    let eye = cameras.iter().next().map(|pose| pose.translation);
    for (drop, mut transform, mut visibility) in &mut drops {
        let pose = cache
            .sources
            .get(drop.0)
            .filter(|source| {
                access.ready()
                    && sources::block(&chunks, &journal, source.cell) == Some(LEAVES)
                    && eye.is_some_and(|eye| eye.distance_squared(source.origin) < RANGE * RANGE)
            })
            .and_then(|source| motion::pose(*source, time.elapsed_secs()));
        if let Some((pose, splash)) = pose {
            *transform = pose;
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
            if splash {
                status.splashes += 1;
            } else {
                status.falling += 1;
            }
        } else if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
}
