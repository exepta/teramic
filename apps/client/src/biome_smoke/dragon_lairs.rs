//! Native views of generated nests with their actual ecology guardian and untouched terrain.
use super::*;
use terra_core::creatures::{CreatureKind, CreaturePopulation};
use terra_network::client::{CreatureTrack, ReplicatedCreatures};
use terra_world::{creatures::Habitat, edits::EditJournal, generation::structures::StructureKind};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Dark,
    Earth,
    Water,
}

impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Dark => "dragon-lair-dark",
            Self::Earth => "dragon-lair-earth",
            Self::Water => "dragon-lair-water",
        }
    }

    fn sample(self) -> ([i64; 2], StructureKind) {
        match self {
            Self::Dark => ([-1, -5], StructureKind::DarkDragonLair),
            Self::Earth => ([-1, 3], StructureKind::EarthDragonLair),
            Self::Water => ([-6, 5], StructureKind::WaterDragonLair),
        }
    }
}

#[derive(Resource)]
struct Nest {
    home: [i64; 2],
    kind: CreatureKind,
    anchor: [i64; 3],
}

pub(super) fn views(world: &mut World, terrain: &TerraGenerator, kind: Kind) -> Option<Views> {
    let (region, structure) = kind.sample();
    let site = terrain.structure_site(region)?;
    if site.kind != structure {
        return None;
    }
    let [cx, y, cz] = site.center;
    let home = [cx.div_euclid(32), cz.div_euclid(32)];
    let (kind, anchor) = terrain.dragon_nest(home)?;
    let center = Vec3::from_array(anchor.map(|v| v as f32)) + Vec3::new(0.5, 0.0, 0.5);
    let [fx, fz] = site.dragon_entrance()?;
    let forward = Vec3::new(fx as f32, 0.0, fz as f32);
    let right = Vec3::Y.cross(forward);
    let mut overview = center + forward * 56.0 - right * 19.;
    let natural = terrain.surface_height(f64::from(overview.x), f64::from(overview.z)) as f32;
    overview.y = (natural + 10.).max(center.y + 33.0);
    let generator = SectionGenerator::new(terrain.clone());
    for _ in 0..52 {
        if generator
            .block_at(overview.floor().as_i64vec3().to_array())
            .ok()?
            == 0
        {
            break;
        }
        overview.y += 1.;
    }
    let close = center + right * 18.0 + forward * 11.0 + Vec3::Y * 6.3;
    // Soft fill from the open mouth for a readable photograph of the actual grotto.
    let tint = match kind {
        CreatureKind::DarkDragon => Color::srgb(0.79, 0.62, 1.),
        CreatureKind::EarthDragon => Color::srgb(1., 0.82, 0.60),
        _ => Color::srgb(0.65, 0.87, 1.),
    };
    world.spawn((
        PointLight {
            color: tint,
            intensity: 520_000.,
            range: 48.,
            radius: 5.,
            ..default()
        },
        Transform::from_translation(center + forward * 12. - right * 6. + Vec3::Y * 12.),
    ));
    let floor = [cx, y - 1, cz];
    let probes = vec![(floor, generator.block_at(floor).ok()?), (site.center, 0)];
    for eye in [overview, close] {
        let voxel = eye.floor().as_i64vec3().to_array();
        if generator.block_at(voxel).ok()? != 0 {
            error!(?voxel, "Lair camera is obstructed");
            return None;
        }
    }
    world.init_resource::<ReplicatedCreatures>();
    world.insert_resource(Nest { home, kind, anchor });
    Some(Views {
        overview: Transform::from_translation(overview)
            .looking_at(center + forward * 19. + Vec3::Y * 13., Vec3::Y),
        close: Transform::from_translation(close).looking_at(center + Vec3::Y * 4.2, Vec3::Y),
        anchor: center,
        probes,
    })
}

pub(super) fn guardian_ready(world: &mut World) -> Result<bool, String> {
    let nest = world.resource::<Nest>();
    let id = terra_core::creatures::creature_id(nest.home, 0).unwrap();
    if world
        .resource::<ReplicatedCreatures>()
        .creatures
        .contains_key(&id)
    {
        return Ok(world
            .query::<&terra_render::creatures::CreatureAvatar>()
            .iter(world)
            .any(|avatar| avatar.0 == id));
    }
    let mut population = CreaturePopulation::default();
    let habitat = Habitat {
        terrain: &world.resource::<WorldTerrain>().0,
        chunks: world.resource::<ChunkStore>(),
        journal: world.resource::<EditJournal>(),
    };
    if !terra_world::creatures::spawn_cell(&mut population, &habitat, nest.home) {
        return Err(format!(
            "Guardian {:?} cannot occupy its generated nest at {:?}",
            nest.kind, nest.anchor
        ));
    }
    let Some((id, animal)) = population
        .animals
        .pop_first()
        .filter(|(_, a)| a.kind == nest.kind)
    else {
        return Err(format!(
            "Missing {:?} in generated ecology cell {:?}",
            nest.kind, nest.home
        ));
    };
    world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .insert(
            id,
            CreatureTrack {
                previous: animal.clone(),
                current: animal,
                interval: 0.1,
                received: Instant::now(),
                playback: Default::default(),
            },
        );
    info!(?id, "Generated dragon guardian ready for presentation");
    Ok(false)
}
