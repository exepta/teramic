use bevy::{camera::Exposure, prelude::*};
use terra_player::TerraCamera;
use terra_world::{edits::EditJournal, lava::level, streaming::WorldAccess};

#[derive(Component)]
struct LavaGlow(usize);

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        illuminate.in_set(crate::scene::caves::lights::ApplyLocalLights),
    );
}

fn illuminate(
    mut commands: Commands,
    time: Res<Time>,
    access: Res<WorldAccess>,
    sources: Res<crate::furnaces::molten::VolcanicSources>,
    journal: Res<EditJournal>,
    cameras: Query<&Exposure, With<TerraCamera>>,
    mut lamps: Query<(Entity, &LavaGlow, &mut PointLight, &mut Transform)>,
) {
    // Coalesce neighbouring regions so a broad lake illuminates several
    // stretches of bank. The same exposed cells drive the bubbling effects.
    let mut cells = [[0_i64; 3]; 6];
    let mut count = 0;
    let camera = cameras.iter().next();
    if access.ready() && camera.is_some() {
        for &cell in &sources.surfaces {
            let point = Vec3::from_array(cell.map(|v| v as f32));
            if cells[..count].iter().any(|&other| {
                point.distance_squared(Vec3::from_array(other.map(|v| v as f32))) < 49.
            }) {
                continue;
            }
            cells[count] = cell;
            count += 1;
            if count == cells.len() {
                break;
            }
        }
    }

    let gain = crate::scene::caves::lights::exposure_gain(camera.map_or(6., |c| c.ev100));
    let emitter = |cell: [i64; 3]| {
        let phase = time.elapsed_secs() * 1.9 + (cell[0] ^ cell[2]).rem_euclid(71) as f32;
        let depth = level(&journal, cell) as f32;
        let lift = Vec3::new(0.5, depth + 0.28, 0.5);
        (
            Vec3::from_array(cell.map(|v| v as f32)) + lift,
            (620. + 25. * phase.sin()) * (0.36 + depth * 0.64) * gain,
        )
    };
    let mut present = [false; 6];
    for (entity, slot, mut light, mut transform) in &mut lamps {
        if slot.0 >= count {
            commands.entity(entity).despawn();
            continue;
        }
        let (point, intensity) = emitter(cells[slot.0]);
        transform.translation = point;
        light.intensity = intensity;
        present[slot.0] = true;
    }
    for (slot, &cell) in cells[..count].iter().enumerate() {
        if !present[slot] {
            let (point, intensity) = emitter(cell);
            commands.spawn((
                Name::new("Molten surface glow"),
                LavaGlow(slot),
                Transform::from_translation(point),
                PointLight {
                    color: Color::srgb(1., 0.40, 0.12),
                    intensity,
                    range: 10.,
                    radius: 1.2,
                    shadow_maps_enabled: false,
                    ..default()
                },
            ));
        }
    }
}
