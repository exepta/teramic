//! Visible realm cloth on village walls, stronghold gates and roof flagpoles.
//! No generated block or gameplay state is changed to attach a realm's cloth.
use crate::players::model::Builder;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::kingdom::KingdomHeraldry;
use terra_network::client::ClientSession;
use terra_world::{
    edits::EditJournal,
    generation::structures::{BuildingRole, BuildingSummary},
    streaming::ChunkStore,
};

#[derive(Resource, Default, Debug)]
pub struct VillageHeraldryStatus {
    pub banners: usize,
    pub realms: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Profile {
    Village,
    Stronghold,
    Flag,
}
struct Mount {
    cell: [i64; 3],
    center: Vec3,
    normal: Vec3,
    profile: Profile,
}
type PlacedKey = (u64, [i64; 3], Profile);
type MeshKey = (KingdomHeraldry, Profile);
#[derive(Resource, Default)]
struct Banners {
    placed: BTreeMap<PlacedKey, (Entity, KingdomHeraldry)>,
    meshes: BTreeMap<MeshKey, Handle<Mesh>>,
    material: Handle<StandardMaterial>,
    next: f32,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Banners>()
        .init_resource::<VillageHeraldryStatus>()
        .add_systems(Update, synchronize);
}
fn anchor(building: &BuildingSummary) -> Option<([i64; 3], Vec3)> {
    if !matches!(
        building.role,
        BuildingRole::Church | BuildingRole::Watch | BuildingRole::Police | BuildingRole::Smith
    ) {
        return None;
    }
    let [x, y, z] = building.entrance;
    let [low, high] = building.bounds;
    let distances = [z - low[2], high[2] - z, x - low[0], high[0] - x];
    let side = (0..4).min_by_key(|i| distances[*i])?;
    let normal = [-Vec3::Z, Vec3::Z, -Vec3::X, Vec3::X][side];
    let tangent = [normal.z as i64, 0, -normal.x as i64];
    Some(([x + tangent[0] * 2, y + 1, z + tangent[2] * 2], normal))
}
#[cfg(test)]
fn cloth(heraldry: KingdomHeraldry) -> Result<Mesh, String> {
    cloth_profile(heraldry, Profile::Village)
}
fn cloth_profile(heraldry: KingdomHeraldry, profile: Profile) -> Result<Mesh, String> {
    let mut builder = Builder::default();
    // Avatar Builder accepts sRGB and performs the PBR linear conversion once.
    let color = heraldry.color.map(|value| f32::from(value) / 255.0);
    let (half, top, bottom, tip, glyph, center_y) = match profile {
        Profile::Village => (0.35, 0.60, -0.48, -0.70, 0.512, 0.104),
        Profile::Stronghold => (1.0, 1.5, -1.0, -1.5, 1.25, 0.24),
        Profile::Flag => (1.4, 0.65, -0.65, -0.65, 0.88, 0.0),
    };
    builder.voxel(
        [0.0, (top + bottom) * 0.5, 0.0],
        [half * 2.0, top - bottom, 0.012],
        color,
    )?;
    if profile == Profile::Flag {
        builder.triangle(
            [
                Vec3::new(half, bottom, -0.007),
                Vec3::new(half + 0.35, 0.0, -0.007),
                Vec3::new(half, top, -0.007),
            ],
            color,
        )?;
    } else {
        builder.triangle(
            [
                Vec3::new(-half, bottom, -0.007),
                Vec3::new(0.0, tip, -0.007),
                Vec3::new(half, bottom, -0.007),
            ],
            color,
        )?;
    }
    let gold = [0.82, 0.59, 0.24];
    for x in [-half + 0.01, half - 0.01] {
        builder.voxel(
            [x, (top + bottom) * 0.5, -0.009],
            [0.022, top - bottom + 0.01, 0.012],
            gold,
        )?;
    }
    builder.voxel([0.0, top, -0.009], [half * 2.0, 0.025, 0.012], gold)?;
    if profile != Profile::Flag {
        builder.voxel(
            [0.0, top + 0.06, 0.01],
            [half * 2.0 + 0.16, 0.065, 0.075],
            [0.24, 0.14, 0.065],
        )?;
    }
    let icon = if f32::from(heraldry.color[0]) * 0.2126
        + f32::from(heraldry.color[1]) * 0.7152
        + f32::from(heraldry.color[2]) * 0.0722
        > 164.0
    {
        [0.045, 0.027, 0.012]
    } else {
        [0.97, 0.80, 0.43]
    };
    if let Some(mask) = heraldry.icon.glyph_mask() {
        for (row, &bits) in mask.iter().enumerate() {
            let mut x = 0;
            while x < 32 {
                if bits & (1 << x) == 0 {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < 32 && bits & (1 << x) != 0 {
                    x += 1;
                }
                let cell = glyph / 32.0;
                builder.voxel(
                    [
                        (start as f32 + (x - start) as f32 * 0.5 - 16.0) * cell,
                        center_y + glyph * 0.5 - row as f32 * cell,
                        -0.021,
                    ],
                    [(x - start) as f32 * cell, cell, 0.006],
                    icon,
                )?;
            }
        }
    }
    builder.finish()
}
fn open(material: u16) -> bool {
    material == 0
}
fn wall(material: u16) -> bool {
    material != 0
        && material != terra_voxel::MaterialId::SolidWater as u16
        && !matches!(
            material,
            65..=70 | terra_core::castle::WINDOW | terra_core::castle::ARROW_SLIT
        )
}
/// Stronghold builders face their formal entrances south; gate bounds include
/// their bridge and are deliberately unsuitable for nearest-bounds-face picking.
fn stronghold_wall(
    building: &BuildingSummary,
    side: i64,
    mut sample: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<Mount> {
    if !matches!(
        building.role,
        BuildingRole::Keep
            | BuildingRole::Gate
            | BuildingRole::Church
            | BuildingRole::Watch
            | BuildingRole::Barracks
    ) {
        return None;
    }
    sample(building.entrance)?;
    let heights: &[i64] = if building.role == BuildingRole::Keep {
        &[14, 8, 20, 7, 6, 5]
    } else {
        &[8, 7, 6, 5, 4, 3]
    };
    let offsets: &[i64] = if building.role == BuildingRole::Watch {
        &[0, 2, 4, 6, 8, 10, 12]
    } else {
        &[4, 6, 8, 10, 12, 2]
    };
    let depths = [0, 1, 2, 3, 4, 5, 6, 7, 8, -1, -2, -3, -4, -5, -6, -7, -8];
    for &height in heights {
        for &offset in offsets {
            for depth in depths {
                let p = [
                    building.entrance[0] + offset * side,
                    building.entrance[1] + height,
                    building.entrance[2] + depth,
                ];
                if p[0] < building.bounds[0][0]
                    || p[0] > building.bounds[1][0]
                    || p[2] < building.bounds[0][2]
                    || p[2] > building.bounds[1][2]
                    || !sample(p).is_some_and(wall)
                {
                    continue;
                }
                // Check the whole hanging face, not just the centre: a curved tower can
                // have air beside its surface and a gate can have a seven-cell opening.
                let clear = (-1..=1).all(|dx| {
                    (-3..=0).all(|dy| sample([p[0] + dx, p[1] + dy, p[2] + 1]).is_some_and(open))
                });
                let no_window = (-3..=0).all(|dy| {
                    sample([p[0], p[1] + dy, p[2]]).is_none_or(|m| {
                        !matches!(
                            m,
                            65..=70 | terra_core::castle::WINDOW | terra_core::castle::ARROW_SLIT
                        )
                    })
                });
                if clear && no_window {
                    return Some(Mount {
                        cell: p,
                        center: Vec3::new(
                            p[0] as f32 + 0.5,
                            p[1] as f32 - 1.05,
                            p[2] as f32 + 1.025,
                        ),
                        normal: Vec3::Z,
                        profile: Profile::Stronghold,
                    });
                }
            }
        }
    }
    None
}
fn roof_flag(
    building: &BuildingSummary,
    mut sample: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<Mount> {
    if building.role != BuildingRole::Watch {
        return None;
    }
    let x = (building.bounds[0][0] + building.bounds[1][0]) / 2;
    let z = (building.bounds[0][2] + building.bounds[1][2]) / 2;
    for y in (building.entrance[1] + 6..=building.bounds[1][1] + 16).rev() {
        if sample([x, y, z]) == Some(36)
            && sample([x, y - 1, z]) == Some(36)
            && sample([x, y + 1, z]) == Some(0)
        {
            let p = [x, y, z];
            // The left edge attaches to the surviving wooden mast. Keep the
            // emblem upright while the horizontal flag points to its right.
            return Some(Mount {
                cell: p,
                center: Vec3::new(x as f32 + 0.5 - 1.45, y as f32 - 0.05, z as f32 + 0.52),
                normal: Vec3::Z,
                profile: Profile::Flag,
            });
        }
    }
    None
}
fn priority(role: BuildingRole) -> u8 {
    match role {
        BuildingRole::Keep => 0,
        BuildingRole::Gate => 1,
        BuildingRole::Church => 2,
        BuildingRole::Barracks => 3,
        BuildingRole::Watch => 4,
        _ => 5,
    }
}
#[allow(clippy::too_many_arguments)]
fn synchronize(
    mut commands: Commands,
    time: Res<Time>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    session: Option<Res<ClientSession>>,
    mut state: ResMut<Banners>,
    mut status: ResMut<VillageHeraldryStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs();
    if now < state.next {
        return;
    }
    state.next = now + 0.5;
    let registry = session
        .as_ref()
        .filter(|s| s.ready)
        .map(|s| &s.kingdoms)
        .or_else(|| journal.as_ref().map(|j| &j.gameplay().kingdoms));
    let Some((chunks, registry)) = chunks.as_ref().zip(registry) else {
        for (entity, _) in state.placed.values() {
            commands.entity(*entity).despawn();
        }
        state.placed.clear();
        status.banners = 0;
        status.realms = 0;
        return;
    };
    if state.material == Handle::default() {
        state.material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.94,
            cull_mode: None,
            ..default()
        });
    }
    let mut seen = BTreeSet::new();
    let mut settlements = BTreeSet::new();
    let mut colors = BTreeSet::new();
    for settlement in chunks.village_settlements() {
        if !settlements.insert(settlement.id) {
            continue;
        }
        let domain = settlement.domain();
        let ordinary =
            settlement.layout == terra_world::generation::structures::SettlementLayout::Village;
        let mut buildings = settlement.services.iter().collect::<Vec<_>>();
        buildings.sort_by_key(|building| {
            (
                priority(building.role),
                building.entrance[0].abs_diff(domain.center[0])
                    + building.entrance[2].abs_diff(domain.center[2]),
            )
        });
        let mut plans = Vec::new();
        for building in buildings {
            if ordinary {
                let Some((cell, normal)) = anchor(building) else {
                    continue;
                };
                let Some(material) = chunks.block_at(cell) else {
                    continue;
                };
                if !wall(material) || material == terra_core::castle::ROYAL_WALL {
                    continue;
                }
                let center = Vec3::new(
                    cell[0] as f32 + 0.5,
                    cell[1] as f32 + 1.05,
                    cell[2] as f32 + 0.5,
                ) + normal * 0.515;
                plans.push(Mount {
                    cell,
                    center,
                    normal,
                    profile: Profile::Village,
                });
            } else {
                for side in if matches!(building.role, BuildingRole::Keep | BuildingRole::Gate) {
                    &[-1, 1][..]
                } else {
                    &[1][..]
                } {
                    if let Some(mount) = stronghold_wall(building, *side, |p| chunks.block_at(p)) {
                        plans.push(mount);
                    }
                }
                if let Some(flag) = roof_flag(building, |p| chunks.block_at(p)) {
                    plans.push(flag);
                }
            }
            if plans.len() >= 12 {
                break;
            }
        }
        for mount in plans.into_iter().take(12) {
            let Some(style) = registry.domain_heraldry(&domain, mount.cell) else {
                continue;
            };
            let key = (settlement.id, mount.cell, mount.profile);
            let mesh_key = (style, mount.profile);
            seen.insert(key);
            colors.insert(mesh_key);
            if state.placed.get(&key).is_some_and(|(_, old)| *old == style) {
                continue;
            }
            let mesh = if let Some(mesh) = state.meshes.get(&mesh_key) {
                mesh.clone()
            } else {
                let Ok(mesh) = cloth_profile(style, mount.profile) else {
                    continue;
                };
                let handle = meshes.add(mesh);
                state.meshes.insert(mesh_key, handle.clone());
                handle
            };
            if let Some((entity, _)) = state.placed.remove(&key) {
                commands.entity(entity).despawn();
            }
            let root = commands
                .spawn((
                    Name::new("Settlement heraldry"),
                    Mesh3d(mesh),
                    MeshMaterial3d(state.material.clone()),
                    Transform::from_translation(mount.center)
                        .with_rotation(Quat::from_rotation_arc(-Vec3::Z, mount.normal)),
                ))
                .id();
            state.placed.insert(key, (root, style));
            if seen.len() >= 512 {
                break;
            }
        }
        if seen.len() >= 512 {
            break;
        }
    }
    state.placed.retain(|key, (entity, _)| {
        if seen.contains(key) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });
    state.meshes.retain(|style, _| colors.contains(style));
    status.banners = state.placed.len();
    status.realms = colors
        .iter()
        .map(|(style, _)| style)
        .collect::<BTreeSet<_>>()
        .len();
}
#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::kingdom::{BotKingdom, KingdomIcon};
    #[test]
    fn native_village_cloth_contains_real_color_and_glyph_for_every_bot_realm() {
        for realm in BotKingdom::ALL {
            let style = realm.heraldry();
            let mesh = cloth(style).unwrap();
            assert!(mesh.count_vertices() > 100);
            let expected = Color::srgb_u8(style.color[0], style.color[1], style.color[2])
                .to_linear()
                .to_f32_array();
            let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("Missing native cloth vertex colors")
            };
            assert!(
                colors.iter().any(|color| color
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| (*a - b).abs() < 1e-6)),
                "Native cloth must match the actual realm picker RGB"
            );
            let other = cloth(KingdomHeraldry {
                icon: KingdomIcon::from_key("shield").unwrap(),
                ..style
            })
            .unwrap();
            assert_ne!(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION),
                other.attribute(Mesh::ATTRIBUTE_POSITION)
            );
        }
    }
    #[test]
    fn ordinary_service_anchors_stay_outside_doors_on_each_front_face() {
        for (entrance, normal) in [
            ([5, 10, 0], -Vec3::Z),
            ([5, 10, 10], Vec3::Z),
            ([0, 10, 5], -Vec3::X),
            ([10, 10, 5], Vec3::X),
        ] {
            let b = BuildingSummary {
                role: BuildingRole::Church,
                entrance,
                bounds: [[0, 8, 0], [10, 30, 10]],
            };
            let (cell, actual) = anchor(&b).unwrap();
            assert_eq!(actual, normal);
            assert_eq!(cell[1], 11);
            assert!(cell[0].abs_diff(entrance[0]) + cell[2].abs_diff(entrance[2]) == 2);
        }
    }
    #[test]
    fn large_gate_cloth_finds_actual_curved_tower_surfaces_and_hangs_above_ground() {
        let gate = BuildingSummary {
            role: BuildingRole::Gate,
            entrance: [0, 0, 0],
            bounds: [[-14, -4, -5], [14, 24, 21]],
        };
        let sample = |p: [i64; 3]| {
            if !(0..12).contains(&p[1]) {
                return Some(0);
            }
            let shell = [-8, 8].into_iter().any(|x| {
                let rr = (p[0] - x).pow(2) + p[2].pow(2);
                (16..=25).contains(&rr)
            });
            if shell { Some(43) } else { Some(0) }
        };
        for sign in [-1, 1] {
            let mount = stronghold_wall(&gate, sign, sample)
                .expect("Both gate towers need visible heraldic cloth");
            assert_eq!(
                mount.normal,
                Vec3::Z,
                "The bridge must not reverse the gate facade"
            );
            assert!(mount.cell[0] * sign >= 4);
            assert_eq!(sample(mount.cell), Some(43));
            assert!(
                mount.center.y - 1.5 > gate.entrance[1] as f32 + 1.0,
                "Large cloth must remain above the gate approach"
            );
            assert_eq!(
                sample([mount.cell[0], mount.cell[1], mount.cell[2] + 1]),
                Some(0)
            );
        }
        let style = terra_core::kingdom::BotKingdom::Betross.heraldry();
        let mesh = cloth_profile(style, Profile::Stronghold).unwrap();
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Banner mesh positions")
        };
        let min = positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max = positions
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (3.0..3.2).contains(&(max - min)),
            "Stronghold cloth must be approximately three metres tall"
        );
        let other = cloth_profile(
            terra_core::kingdom::BotKingdom::Helteria.heraldry(),
            Profile::Stronghold,
        )
        .unwrap();
        assert_ne!(
            mesh.attribute(Mesh::ATTRIBUTE_COLOR),
            other.attribute(Mesh::ATTRIBUTE_COLOR)
        );
        assert_ne!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            other.attribute(Mesh::ATTRIBUTE_POSITION)
        );
    }
    #[test]
    fn roof_flag_attaches_to_existing_wooden_pole_and_contains_real_emblem_geometry() {
        let tower = BuildingSummary {
            role: BuildingRole::Watch,
            entrance: [0, 3, -8],
            bounds: [[-6, 1, -20], [6, 29, -8]],
        };
        let mount = roof_flag(&tower, |p| {
            Some(if p[0] == 0 && p[2] == -14 && (29..=32).contains(&p[1]) {
                36
            } else {
                0
            })
        })
        .unwrap();
        assert_eq!(mount.cell, [0, 32, -14]);
        assert!(mount.center.y > tower.bounds[1][1] as f32);
        let mesh = cloth_profile(
            terra_core::kingdom::BotKingdom::Betross.heraldry(),
            Profile::Flag,
        )
        .unwrap();
        assert!(mesh.count_vertices() > 100);
        let other = cloth_profile(
            terra_core::kingdom::BotKingdom::Lutera.heraldry(),
            Profile::Flag,
        )
        .unwrap();
        assert_ne!(
            mesh.attribute(Mesh::ATTRIBUTE_COLOR),
            other.attribute(Mesh::ATTRIBUTE_COLOR)
        );
        assert_ne!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            other.attribute(Mesh::ATTRIBUTE_POSITION)
        );
    }
}
