//! Alchemy shares the existing two effect meshes and local fire-light budget.
use super::*;
use terra_network::protocol::session::{CampfireVisual, CauldronVisual};
mod liquid;

pub(super) struct Nearby<'a> {
    fires: Vec<&'a CampfireVisual>,
    attached_fires: BTreeSet<[i64; 3]>,
    bowls: Vec<&'a CauldronVisual>,
}

pub(super) fn nearby<'a>(
    session: Option<&'a ClientSession>,
    chunks: &ChunkStore,
    journal: &EditJournal,
    eye: Vec3,
    fire_slots: usize,
) -> Nearby<'a> {
    let valid = |position, material| {
        chunks
            .block_at(position)
            .is_some_and(|raw| journal.block(position).unwrap_or(raw) == material)
            && Vec3::from_array(position.map(|v| v as f32)).distance_squared(eye) < 96.0_f32.powi(2)
    };
    let mut fires: Vec<_> = session
        .into_iter()
        .flat_map(|s| &s.campfires)
        .filter(|fire| {
            fire.lit
                && (valid(fire.position, 162)
                    || valid(fire.position, 161)
                        && journal
                            .object_state(fire.position)
                            .is_some_and(|state| terra_core::brewing::has_fire(state.facing)))
        })
        .collect();
    let mut bowls: Vec<_> = session
        .into_iter()
        .flat_map(|s| &s.cauldrons)
        .filter(|bowl| (bowl.water > 0 || bowl.spoon.is_some()) && valid(bowl.position, 161))
        .collect();
    let distance = |p: [i64; 3]| Vec3::from_array(p.map(|v| v as f32)).distance_squared(eye);
    fires.sort_by(|a, b| distance(a.position).total_cmp(&distance(b.position)));
    bowls.sort_by(|a, b| distance(a.position).total_cmp(&distance(b.position)));
    fires.truncate(fire_slots.min(MAX_FIRES));
    bowls.truncate(MAX_FIRES);
    let attached_fires = fires
        .iter()
        .filter(|fire| valid(fire.position, 161))
        .map(|fire| fire.position)
        .collect();
    Nearby {
        fires,
        attached_fires,
        bowls,
    }
}

