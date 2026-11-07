//! Per-cell heraldry and deterministic detail variation for authored castle scenes.
use super::{ModelCell, ModelSnapshot};
use bevy::math::Quat;
use terra_core::kingdom::{KingdomHeraldry, KingdomRegistry};
use terra_voxel::SectionPos;

/// At most112 local profiles per section job; pose stays in the shared transform.
#[derive(Default)]
pub(super) struct Profiles(
    std::collections::HashMap<(u16, u8), Vec<crate::blocks::PrimitiveTemplate>>,
);
impl Profiles {
    pub(super) fn get<'a>(
        &'a mut self,
        model: &ModelCell,
        source: &[crate::blocks::PrimitiveTemplate],
    ) -> Result<std::borrow::Cow<'a, [crate::blocks::PrimitiveTemplate]>, String> {
        use terra_core::castle::connections::{
            MODULE_COUNT, MODULE_PRIMITIVES, modules, profile_key,
        };
        if source.len() != MODULE_COUNT * MODULE_PRIMITIVES {
            return Err("Invalid authored battlement module layout".into());
        }
        let neighbors = std::array::from_fn(|side| {
            model.neighbors[side].map(|id| (id, model.neighbor_facing[side]))
        });
        let mask = profile_key(model.material, model.state.facing, neighbors);
        let profile = self.0.entry((model.material, mask)).or_insert_with(|| {
            modules(model.material, mask)
                .into_iter()
                .flat_map(|module| {
                    source[module * MODULE_PRIMITIVES..(module + 1) * MODULE_PRIMITIVES]
                        .iter()
                        .cloned()
                })
                .collect()
        });
        Ok(std::borrow::Cow::Borrowed(profile))
    }
}

fn banner(material: u16) -> bool {
    matches!(
        material,
        terra_core::castle::BERGFRIED | terra_core::castle::ROYAL_WALL
    )
}

impl ModelSnapshot {
    pub(in crate::meshing) fn set_domains(
        &mut self,
        domains: Vec<terra_core::kingdom::SettlementDomain>,
    ) {
        self.domains = domains;
    }
    fn resolved_heraldry(
        &self,
        registry: &KingdomRegistry,
        position: [i64; 3],
    ) -> Option<KingdomHeraldry> {
        registry.heraldry_at(position).or_else(|| {
            self.domains
                .iter()
                .find(|domain| domain.contains(position))
                .and_then(|domain| registry.domain_heraldry(domain, position))
        })
    }

    pub(in crate::meshing) fn heraldry_changed(
        &self,
        section: SectionPos,
        registry: &KingdomRegistry,
    ) -> bool {
        let Ok(origin) = section.origin() else {
            return false;
        };
        self.models
            .iter()
            .filter(|model| banner(model.material))
            .any(|model| {
                let position =
                    std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
                self.heraldry.get(&model.cell).copied()
                    != self.resolved_heraldry(registry, position)
            })
    }

    pub(in crate::meshing) fn refresh_heraldry(
        &mut self,
        section: SectionPos,
        registry: &KingdomRegistry,
    ) {
        let Ok(origin) = section.origin() else {
            return;
        };
        self.heraldry.clear();
        for model in self.models.iter().filter(|model| banner(model.material)) {
            let position = std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
            if let Some(heraldry) = self.resolved_heraldry(registry, position) {
                self.heraldry.insert(model.cell, heraldry);
            }
        }
    }

    pub(super) fn castle_heraldry(&self, cell: [i32; 3]) -> KingdomHeraldry {
        self.heraldry.get(&cell).copied().unwrap_or_default()
    }
}

