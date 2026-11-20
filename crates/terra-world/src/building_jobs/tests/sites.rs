use super::*;
use crate::props::{PropId, PropKind, WorldProp};

fn visitor(kind: CreatureKind, slot: u8, position: [f64; 3]) -> Creature {
    let home = [0, 1];
    let creature = Creature {
        id: if kind == CreatureKind::Villager {
            villagers::id(home, u16::from(slot)).unwrap()
        } else {
            terra_core::creatures::creature_id(home, slot).unwrap()
        },
        kind,
        home,
        position,
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: if kind == CreatureKind::Villager {
            villagers::UNEMPLOYED
        } else {
            0
        },
        needs: CreatureNeeds::full(kind),
    };
    assert!(creature.valid());
    creature
}

#[test]
fn blueprint_ignores_mobile_npcs_mobs_and_natural_grass_props_without_moving_them() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(0);
    for (kind, position) in [
        (CreatureKind::Villager, [8.5, 12.0, 24.5]),
        (CreatureKind::Bear, [8.5, 12.0, 24.5]),
    ] {
        let creature = visitor(kind, 0, position);
        journal
            .gameplay_mut()
            .creatures
            .animals
            .insert(creature.id, creature);
    }
    let section = global_to_section([8, 11, 24]).0;
    for (index, kind) in [PropKind::Grass, PropKind::Flower].into_iter().enumerate() {
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .props
            .push(WorldProp {
                id: PropId(100 + index as u64),
                kind,
                position: [8.5 + index as f64, 12.0, 24.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section,
                removed: false,
            });
    }
    let before = journal.gameplay().clone();
    let props = chunks.loaded[&section].props.clone();
    for facing in 0..4 {
        // The rotated working ring also stays within the same claimed chunk.
        let job = plan_house(
            &chunks,
            &journal,
            &terrain,
            realm,
            owner,
            [8, 12, 24],
            facing,
        )
        .unwrap();
        assert!(job.valid());
    }
    assert_eq!(journal.gameplay(), &before);
    assert_eq!(chunks.loaded[&section].props, props);
    assert_eq!(chunks.block_at([8, 12, 24]), Some(0));
}

#[test]
fn explicit_air_and_one_block_natural_terraces_generate_real_persisted_preparation() {
    for material in [1, 2, 3, 4, 5, 11, 13, 19, 42, 48] {
        let (mut chunks, mut journal, terrain, realm, owner) = fixture(0);
        let raised = [4, 12, 24];
        let cleared = [3, 11, 22];
        set(&mut chunks, raised, material);
        set(&mut chunks, cleared, 0);
        journal.record(cleared, 0).unwrap();
        let before = journal.gameplay().clone();
        let edits = journal.edited_blocks();
        let job = plan_house(&chunks, &journal, &terrain, realm, owner, [8, 12, 24], 0).unwrap();
        assert_eq!(job.steps[0].phase, BuildPhase::Foundation);
        assert_eq!(job.steps[0].cells[0].position, raised);
        assert_eq!(job.steps[0].cells[0].material, 0);
        assert!(job.steps[0].costs.is_empty());
        let saved: BuildingJob =
            serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
        assert_eq!(saved.steps, job.steps);
        assert_eq!(chunks.block_at(raised), Some(material));
        assert_eq!(journal.edited_blocks(), edits);
        assert_eq!(journal.gameplay(), &before);
    }
}

#[test]
fn solid_buildings_trees_player_terrain_and_water_still_block_blueprints() {
    for (position, material, edited) in [
        ([4, 12, 24], 6, false),
        ([4, 12, 24], 7, false),
        ([4, 12, 24], 16, false),
        ([4, 12, 24], 29, false),
        ([4, 12, 24], 43, false),
        // Generated foundation stone stays protected without a journal entry.
        ([4, 11, 24], 43, false),
        ([4, 11, 24], terra_core::farming::FARMLAND, false),
        ([4, 11, 24], terra_voxel::MaterialId::Path as u16, false),
        ([4, 12, 24], 1, true),
        // Saved stone is an existing floor, unlike saved grass/dirt below feet.
        ([4, 11, 24], 3, true),
        ([4, 12, 24], 27, false),
        // More than four natural blocks exceeds bounded terrace preparation.
        ([4, 16, 24], 1, false),
    ] {
        let (mut chunks, mut journal, terrain, realm, owner) = fixture(0);
        set(&mut chunks, position, material);
        if edited {
            journal.record(position, material).unwrap();
        }
        let before = journal.gameplay().clone();
        let edits = journal.edited_blocks();
        assert_eq!(
            plan_house(&chunks, &journal, &terrain, realm, owner, [8, 12, 24], 0),
            Err(BuildError::Blocked),
            "obstacle at {position:?}, material {material}, edited {edited}"
        );
        assert_eq!(journal.gameplay(), &before);
        assert_eq!(journal.edited_blocks(), edits);
        assert_eq!(chunks.block_at(position), Some(material));
    }
    let (chunks, mut journal, terrain, realm, owner) = fixture(0);
    let position = [4, 12, 24];
    journal
        .set_fluid(
            position,
            crate::water::FluidCell {
                amount: 8,
                flow: [0; 2],
                source: true,
            },
        )
        .unwrap();
    assert_eq!(
        plan_house(&chunks, &journal, &terrain, realm, owner, [8, 12, 24], 0),
        Err(BuildError::Blocked)
    );
    assert_eq!(journal.fluid(position).unwrap().amount, 8);
}