impl Nearby<'_> {
    pub(super) fn active(&self) -> bool {
        !self.fires.is_empty() || !self.bowls.is_empty()
    }

    pub(super) fn fire_cells(&self) -> impl Iterator<Item = [i64; 3]> + '_ {
        self.fires.iter().map(|fire| fire.position)
    }

    pub(super) fn retain_fires(&mut self, cells: &BTreeSet<[i64; 3]>) {
        self.fires.retain(|fire| cells.contains(&fire.position));
    }

    pub(super) fn lights(&self) -> impl Iterator<Item = ([i64; 3], Vec3)> + '_ {
        self.fires.iter().map(|fire| {
            (
                fire.position,
                Vec3::from_array(fire.position.map(|v| v as f32))
                    + Vec3::new(
                        0.5,
                        if self.attached_fires.contains(&fire.position) {
                            0.27
                        } else {
                            0.42
                        },
                        0.5,
                    ),
            )
        })
    }

    pub(super) fn append(
        &self,
        fire: &mut Geometry,
        smoke: &mut Geometry,
        world: (&ChunkStore, &EditJournal),
        view: (f32, Vec3, Vec3),
    ) {
        let (chunks, journal) = world;
        let (time, right, up) = view;
        let block = |p| {
            chunks
                .block_at(p)
                .map(|raw| journal.block(p).unwrap_or(raw))
        };
        for source in &self.fires {
            let anchor = Vec3::from_array(source.position.map(|v| v as f32));
            let attached = self.attached_fires.contains(&source.position);
            let covered = attached
                || block([
                    source.position[0],
                    source.position[1] + 1,
                    source.position[2],
                ]) == Some(161);
            let scale = if attached {
                terra_world::interaction_shapes::ATTACHED_FIRE_SCALE as f32
            } else {
                1.0
            };
            for index in 0..7 {
                let phase = time * 7.7 + index as f32 * 1.9;
                let angle = index as f32 * 2.4;
                let flicker = phase.sin() * 0.5 + 0.5;
                let height = if attached {
                    0.09 + flicker * 0.05
                } else {
                    (0.23 + flicker * 0.28) * if covered { 2.15 } else { 1.0 }
                };
                let center = anchor
                    + Vec3::new(
                        0.5 + angle.cos() * 0.12 * scale,
                        0.20 + height * 0.5,
                        0.5 + angle.sin() * 0.12 * scale,
                    );
                fire.cuboid(
                    center,
                    Vec3::new(0.041 * scale, height * 0.5, 0.041 * scale),
                    Quat::from_rotation_y(angle),
                    [3.0, 0.5, 0.035, 1.0],
                );
                fire.cuboid(
                    center - Vec3::Y * height * 0.20,
                    Vec3::new(0.022 * scale, height * 0.28, 0.022 * scale),
                    Quat::IDENTITY,
                    [5.0, 2.1, 0.12, 1.0],
                );
            }
            if !covered {
                for index in 0..8 {
                    let age = (time * 0.26 + index as f32 / 8.0).fract();
                    let center = anchor
                        + Vec3::new(
                            0.5 + (time + index as f32).sin() * age * 0.12,
                            0.45 + age * 2.4,
                            0.5,
                        );
                    let cell = center.floor().as_i64vec3().to_array();
                    if !block(cell).is_some_and(terra_world::furnaces::smoke_passable) {
                        continue;
                    }
                    smoke.wisp(
                        center,
                        0.10 + age * 0.22,
                        right,
                        up,
                        [0.22, 0.205, 0.19, (1.0 - age) * 0.25],
                    );
                    smoke.confine_last_wisp(cell, |p| {
                        block(p).is_some_and(terra_world::furnaces::smoke_passable)
                    });
                }
            }
        }
        for bowl in &self.bowls {
            let anchor = Vec3::from_array(bowl.position.map(|v| v as f32));
            let heated = bowl.heated
                && (journal.has_attached_campfire(bowl.position)
                    || bowl.position[1].checked_sub(1).is_some_and(|y| {
                        block([bowl.position[0], y, bowl.position[2]]) == Some(162)
                    }));
            append_spoon(fire, bowl, anchor, time);
            if bowl.water == 0 {
                continue;
            }
            let rgb = bowl
                .recipe
                .and_then(terra_core::brewing::recipe)
                .map_or([0.15, 0.47, 0.78], |r| {
                    r.color.map(|v| f32::from(v) / 255.0)
                });
            let mut color = Color::srgb_from_array(rgb).to_linear().to_f32_array();
            if bowl.brewing {
                for channel in &mut color[..3] {
                    *channel *= 1.35;
                }
            }
            liquid::append(fire, anchor, bowl.water, color, time, heated);
            if heated {
                for bubble in 0..4 {
                    let age = (time * 0.55 + bubble as f32 / 4.0).fract();
                    let angle = bubble as f32 * 2.4 + time * 0.3;
                    let center = anchor
                        + Vec3::new(
                            0.5 + angle.cos() * 0.17,
                            0.96 + age * 0.50,
                            0.5 + angle.sin() * 0.17,
                        );
                    smoke.wisp(
                        center,
                        0.018 + age * 0.020,
                        right,
                        up,
                        [rgb[0], rgb[1], rgb[2], (1.0 - age) * 0.52],
                    );
                }
            }
        }
    }
}

fn append_spoon(target: &mut Geometry, bowl: &CauldronVisual, anchor: Vec3, time: f32) {
    use terra_core::{ItemKind, ToolKind};
    static SPOONS: std::sync::LazyLock<[crate::meshing::builder::MeshBatch; 3]> =
        std::sync::LazyLock::new(|| {
            [
                ToolKind::WoodSpoon,
                ToolKind::StoneSpoon,
                ToolKind::GoldSpoon,
            ]
            .map(|tool| {
                crate::previews::tools::batches(ItemKind::Tool(tool), 0)
                    .expect("built-in spoon geometry")
                    .remove(0)
            })
        });
    let index = match bowl.spoon {
        Some(ToolKind::WoodSpoon) => 0,
        Some(ToolKind::StoneSpoon) => 1,
        Some(ToolKind::GoldSpoon) => 2,
        _ => return,
    };
    let mesh = &SPOONS[index];
    let yaw = Quat::from_rotation_y(if bowl.brewing { time * 0.8 } else { 0.35 });
    let tilt = Quat::from_rotation_z(std::f32::consts::PI - 0.4);
    let pivot_height = liquid_height(bowl.water.max(1)) + 0.50;
    let base = target.positions.len() as u32;
    target.positions.extend(mesh.positions.iter().map(|p| {
        (anchor
            + Vec3::new(0.5, 0.0, 0.5)
            + yaw
                * (Vec3::new(0.20, pivot_height, 0.0)
                    + tilt * (Vec3::from_array(*p) - Vec3::new(0.5, 0.0, 0.5)) * 0.60))
            .to_array()
    }));
    target.normals.extend(
        mesh.normals
            .iter()
            .map(|normal| (yaw * tilt * Vec3::from_array(*normal)).to_array()),
    );
    target.colors.extend_from_slice(&mesh.colors);
    target.uvs.extend_from_slice(&mesh.uvs);
    target
        .indices
        .extend(mesh.indices.iter().map(|index| base + index));
}

