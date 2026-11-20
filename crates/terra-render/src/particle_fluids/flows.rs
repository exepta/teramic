//! One published, collision-clipped surface for both standing and running water.
//! The server pays fluid transfers; the renderer only interpolates those targets.
use super::*;
use bevy::ecs::{message::MessageCursor, system::SystemParam};
use std::collections::HashSet;
use terra_voxel::SectionPos;
use terra_world::{
    edits::EditJournal,
    streaming::{
        ChunkFocus, ChunkStore, SectionPublished, SectionRemoved, StreamingSet, WaterPublished,
    },
};

#[derive(Default)]
struct Queue {
    pending: HashSet<SectionPos>,
    sections: MessageCursor<SectionPublished>,
    water: MessageCursor<WaterPublished>,
    removed: MessageCursor<SectionRemoved>,
    seeded: bool,
}
#[derive(SystemParam)]
struct Source<'w> {
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    focus: Res<'w, ChunkFocus>,
    sections: Option<Res<'w, Messages<SectionPublished>>>,
    water: Option<Res<'w, Messages<WaterPublished>>>,
    removed: Option<Res<'w, Messages<SectionRemoved>>>,
}
pub(super) fn install(app: &mut App) {
    app.add_systems(Update, update.after(StreamingSet::Receive));
}
fn update(
    mut output: ResMut<FarFluidSections>,
    mut input: ResMut<ParticleFluidInput>,
    source: Source,
    mut queue: Local<Queue>,
) {
    if std::env::args().any(|a| a.starts_with("--smoke-test"))
        && std::env::var("TERRA_PARTICLE_MANUAL_INPUT").as_deref() == Ok("1")
    {
        return;
    }
    // A camera-local simulation cannot own saved-world liquid: its moving box
    // reshaped a pool, overlapped flowing water and changed the apparent volume.
    if input.enabled || input.target_mass != [0.; 4] {
        input.enabled = false;
        input.particles.clear();
        input.solids.clear();
        input.paths.clear();
        input.target_mass = [0.; 4];
        input.revision = input.revision.wrapping_add(1);
    }
    if source.chunks.loaded.is_empty() {
        queue.pending.clear();
        queue.seeded = false;
        if !output.sections.is_empty() {
            output.sections.clear();
            output.revision = output.revision.wrapping_add(1);
        }
        return;
    }
    let notified = source.sections.is_some() && source.water.is_some() && source.removed.is_some();
    if !queue.seeded || (!notified && (source.chunks.is_changed() || source.journal.is_changed())) {
        queue.pending.extend(source.chunks.loaded.keys().copied());
        queue.seeded = true;
    }
    if let Some(messages) = &source.sections {
        let positions: Vec<_> = queue.sections.read(messages).map(|m| m.position).collect();
        queue.pending.extend(positions);
    }
    if let Some(messages) = &source.water {
        let positions: Vec<_> = queue.water.read(messages).map(|m| m.position).collect();
        queue.pending.extend(positions);
    }
    let removed: Vec<_> = if let Some(messages) = &source.removed {
        queue.removed.read(messages).map(|m| m.position).collect()
    } else {
        output
            .sections
            .keys()
            .filter(|p| !source.chunks.loaded.contains_key(*p))
            .copied()
            .collect()
    };
    for p in removed {
        queue.pending.remove(&p);
        if output.sections.remove(&p).is_some() {
            output.revision = output.revision.wrapping_add(1);
        }
    }
    let mut pending: Vec<_> = queue.pending.iter().copied().collect();
    pending.sort_by(|a, b| {
        crate::meshing::lod::distance_squared(*a, source.focus.0)
            .total_cmp(&crate::meshing::lod::distance_squared(*b, source.focus.0))
    });
    let mut accepted = 0;
    for p in pending.into_iter().take(256) {
        if accepted >= 32 {
            break;
        }
        let Some(section) = source.chunks.loaded.get(&p) else {
            queue.pending.remove(&p);
            continue;
        };
        if section.water_epoch != source.journal.water_mesh_epoch(p) {
            continue;
        }
        let patch = section
            .water
            .as_ref()
            .filter(|patch| !patch.indices.is_empty());
        // Neighbour edits can publish a byte-for-byte identical wet patch. A
        // new GPU entity would unnecessarily restart the running interpolation.
        if output
            .sections
            .get(&p)
            .and_then(|old| old.published_water.as_ref())
            == patch
        {
            queue.pending.remove(&p);
            continue;
        }
        if let Some(patch) = patch {
            let epoch = output.revision.wrapping_add(1);
            output.sections.insert(
                p,
                FarFluidSection {
                    epoch,
                    fields: std::array::from_fn(|_| Vec::new()),
                    films: Vec::new(),
                    published_water: Some(patch.clone()),
                },
            );
            output.body_patches = output.body_patches.wrapping_add(1);
            output.revision = output.revision.wrapping_add(1);
            accepted += 1;
        } else if output.sections.remove(&p).is_some() {
            output.revision = output.revision.wrapping_add(1);
            accepted += 1;
        }
        queue.pending.remove(&p);
    }
    if output.revision == 0 {
        output.revision = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::BlockSection;
    use terra_world::{
        streaming::LoadedSection,
        water::{FluidCell, WaterPatch},
    };

    fn running_patch() -> WaterPatch {
        WaterPatch {
            positions: vec![[3., 3., 3.], [3., 3., 4.], [5., 2.125, 4.], [5., 2.125, 3.]],
            normals: vec![[0., 1., 0.]; 4],
            flow: vec![[1., 0.]; 4],
            depth_shore: vec![[0.125, 0.]; 4],
            surface_effects: vec![[0.; 4]; 4],
            wave_weights: vec![0.; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }
    fn world() -> App {
        let mut app = App::new();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        journal
            .set_fluid(
                [3, 2, 3],
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [1, 0],
                },
            )
            .unwrap();
        let water_epoch = journal.water_mesh_epoch(SectionPos::default());
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data: BlockSection::empty(SectionPos::default()).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch,
                water: Some(running_patch()),
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks)
            .insert_resource(ChunkFocus([4., 3., 3.]))
            .insert_resource(journal)
            .init_resource::<FarFluidSections>()
            .init_resource::<ParticleFluidInput>()
            .add_message::<SectionPublished>()
            .add_message::<WaterPublished>()
            .add_message::<SectionRemoved>();
        install(&mut app);
        app
    }
    #[test]
    fn even_a_small_flow_uses_the_exact_clipped_worker_mesh_and_never_camera_particles() {
        let mut app = world();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid(
                [3, 2, 3],
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [1, 0],
                },
            )
            .unwrap();
        let epoch = app
            .world()
            .resource::<EditJournal>()
            .water_mesh_epoch(SectionPos::default());
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .water_epoch = epoch;
        app.update();
        let baseline = app.world().resource::<FarFluidSections>().revision;
        let particle_revision = app.world().resource::<ParticleFluidInput>().revision;
        for x in 0..32 {
            app.world_mut().resource_mut::<ChunkFocus>().0 = [x as f64 * 4., 72., 16.];
            app.update();
            let surface = app.world().resource::<FarFluidSections>();
            let patch = surface.sections[&SectionPos::default()]
                .published_water
                .as_ref()
                .unwrap();
            assert_eq!(patch.positions, running_patch().positions);
            assert_eq!(patch.flow, running_patch().flow);
            assert!(
                surface.sections[&SectionPos::default()]
                    .fields
                    .iter()
                    .all(Vec::is_empty)
            );
            assert_eq!(surface.revision, baseline);
            assert_eq!(surface.sampled_cells, 0);
            let input = app.world().resource::<ParticleFluidInput>();
            assert!(!input.enabled && input.particles.is_empty());
            assert_eq!(input.target_mass, [0.; 4]);
            assert_eq!(input.revision, particle_revision);
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .fluid([3, 2, 3])
                    .unwrap()
                    .amount,
                8
            );
        }
    }
    #[test]
    fn repeated_publications_keep_the_surface_and_flow_only_changes_still_update_it() {
        let mut app = world();
        app.update();
        let position = SectionPos::default();
        let epoch = app.world().resource::<FarFluidSections>().sections[&position].epoch;
        for _ in 0..3 {
            app.world_mut().write_message(WaterPublished { position });
            app.update();
            assert_eq!(
                app.world().resource::<FarFluidSections>().sections[&position].epoch,
                epoch
            );
        }
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&position)
            .unwrap()
            .water
            .as_mut()
            .unwrap()
            .flow
            .fill([0., -1.]);
        app.world_mut().write_message(WaterPublished { position });
        app.update();
        let surface = &app.world().resource::<FarFluidSections>().sections[&position];
        assert_ne!(surface.epoch, epoch);
        assert_eq!(
            surface.published_water.as_ref().unwrap().flow,
            [[0., -1.]; 4]
        );
    }
    #[test]
    fn source_withdrawal_waits_for_the_matching_worker_publication_then_removes_the_old_surface() {
        let mut app = world();
        app.update();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_fluid([3, 2, 3], FluidCell::default())
            .unwrap();
        app.world_mut()
            .resource_mut::<Messages<WaterPublished>>()
            .write(WaterPublished {
                position: SectionPos::default(),
            });
        app.update();
        assert!(
            app.world()
                .resource::<FarFluidSections>()
                .sections
                .contains_key(&SectionPos::default())
        );
        let epoch = app
            .world()
            .resource::<EditJournal>()
            .water_mesh_epoch(SectionPos::default());
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            let section = chunks.loaded.get_mut(&SectionPos::default()).unwrap();
            section.water = None;
            section.water_epoch = epoch;
        }
        app.world_mut()
            .resource_mut::<Messages<WaterPublished>>()
            .write(WaterPublished {
                position: SectionPos::default(),
            });
        app.update();
        assert!(
            app.world()
                .resource::<FarFluidSections>()
                .sections
                .is_empty()
        );
        assert!(!app.world().resource::<ParticleFluidInput>().enabled);
    }
}