#[test]
fn occupants_only_pause_real_construction_and_no_material_is_spent_until_they_leave() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&worker)
        .unwrap()
        .position = [7.5, 12.0, 24.5];
    let mob = visitor(CreatureKind::Bear, 0, [8.5, 12.0, 24.5]);
    let mob_id = mob.id;
    journal.gameplay_mut().creatures.animals.insert(mob_id, mob);
    plank(&mut journal, worker);
    let id = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, vec![worker], [8, 12, 24]))
        .unwrap();
    let mut runtime = BuildRuntime::default();
    let properties = BlockPropertiesCatalog::default();
    let before = totals(&journal);
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Waiting(BuildWait::Blocked)
    );
    assert_eq!(journal.gameplay().buildings.jobs[&id].work_seconds, 0.0);
    assert_eq!(chunks.block_at([8, 12, 24]), Some(0));
    assert_eq!(totals(&journal), before);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&mob_id)
        .unwrap()
        .position = [12.5, 12.0, 28.5];
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert!(journal.gameplay().buildings.jobs[&id].work_seconds > 1.9);
    assert_eq!(totals(&journal), before);
}

#[test]
fn a_real_craftsman_prepares_a_clay_terrace_and_physically_finishes_the_house() {
    build_full_house_with_preparation(0, HOUSE_TEMPLATE, false, true, SitePreparation::ClayTerrace);
}

#[test]
fn a_valid_custom_floor_at_ground_level_is_prepared_once_and_physically_built() {
    use terra_core::building_jobs::template::{
        BuildingTemplate, TEMPLATE_SCHEMA, TemplateCell, TemplateStep,
    };

    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let raised = [4, 12, 24];
    set(&mut chunks, raised, 19);
    let costs = vec![ItemStack {
        item: ItemKind::Block(3),
        count: 1,
    }];
    let template = BuildingTemplate {
        schema_version: TEMPLATE_SCHEMA,
        id: "custom-ground-floor".into(),
        display_name: "Custom ground floor".into(),
        category: Default::default(),
        anchor: [0; 3],
        bounds: BuildArea::new([-4, 0, 0], [0; 3]).unwrap(),
        steps: vec![TemplateStep {
            phase: BuildPhase::Floor,
            cells: vec![TemplateCell {
                position: [-4, 0, 0],
                material: 3,
                facing: 0,
                rotate_facing: false,
                peer: None,
                secondary: false,
            }],
            costs: costs.clone(),
        }],
        materials: costs,
    };
    // Exercise the same validated JSON format used by the standalone builder.
    let template = BuildingTemplate::from_json(&serde_json::to_string(&template).unwrap()).unwrap();
    let mut job = planning::plan_resolved(
        &chunks,
        &journal,
        &terrain,
        realm,
        owner,
        [8, 12, 24],
        0,
        &template,
    )
    .unwrap();
    assert_eq!(
        job.steps
            .iter()
            .flat_map(|step| &step.cells)
            .filter(|cell| cell.position == raised && cell.material == 0)
            .count(),
        1,
        "the ground-level floor and terrace share one excavation"
    );
    job.free_resources = true;
    let job: BuildingJob = serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
    assert!(job.valid());
    let floor_step = job
        .steps
        .iter()
        .position(|step| {
            step.phase == BuildPhase::Floor
                && step
                    .cells
                    .iter()
                    .any(|cell| cell.position == raised && cell.material == 3)
        })
        .unwrap();
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let mut runtime = BuildRuntime::default();
    runtime.set_creative_owners([owner]);
    let properties = BlockPropertiesCatalog::default();
    for _ in 0..5_000 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
        if journal.gameplay().buildings.jobs[&id].next_step > floor_step {
            break;
        }
    }
    let built = &journal.gameplay().buildings.jobs[&id];
    assert!(
        built.next_step > floor_step,
        "custom floor stalls at step {}: {:?}",
        built.next_step,
        built.status
    );
    assert_eq!(chunks.block_at(raised), Some(3));
    assert_eq!(journal.block(raised), Some(3));
}