fn liquid_height(water: u8) -> f32 {
    0.48 + f32::from(water.min(3)) * 0.12
}
fn liquid_radius(height: f32) -> f32 {
    let rows = [
        (0.38, 0.20),
        (0.46, 0.30),
        (0.57, 0.37),
        (0.68, 0.38),
        (0.78, 0.35),
        (0.87, 0.32),
    ];
    rows.windows(2)
        .find(|pair| height <= pair[1].0)
        .map_or(0.29, |pair| {
            let t = ((height - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0);
            pair[0].1 + (pair[1].1 - pair[0].1) * t - 0.031
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unchanged_cold_brew_never_republishes_its_shared_mesh_asset() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>();
        let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh());
        let geometry = || {
            let mut geometry = Geometry::default();
            geometry.cuboid(
                Vec3::ZERO,
                Vec3::splat(0.1),
                Quat::IDENTITY,
                [0.3, 0.1, 0.5, 1.0],
            );
            geometry
        };
        assert!(geometry().upload(&mut app.world_mut().resource_mut::<Assets<Mesh>>(), &handle));
        app.update();
        let mut cursor = app
            .world()
            .resource::<Messages<AssetEvent<Mesh>>>()
            .get_cursor_current();
        for _ in 0..4 {
            assert!(
                geometry().upload(&mut app.world_mut().resource_mut::<Assets<Mesh>>(), &handle)
            );
            app.update();
            assert_eq!(cursor.read(app.world().resource()).count(), 0);
        }
        let mut changed = geometry();
        changed.colors[0][0] = 0.7;
        assert!(changed.upload(&mut app.world_mut().resource_mut::<Assets<Mesh>>(), &handle));
        app.update();
        assert_eq!(
            cursor
                .read(app.world().resource())
                .filter(|event| matches!(event,AssetEvent::Modified{id}if *id==handle.id()))
                .count(),
            1
        );
        assert!(
            !Geometry::default()
                .upload(&mut app.world_mut().resource_mut::<Assets<Mesh>>(), &handle)
        );
        app.update();
        assert_eq!(cursor.read(app.world().resource()).count(), 0);
    }

    fn put(chunks: &mut ChunkStore, position: [i64; 3], material: u16) {
        let (section, local) = terra_voxel::global_to_section(position);
        chunks
            .loaded
            .entry(section)
            .or_insert_with(|| terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, material)
            .unwrap();
    }

    #[test]
    fn replicated_fire_and_bowl_geometry_follow_real_loaded_state_without_extra_entities() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let bowl = [-1, 1, -1];
        let lit = [-2, 0, -1];
        let cold = [-3, 0, -1];
        for (position, material) in [(bowl, 161), (lit, 162), (cold, 162)] {
            put(&mut chunks, position, material);
        }
        let mut session = ClientSession::default();
        session.campfires = vec![
            CampfireVisual {
                position: lit,
                lit: true,
            },
            CampfireVisual {
                position: cold,
                lit: false,
            },
        ];
        session.cauldrons = vec![CauldronVisual {
            position: bowl,
            water: 3,
            heated: true,
            brewing: true,
            recipe: Some(1),
            spoon: Some(terra_core::ToolKind::GoldSpoon),
        }];
        let sources = nearby(Some(&session), &chunks, &journal, Vec3::ZERO, MAX_FIRES);
        assert_eq!(sources.fire_cells().collect::<Vec<_>>(), [lit]);
        assert_eq!(sources.lights().count(), 1);
        let mut fire = Geometry::default();
        let mut smoke = Geometry::default();
        sources.append(
            &mut fire,
            &mut smoke,
            (&chunks, &journal),
            (1.0, Vec3::X, Vec3::Y),
        );
        assert!(!fire.positions.is_empty() && !smoke.positions.is_empty());
        assert!(fire.positions.len() < 2048 && smoke.positions.len() < 128);
        for triangle in fire.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|index| Vec3::from_array(fire.positions[index as usize]));
            assert!(
                (b - a)
                    .cross(c - a)
                    .dot(Vec3::from_array(fire.normals[triangle[0] as usize]))
                    > 0.0
            );
        }
        assert!(
            fire.positions
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
        // An authoritative removal overrides stale visual packets immediately.
        journal.record(lit, 0).unwrap();
        assert_eq!(
            nearby(Some(&session), &chunks, &journal, Vec3::ZERO, MAX_FIRES)
                .lights()
                .count(),
            0
        );
        session.cauldrons[0].water = 0;
        session.cauldrons[0].spoon = None;
        assert!(!nearby(Some(&session), &chunks, &journal, Vec3::ZERO, MAX_FIRES).active());
        chunks.loaded.clear();
        assert!(
            !nearby(
                Some(&session),
                &chunks,
                &EditJournal::default(),
                Vec3::ZERO,
                MAX_FIRES
            )
            .active()
        );
    }

    #[test]
    fn nearest_effect_sources_obey_fixed_fire_and_bowl_caps() {
        let mut chunks = ChunkStore::default();
        let mut session = ClientSession::default();
        for x in (0..64).rev() {
            let position = [x, 0, 0];
            put(&mut chunks, position, 162);
            session.campfires.push(CampfireVisual {
                position,
                lit: true,
            });
            let position = [x, 1, 0];
            put(&mut chunks, position, 161);
            session.cauldrons.push(CauldronVisual {
                position,
                water: 1,
                heated: false,
                brewing: false,
                recipe: None,
                spoon: None,
            });
        }
        let sources = nearby(
            Some(&session),
            &chunks,
            &EditJournal::default(),
            Vec3::ZERO,
            MAX_FIRES,
        );
        assert_eq!(
            sources.fire_cells().collect::<Vec<_>>(),
            (0..MAX_FIRES as i64).map(|x| [x, 0, 0]).collect::<Vec<_>>()
        );
        assert_eq!(sources.lights().count(), MAX_FIRES);
        assert_eq!(sources.bowls.len(), MAX_FIRES);
    }

    #[test]
    fn all_three_water_levels_fit_inside_the_authored_open_bowl() {
        let mut previous = 0.0;
        for water in 1..=3 {
            let height = liquid_height(water);
            let radius = liquid_radius(height);
            assert!(height > previous && height < 0.87);
            assert!((0.28..0.36).contains(&radius));
            previous = height;
        }
        assert_eq!(liquid_height(u8::MAX), liquid_height(3));
    }

    #[test]
    fn every_spoon_stays_dipped_at_one_to_three_portions_and_clears_the_rim() {
        for spoon in [
            terra_core::ToolKind::WoodSpoon,
            terra_core::ToolKind::StoneSpoon,
            terra_core::ToolKind::GoldSpoon,
        ] {
            for water in 1..=3 {
                let height = liquid_height(water);
                for time in [0.0, 1.0, 3.0] {
                    let mut geometry = Geometry::default();
                    append_spoon(
                        &mut geometry,
                        &CauldronVisual {
                            position: [0; 3],
                            water,
                            heated: true,
                            brewing: true,
                            recipe: Some(1),
                            spoon: Some(spoon),
                        },
                        Vec3::ZERO,
                        time,
                    );
                    let bottom = geometry
                        .positions
                        .iter()
                        .map(|point| point[1])
                        .fold(f32::INFINITY, f32::min);
                    assert!((height - 0.08..height - 0.03).contains(&bottom));
                    assert!(bottom > 0.39, "the bowl bottom stays clear");
                    for point in &geometry.positions {
                        let radius = Vec2::new(point[0] - 0.5, point[2] - 0.5).length();
                        if (0.865..=0.95).contains(&point[1]) {
                            assert!(
                                radius < 0.303,
                                "the stirrer never intersects the rolled rim"
                            );
                        }
                        if point[1] < 0.87 {
                            assert!(radius < liquid_radius(point[1]));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cold_filled_cauldron_keeps_the_actual_shared_asset_unchanged_across_effect_ticks() {
        let mut chunks = ChunkStore::default();
        let position = [-1, 0, -1];
        put(&mut chunks, position, 161);
        let journal = EditJournal::default();
        let mut session = ClientSession::default();
        session.cauldrons.push(CauldronVisual {
            position,
            water: 3,
            heated: false,
            brewing: false,
            recipe: Some(1),
            spoon: Some(terra_core::ToolKind::WoodSpoon),
        });
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>();
        let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh());
        let mut cursor = app
            .world()
            .resource::<Messages<AssetEvent<Mesh>>>()
            .get_cursor_current();
        for (index, time) in [0.0, 0.05, 0.2, 10.0].into_iter().enumerate() {
            let mut liquid = Geometry::default();
            nearby(Some(&session), &chunks, &journal, Vec3::ZERO, MAX_FIRES).append(
                &mut liquid,
                &mut Geometry::default(),
                (&chunks, &journal),
                (time, Vec3::X, Vec3::Y),
            );
            assert!(liquid.upload(&mut app.world_mut().resource_mut::<Assets<Mesh>>(), &handle));
            app.update();
            let changes = cursor
                .read(app.world().resource())
                .filter(|event| matches!(event,AssetEvent::Modified{id}if *id==handle.id()))
                .count();
            assert_eq!(changes, usize::from(index == 0));
        }
    }

    #[test]
    fn compact_confirmed_fire_and_boiling_water_stay_in_one_cell_without_a_recipe() {
        let position = [-16, -1, -16];
        let anchor = Vec3::from_array(position.map(|v| v as f32));
        let mut chunks = ChunkStore::default();
        put(&mut chunks, position, 161);
        let mut journal = EditJournal::default();
        journal.record(position, 161).unwrap();
        journal
            .set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    facing: terra_core::brewing::ATTACHED_FIRE,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut session = ClientSession::default();
        session.campfires.push(CampfireVisual {
            position,
            lit: true,
        });
        session.cauldrons.push(CauldronVisual {
            position,
            water: 1,
            heated: true,
            brewing: false,
            recipe: None,
            spoon: None,
        });
        let sources = nearby(Some(&session), &chunks, &journal, anchor, MAX_FIRES);
        assert_eq!(sources.fire_cells().collect::<Vec<_>>(), [position]);
        assert!((sources.lights().next().unwrap().1.y - anchor.y - 0.27).abs() < 1e-5);
        let mut fire = Geometry::default();
        let mut smoke = Geometry::default();
        sources.append(
            &mut fire,
            &mut smoke,
            (&chunks, &journal),
            (1.0, Vec3::X, Vec3::Y),
        );
        assert!(
            !smoke.positions.is_empty(),
            "heated plain water actually steams"
        );
        assert_eq!(fire.positions.len(), 7 * 2 * 24 + 167);
        for point in &fire.positions[..7 * 2 * 24] {
            assert!(point[1] >= anchor.y + 0.199 && point[1] <= anchor.y + 0.341);
        }
        assert!(fire.positions.iter().all(|point| {
            (0..3).all(|axis| point[axis] >= anchor[axis] && point[axis] < anchor[axis] + 1.0)
        }));
        // A later geometry delta removes the fire immediately, even if its visual packet is stale.
        journal
            .set_object_state(position, Some(Default::default()))
            .unwrap();
        let sources = nearby(Some(&session), &chunks, &journal, anchor, MAX_FIRES);
        assert_eq!(sources.lights().count(), 0);
        let mut fire = Geometry::default();
        let mut smoke = Geometry::default();
        sources.append(
            &mut fire,
            &mut smoke,
            (&chunks, &journal),
            (2.0, Vec3::X, Vec3::Y),
        );
        assert!(smoke.positions.is_empty());
        assert_eq!(fire.positions.len(), 65, "only the still liquid remains");
    }
}
