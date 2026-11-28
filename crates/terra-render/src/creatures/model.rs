use self::voxel::Builder;
use bevy::prelude::*;
use terra_core::creatures::CreatureKind as Kind;

pub(super) const ELEPHANT_HEAD: Vec3 = Vec3::new(0., 2.29, -0.80);

mod aquatic;
mod bear;
mod bee;
mod clownfish;
pub(super) mod dragon;
mod elephant;
mod eyes;
mod forest;
mod horse;
mod lava_worm;
mod mammal;
pub(super) mod poultry;
pub(super) mod sandworm;
mod shape;
pub(super) mod soldier;
mod spider;
mod spirit;
mod stalker;
pub(super) mod stone_golem;
mod villager;
mod voxel;
pub(super) mod wild;

#[derive(Clone, Copy, Debug)]
pub(super) enum Part {
    Body,
    Golem(stone_golem::Joint, bool),
    Dragon(dragon::Joint),
    DragonLight(dragon::Joint),
    SpiritGlow,
    ElephantEar(f32),
    ElephantTrunk,
    LavaJaw(u8),
    SpiritArm(f32),
    SpiritOrbit(f32),
    Head,
    Leg(f32),
    Tail,
    Wing(f32),
    ReptileJoint(u8),
    SnakeTongue,
    Proboscis,
    Eyelids,
    Bell,
    Tentacles(f32),
    Fin(f32),
    Jaw,
    SpiderLeg { side: f32, index: u8 },
    SpiderEyes,
    WebShot,
    Silk,
    StalkerArm(f32),
    StalkerForearm(f32),
    StalkerHand(f32),
    StalkerEyes,
    SkeletonEyes,
    ThornGlow { head: bool },
    VillagerArm(f32),
    VillagerSleeve(f32),
    VillagerThigh(f32),
    MilitaryWeapon(f32),
    MilitaryTabard,
    LavaRing { index: u8, glowing: bool },
    WormSegment(u8),
    WormMound,
    WormDust,
    WormRipple(u8),
    WormMandible(u8),
    WormSpit(u8),
}

pub(super) fn parts(kind: Kind) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    parts_variant(kind, 0)
}

/// Cache key only; the persisted profession and immutable sex stay independent.
pub(super) fn villager_variant(role: u8, sex: terra_core::creatures::CreatureSex) -> u8 {
    let role = role % terra_core::creatures::villagers::ROLE_COUNT;
    if terra_core::creatures::villagers::is_civilian(role)
        && (sex == terra_core::creatures::CreatureSex::Female) != villager::female(role)
    {
        role + terra_core::creatures::villagers::ROLE_COUNT
    } else {
        role
    }
}
pub(super) fn parts_variant(kind: Kind, variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    if kind == Kind::Fish && variant >= 2 {
        return river_fish();
    }
    match kind {
        Kind::Bee => bee::parts(),
        Kind::Tiger | Kind::Lion | Kind::Crocodile | Kind::Jaguar | Kind::Snake => {
            wild::parts(kind, variant)
        }
        Kind::Butterfly => spirit::butterfly(variant),
        Kind::SpiritGloom => spirit::gloom(variant),
        Kind::SpiritFox => spirit::fox(),
        Kind::Skeleton => forest::skeleton(variant),
        Kind::Raven => forest::raven(),
        Kind::Hen | Kind::Rooster => poultry::parts(kind, variant),
        Kind::BloodSlime => forest::slime(),
        Kind::BloodStalker => forest::stalker(),
        Kind::BloodWisp => forest::wisp(),
        Kind::ThornBrute => forest::brute(),
        Kind::StoneGolem => stone_golem::parts(variant),
        Kind::SandWorm => sandworm::parts(),
        Kind::LavaWorm => lava_worm::parts(),
        Kind::DarkDragon | Kind::EarthDragon | Kind::FireDragon | Kind::WaterDragon => {
            dragon::parts(kind, variant)
        }
        Kind::Fish => clownfish::parts(),
        Kind::Cow | Kind::Pig | Kind::Sheep => mammal::parts(kind),
        Kind::Bear => bear::parts(),
        Kind::Horse => horse::parts(variant),
        Kind::Zebra => horse::zebra(),
        Kind::Elephant => elephant::parts(),
        Kind::Squid => aquatic::squid(),
        Kind::MoonJelly
        | Kind::FireJelly
        | Kind::VoidJelly
        | Kind::SunJelly
        | Kind::CrystalJelly => aquatic::jelly(kind),
        Kind::ReefShark | Kind::HammerheadShark | Kind::WhaleShark => aquatic::shark(kind),
        Kind::SmallSpider | Kind::MediumSpider | Kind::LargeSpider => spider::parts(kind),
        Kind::Stalker => stalker::parts(),
        Kind::Villager => {
            let count = terra_core::creatures::villagers::ROLE_COUNT;
            if variant >= count {
                let role = variant % count;
                return villager::parts_for_sex(role, !villager::female(role));
            }
            if let Some(loadout) =
                terra_core::creatures::soldiers::SoldierLoadout::from_role(variant)
            {
                soldier::parts(loadout)
            } else {
                villager::parts(variant)
            }
        }
    }
}