/// A cut face disappears only for the opposite half in the exact same pose.
pub(super) fn paired_arch(model: &ModelCell) -> bool {
    let opposite = match model.material {
        terra_core::castle::ARCH_LEFT => terra_core::castle::ARCH_RIGHT,
        terra_core::castle::ARCH_RIGHT => terra_core::castle::ARCH_LEFT,
        _ => return false,
    };
    let direction = terra_core::construction::stair_basis(model.state.facing)[0];
    let sign = if model.material == terra_core::castle::ARCH_LEFT {
        1
    } else {
        -1
    };
    let Some(axis) = direction.iter().position(|coordinate| *coordinate != 0) else {
        return false;
    };
    let side = axis * 2 + usize::from(direction[axis] * sign > 0);
    model.neighbors[side] == Some(opposite) && model.neighbor_facing[side] == model.state.facing
}

/// Proper cube rotations vary only decorative faces, retaining authored structure/collision.
pub(super) fn detail_rotation(section: SectionPos, cell: [i32; 3]) -> Quat {
    let origin = section.origin().unwrap_or([0; 3]);
    let mut hash = 0x9e37_79b9_7f4a_7c15_u64;
    for (offset, local) in origin.into_iter().zip(cell) {
        hash ^= (offset.wrapping_add(i64::from(local)) as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash = hash.rotate_left(27).wrapping_mul(0x94d0_49bb_1331_11eb);
    }
    Quat::from_rotation_y((hash & 3) as f32 * std::f32::consts::FRAC_PI_2)
        * if hash & 4 == 0 {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_x(std::f32::consts::PI)
        }
}

pub(super) fn icon_color(heraldry: KingdomHeraldry) -> [f32; 4] {
    let [red, green, blue] = heraldry.color.map(f32::from);
    if red * 0.2126 + green * 0.7152 + blue * 0.0722 > 164.0 {
        crate::dye_tint::rgb([45, 33, 19])
    } else {
        crate::dye_tint::rgb([255, 226, 156])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn templates() -> crate::blocks::TemplateSet {
        use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
        let mut templates = crate::blocks::TemplateSet::default();
        templates.register_castle_material("heraldry_cloth", 0);
        for (index, icon) in terra_core::kingdom::KingdomIcon::ALL
            .into_iter()
            .enumerate()
        {
            templates.register_castle_material(&format!("heraldry_icon_{}", icon.key()), index + 1);
        }
        let model = (0..10)
            .map(|material| PrimitiveTemplate {
                positions: vec![[0.2, 0.3, 0.006], [0.5, 0.3, 0.006], [0.5, 0.6, 0.006]],
                normals: vec![[0.0, 0.0, -1.0]; 3],
                tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
                uvs: vec![[0.0; 2]; 3],
                indices: vec![0, 1, 2],
                material,
            })
            .collect();
        templates.blocks.insert(
            terra_core::castle::ROYAL_WALL,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [9; 6],
                side_cap: None,
                details: Vec::new(),
                model,
            },
        );
        templates
    }

    #[test]
    fn castle_heraldry_uses_actual_claims_updates_live_and_never_changes_other_realms_materials() {
        use terra_core::kingdom::{KingdomIcon, SettlementKind};
        let mut registry = KingdomRegistry::default();
        let owners = [
            terra_core::PlayerUuid::from_u128(71),
            terra_core::PlayerUuid::from_u128(72),
        ];
        let styles = [
            KingdomHeraldry {
                color: [18, 57, 190],
                icon: KingdomIcon::Tree,
            },
            KingdomHeraldry {
                color: [205, 30, 40],
                icon: KingdomIcon::Hammer,
            },
        ];
        let first = registry
            .found_settlement(
                owners[0],
                "Alice",
                [-1, 20, 0],
                "West",
                SettlementKind::Kingdom,
                styles[0],
            )
            .unwrap();
        registry
            .found_settlement(
                owners[1],
                "Bob",
                [1, 20, 0],
                "East",
                SettlementKind::Kingdom,
                styles[1],
            )
            .unwrap();
        let templates = templates();
        let make_snapshot = |section| {
            let mut blocks = terra_voxel::BlockSection::empty(section).unwrap();
            blocks
                .set_block([1, 1, 1], terra_core::castle::ROYAL_WALL)
                .unwrap();
            super::super::capture(&blocks, &templates, &Default::default(), |_| None).unwrap()
        };
        let sections = [SectionPos::new(-1, 8, 0), SectionPos::new(0, 8, 0)];
        let mut snapshots = sections.map(make_snapshot);
        let options = super::super::RenderOptions {
            water_level: None,
            detail_level: crate::meshing::DetailLevel::Near,
            terrain: None,
            exposure: None,
            conditions: Default::default(),
            tints: Default::default(),
        };
        for ((snapshot, section), style) in snapshots.iter_mut().zip(sections).zip(styles) {
            assert!(snapshot.heraldry_changed(section, &registry));
            snapshot.refresh_heraldry(section, &registry);
            assert!(!snapshot.heraldry_changed(section, &registry));
            assert_eq!(snapshot.castle_heraldry([1; 3]), style);
            let geometry =
                super::super::compose(&Default::default(), snapshot, &templates, section, &options)
                    .unwrap();
            assert_eq!(
                geometry.len(),
                3,
                "one cloth, selected emblem and uncolored stone"
            );
            assert!(
                geometry
                    .iter()
                    .find(|batch| batch.material == 0)
                    .unwrap()
                    .colors
                    .iter()
                    .all(|color| *color == crate::dye_tint::rgb(style.color))
            );
            let icons: Vec<_> = geometry
                .iter()
                .filter_map(|batch| templates.castle_icon_materials.get(&batch.material))
                .copied()
                .collect();
            assert_eq!(icons, [style.icon]);
            assert!(
                geometry
                    .iter()
                    .find(|batch| batch.material == 9)
                    .unwrap()
                    .colors
                    .is_empty()
            );
        }
        let replacement = KingdomHeraldry {
            color: [240, 220, 190],
            icon: KingdomIcon::Crown,
        };
        registry
            .change_appearance(owners[0], first, replacement)
            .unwrap();
        assert!(snapshots[0].heraldry_changed(sections[0], &registry));
        assert!(!snapshots[1].heraldry_changed(sections[1], &registry));
        snapshots[0].refresh_heraldry(sections[0], &registry);
        assert_eq!(snapshots[0].castle_heraldry([1; 3]), replacement);
        registry.dissolve(owners[0], first).unwrap();
        assert!(snapshots[0].heraldry_changed(sections[0], &registry));
        snapshots[0].refresh_heraldry(sections[0], &registry);
        assert_eq!(
            snapshots[0].castle_heraldry([1; 3]),
            KingdomHeraldry::default()
        );
        assert_eq!(snapshots[1].castle_heraldry([1; 3]), styles[1]);
        // Adjacent unclaimed chunks do not inherit the nearest settlement.
        let outside = SectionPos::new(1, 8, 0);
        let mut unclaimed = make_snapshot(outside);
        unclaimed.refresh_heraldry(outside, &registry);
        assert_eq!(
            unclaimed.castle_heraldry([1; 3]),
            KingdomHeraldry::default()
        );
    }

    #[test]
    fn castle_arch_cut_faces_require_a_matching_neighbor_in_all_twenty_four_poses() {
        for facing in 0..24 {
            for material in [
                terra_core::castle::ARCH_LEFT,
                terra_core::castle::ARCH_RIGHT,
            ] {
                let mut model = ModelCell {
                    cell: [0; 3],
                    material,
                    state: terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    },
                    peer_wood: Default::default(),
                    neighbors: [None; 6],
                    neighbor_facing: [0; 6],
                };
                assert!(!paired_arch(&model));
                let direction = terra_core::construction::stair_basis(facing)[0];
                let sign = if material == terra_core::castle::ARCH_LEFT {
                    1
                } else {
                    -1
                };
                let axis = direction
                    .iter()
                    .position(|coordinate| *coordinate != 0)
                    .unwrap();
                let side = axis * 2 + usize::from(direction[axis] * sign > 0);
                model.neighbors[side] = Some(if material == terra_core::castle::ARCH_LEFT {
                    terra_core::castle::ARCH_RIGHT
                } else {
                    terra_core::castle::ARCH_LEFT
                });
                model.neighbor_facing[side] = facing;
                assert!(paired_arch(&model), "missing arch join {material}/{facing}");
                model.neighbor_facing[side] = (facing + 1) % 24;
                assert!(
                    !paired_arch(&model),
                    "different poses must retain their end cap"
                );
            }
        }
    }

    #[test]
    fn castle_detail_variants_are_stable_across_section_coordinates_and_preserve_bounds() {
        let position = SectionPos::new(-2, 3, 8);
        let a = detail_rotation(position, [16, 2, 4]);
        let b = detail_rotation(SectionPos::new(-1, 3, 8), [0, 2, 4]);
        assert_eq!(a, b);
        let variants: std::collections::HashSet<_> = (0..64)
            .map(|x| {
                let point =
                    detail_rotation(position, [x, 2, 4]) * bevy::math::Vec3::new(0.2, 0.3, 0.4);
                point
                    .to_array()
                    .map(|coordinate| (coordinate * 1000.0).round() as i32)
            })
            .collect();
        assert_eq!(variants.len(), 8);
        for x in 0..16 {
            let rotation = detail_rotation(position, [x, 2, 4]);
            for corner in 0..8 {
                let point = rotation
                    * bevy::math::Vec3::new(
                        if corner & 1 == 0 { -0.5 } else { 0.5 },
                        if corner & 2 == 0 { -0.5 } else { 0.5 },
                        if corner & 4 == 0 { -0.5 } else { 0.5 },
                    )
                    + bevy::math::Vec3::splat(0.5);
                assert!(
                    point
                        .to_array()
                        .into_iter()
                        .all(|coordinate| (-0.00001..=1.00001).contains(&coordinate))
                );
            }
        }
    }
    #[test]
    fn generated_village_banners_fall_back_to_native_and_follow_capture_and_live_colors() {
        use terra_core::{
            PlayerUuid,
            kingdom::{BotKingdom, DomainKind, KingdomIcon, SettlementDomain},
        };
        let section = SectionPos::new(2, 1, 2);
        let mut blocks = terra_voxel::BlockSection::empty(section).unwrap();
        blocks
            .set_block([1; 3], terra_core::castle::ROYAL_WALL)
            .unwrap();
        let mut snapshot =
            super::super::capture(&blocks, &templates(), &Default::default(), |_| None).unwrap();
        let domain = SettlementDomain {
            id: 8,
            center: [0, 20, 0],
            bounds: [[-32, 0, -32], [63, 80, 63]],
            kind: DomainKind::Kingdom,
            native: Some(BotKingdom::Lutera),
        };
        snapshot.set_domains(vec![domain]);
        let mut registry = KingdomRegistry::default();
        snapshot.refresh_heraldry(section, &registry);
        assert_eq!(
            snapshot.castle_heraldry([1; 3]),
            BotKingdom::Lutera.heraldry()
        );
        let owner = PlayerUuid::from_u128(31);
        let id = registry
            .found(owner, domain.center, "Captured Keep")
            .unwrap();
        assert!(snapshot.heraldry_changed(section, &registry));
        let changed = KingdomHeraldry {
            color: [198, 21, 93],
            icon: KingdomIcon::Anchor,
        };
        registry.change_appearance(owner, id, changed).unwrap();
        snapshot.refresh_heraldry(section, &registry);
        assert_eq!(snapshot.castle_heraldry([1; 3]), changed);
        registry.dissolve(owner, id).unwrap();
        assert!(snapshot.heraldry_changed(section, &registry));
        snapshot.refresh_heraldry(section, &registry);
        assert_eq!(
            snapshot.castle_heraldry([1; 3]),
            BotKingdom::Lutera.heraldry()
        );
    }
}