fn river_fish() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    let teal = [0.035, 0.60, 0.67];
    let gold = [0.98, 0.65, 0.15];
    let cream = [0.96, 0.91, 0.72];
    for z in 0..9 {
        let t = z as f32 / 8.0;
        let w = 0.13 + (std::f32::consts::PI * t).sin() * 0.12;
        let h = 0.13 + (std::f32::consts::PI * t).sin() * 0.15;
        body.voxel([0.0, 0.16, -0.24 + t * 0.48], [w, h, 0.065], teal)?;
        body.voxel(
            [0.0, 0.16 - h * 0.35, -0.24 + t * 0.48],
            [w + 0.006, h * 0.3, 0.066],
            cream,
        )?;
        if z > 1 && z < 7 {
            let rise = 0.045 + (1.0 - t) * 0.12;
            body.voxel(
                [0.0, 0.16 + h * 0.5 + rise * 0.5, -0.24 + t * 0.48],
                [0.028, rise, 0.065],
                if z % 3 == 0 { gold } else { teal },
            )?;
        }
    }
    for side in [-1.0, 1.0] {
        body.voxel([side * 0.10, 0.20, -0.205], [0.035, 0.10, 0.075], cream)?;
        body.voxel(
            [side * 0.121, 0.20, -0.214],
            [0.015, 0.070, 0.045],
            [0.025, 0.035, 0.04],
        )?;
        body.voxel([side * 0.13, 0.09, -0.04], [0.13, 0.026, 0.13], gold)?;
        for i in 0..3 {
            body.voxel(
                [side * 0.10, 0.22, -0.08 + i as f32 * 0.07],
                [0.02, 0.025, 0.024],
                gold,
            )?;
        }
    }
    let mut tail = Builder::default();
    for i in 0..4 {
        tail.voxel(
            [0.0, 0.0, 0.035 + i as f32 * 0.045],
            [0.035, 0.11 + i as f32 * 0.07, 0.05],
            if i >= 2 { gold } else { teal },
        )?;
    }
    Ok(vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Tail, Vec3::new(0.0, 0.16, 0.25), tail.finish()?),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    #[test]
    fn horse_coats_have_sixteen_distinct_shared_indexed_models_and_grounded_hooves() {
        let mut coats = std::collections::HashSet::new();
        for variant in 0..16 {
            let parts = parts_variant(Kind::Horse, variant).unwrap();
            assert_eq!(parts.len(), 8);
            let mut fingerprint = 0_u64;
            let mut vertices = 0;
            for (part, offset, mesh) in parts {
                vertices += mesh.count_vertices();
                assert!(
                    mesh.indices()
                        .unwrap()
                        .iter()
                        .all(|i| i < mesh.count_vertices())
                );
                if let Some(VertexAttributeValues::Float32x4(colors)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                {
                    for color in colors {
                        for value in color {
                            fingerprint = fingerprint.wrapping_mul(1099511628211)
                                ^ u64::from(value.to_bits());
                        }
                    }
                }
                if matches!(part, Part::Leg(_))
                    && let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    let bottom = positions
                        .iter()
                        .map(|v| v[1] + offset.y)
                        .fold(f32::INFINITY, f32::min);
                    assert!(bottom.abs() < 0.001);
                }
                if matches!(part, Part::Body)
                    && let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    let width = positions
                        .iter()
                        .map(|p| p[0])
                        .fold(f32::NEG_INFINITY, f32::max)
                        - positions.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
                    assert!(
                        (0.57..0.595).contains(&width),
                        "muscular chest inside reference hitbox: {width}"
                    );
                }
            }
            assert!(vertices < 16000);
            assert!(coats.insert(fingerprint), "duplicate coat {variant}");
        }
    }

    #[test]
    fn all_species_have_bounded_shared_indexed_models() {
        for kind in Kind::ALL {
            let parts = parts(kind).unwrap();
            let (part_limit, extent_limit, vertex_limit) = match kind {
                // Each paw has its own ankle pivot; the outer breath stream is a separate material.
                // Each dragon also has two independently animated cheek crests.
                Kind::EarthDragon | Kind::WaterDragon => (31, 24.0, 65_000),
                Kind::FireDragon => (35, 24.0, 65_000),
                Kind::DarkDragon => (37, 24.0, 65_000),
                Kind::SandWorm => (58, 23.0, 24_000),
                // Fifteen tapered basalt rings carry separate lava seams.
                Kind::LavaWorm => (40, 2.5, 32_000),
                Kind::Elephant => (10, 3.2, 18_000),
                Kind::Crocodile => (10, 2.5, 16_000),
                Kind::Snake => (11, 2.5, 16_000),
                Kind::Stalker => (11, 3.0, 64_000),
                // Ten stones, five inlays, an awakening seal and two impact meshes.
                Kind::StoneGolem => (18, 4.05, 60_000),
                // Residents use the same cached, detailed reference meshes as
                // their profession variants; the dedicated role test enforces
                // this budget independently for every civilian silhouette.
                Kind::Villager => (10, 2.5, 50_000),
                Kind::BloodStalker => (12, 2.5, 16_000),
                Kind::SmallSpider => (12, 2.5, 16_000),
                Kind::MediumSpider | Kind::LargeSpider => (13, 2.5, 16_000),
                _ => (8, 2.5, 16_000),
            };
            assert!(parts.len() <= part_limit, "{kind:?}: {} parts", parts.len());
            let mut total = 0;
            let mut triangles = 0;
            for (part, offset, mesh) in parts {
                let count = mesh.count_vertices();
                total += count;
                assert!(mesh.indices().unwrap().iter().all(|i| i < count));
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Missing positions");
                };
                let Some(VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!("Missing normals");
                };
                for (position, normal) in positions.iter().zip(normals) {
                    let position = Vec3::from_array(*position) + offset;
                    let normal = Vec3::from_array(*normal);
                    assert!(position.is_finite() && normal.is_finite(), "{kind:?}");
                    assert!(
                        position.abs().max_element() < extent_limit,
                        "{kind:?}: {position}"
                    );
                    assert!(
                        (normal.length_squared() - 1.0).abs() < 0.001,
                        "{kind:?}: {normal}"
                    );
                    if !matches!(part, Part::WormRipple(_) | Part::WebShot) {
                        // Water-like surface ripples and web projectiles are effect geometry.
                        // Every animal's articulated anatomy is a voxel surface.
                        assert!(
                            normal.abs().max_element() > 0.99999
                                && normal.abs().element_sum() < 1.00001,
                            "{kind:?}: every face must be axis-aligned in its local joint frame: {normal}"
                        );
                    }
                }
                let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
                triangles += indices.len() / 3;
                for triangle in indices.chunks_exact(3) {
                    let a = Vec3::from_array(positions[triangle[0]]);
                    let b = Vec3::from_array(positions[triangle[1]]);
                    let c = Vec3::from_array(positions[triangle[2]]);
                    assert!(
                        (b - a).cross(c - a).length_squared() > 1e-16,
                        "{kind:?}: degenerate triangle"
                    );
                }
            }
            assert!(total < vertex_limit, "{kind:?}: {total}");
            println!("{kind:?}: {total} vertices, {triangles} triangles");
        }
    }
    #[test]
    fn riverfin_variant_has_valid_shared_geometry() {
        let fish = parts_variant(Kind::Fish, 2).unwrap();
        assert_eq!(fish.len(), 2);
        let total: usize = fish.iter().map(|(_, _, m)| m.count_vertices()).sum();
        // Body scales receive the shared bevel treatment; eyes and thin fins keep
        // their economical planar geometry and two-part articulation.
        assert!(total <= 3_000, "River fish vertex budget: {total}");
        let triangles = fish
            .iter()
            .map(|(_, _, m)| m.indices().unwrap().len() / 3)
            .sum::<usize>();
        assert!(
            triangles <= 1_400,
            "River fish triangle budget: {triangles}"
        );
        for (_, _, mesh) in fish {
            assert!(
                mesh.indices()
                    .unwrap()
                    .iter()
                    .all(|i| i < mesh.count_vertices())
            );
        }
    }

    #[test]
    fn aquatic_models_use_meter_bounds_and_a_small_fixed_articulation_budget() {
        for kind in Kind::ALL
            .into_iter()
            .filter(|kind| kind.is_aquatic() && *kind != Kind::Fish)
        {
            let parts = parts(kind).unwrap();
            assert_eq!(parts.len(), 5, "{kind:?}: bounded shared articulation");
            assert_eq!(
                parts
                    .iter()
                    .filter(|(part, _, _)| matches!(part, Part::Bell))
                    .count(),
                usize::from(kind.is_jelly())
            );
            let size = Vec3::from_array(kind.size().map(|v| v as f32));
            for (_, origin, mesh) in parts {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Missing positions")
                };
                for vertex in positions {
                    let point = Vec3::from_array(*vertex) + origin;
                    assert!(
                        point.x.abs() <= size.x * 0.55,
                        "{kind:?}: width {point:?}/{size:?}"
                    );
                    assert!(
                        point.y >= -0.005 && point.y <= size.y * 1.025,
                        "{kind:?}: height {point:?}/{size:?}"
                    );
                    assert!(
                        point.z.abs() <= size.z * 0.55,
                        "{kind:?}: length {point:?}/{size:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn bear_coat_claws_and_face_share_eight_parts_with_grounded_paws() {
        let parts = parts(Kind::Bear).unwrap();
        assert_eq!(parts.len(), 8);
        assert_eq!(
            parts
                .iter()
                .filter(|(part, _, _)| matches!(part, Part::Leg(_)))
                .count(),
            4
        );
        for (part, offset, mesh) in &parts {
            if !matches!(part, Part::Leg(_)) {
                continue;
            }
            let Some(VertexAttributeValues::Float32x3(vertices)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("Bear paw positions missing");
            };
            let bottom = vertices
                .iter()
                .map(|p| p[1] + offset.y)
                .fold(f32::INFINITY, f32::min);
            assert!(
                bottom.abs() < 0.001,
                "Bear paw must rest on the authoritative ground plane: {bottom}"
            );
            assert!(
                vertices.iter().any(|p| p[2] < -0.28),
                "Claws should project ahead of the broad paw"
            );
        }
        let vertices: usize = parts.iter().map(|(_, _, mesh)| mesh.count_vertices()).sum();
        assert!(vertices < 16_000, "{vertices}");
        let colors = parts
            .iter()
            .flat_map(|(_, _, mesh)| {
                let Some(VertexAttributeValues::Float32x4(colors)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("bear coat colors")
                };
                colors.iter().map(|color| color.map(f32::to_bits))
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            colors.len() >= 12,
            "Coat layers, muzzle, eyes and claws retain their palette"
        );
    }

    #[test]
    fn voxel_outline_is_closed_and_outward_facing() {
        let mut builder = Builder::default();
        shape::rounded(&mut builder, [0.0; 3], [0.8, 0.7, 1.2], 0.12, [0.8; 3]).unwrap();
        let mesh = builder.finish().unwrap();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!()
        };
        assert!(mesh.indices().unwrap().len() / 3 <= 1_024);
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        for triangle in indices.chunks_exact(3) {
            let [a, b, c] =
                [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from_array(positions[i]));
            let center = (a + b + c) / 3.0;
            assert!(center.dot((b - a).cross(c - a)) > 0.0);
            assert!(center.dot(Vec3::from_array(normals[triangle[0]])) > 0.0);
        }
    }
}
