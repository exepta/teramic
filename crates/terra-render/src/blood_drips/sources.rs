use super::*;

pub(super) fn block(chunks: &ChunkStore, journal: &EditJournal, cell: [i64; 3]) -> Option<u16> {
    chunks
        .block_at(cell)
        .map(|base| journal.block(cell).unwrap_or(base))
}

fn source(chunks: &ChunkStore, journal: &EditJournal, cell: [i64; 3], eye: Vec3) -> Option<Source> {
    let seed = motion::hash(cell);
    if !seed.is_multiple_of(11) {
        return None;
    }
    if block(chunks, journal, cell) != Some(LEAVES)
        || block(chunks, journal, [cell[0], cell[1] - 1, cell[2]]) != Some(0)
    {
        return None;
    }
    let origin = Vec3::from_array(cell.map(|v| v as f32))
        + Vec3::new(
            0.25 + (seed % 100) as f32 * 0.005,
            -0.015,
            0.25 + ((seed >> 8) % 100) as f32 * 0.005,
        );
    let distance = eye.distance_squared(origin);
    if distance >= RANGE * RANGE || distance > 28.0 * 28.0 && !seed.is_multiple_of(3) {
        return None;
    }
    let water = chunks
        .surface_column_at(cell[0], cell[2])
        .and_then(|c| c.water_level);
    for dy in 1..=32 {
        let p = [cell[0], cell[1] - dy, cell[2]];
        let material = block(chunks, journal, p)?;
        let solid = terra_voxel::MaterialId::from_raw(material).is_some_and(|m| m.is_solid());
        let wet =
            water.is_some_and(|level| p[1] as f64 + 1.0 <= level && level < f64::from(origin.y));
        if solid || wet || material == 27 {
            let impact = if wet && !solid {
                water? as f32
            } else {
                p[1] as f32 + 1.0
            };
            return (origin.y - impact > 0.4).then_some(Source {
                cell,
                origin,
                impact,
                seed,
                distance,
            });
        }
    }
    None
}

pub(super) fn refresh(
    time: Res<Time>,
    access: Res<WorldAccess>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    cameras: Query<&Transform, With<TerraCamera>>,
    mut cache: ResMut<Cache>,
    mut status: ResMut<BloodDripStatus>,
) {
    let Some(eye) = cameras
        .iter()
        .next()
        .filter(|_| access.ready())
        .map(|c| c.translation)
    else {
        cache.sources.clear();
        status.candidates = 0;
        return;
    };
    if time.elapsed_secs() < cache.next {
        return;
    }
    cache.next = time.elapsed_secs() + 0.5;
    cache.sources.clear();
    cache.sections.clear();
    for (&position, section) in &chunks.loaded {
        if !section.data.may_contain(LEAVES) && journal.revision(position) == 0 {
            continue;
        }
        let Ok(origin) = position.origin() else {
            continue;
        };
        let center = Vec3::from_array(origin.map(|v| v as f32)) + Vec3::splat(8.0);
        let distance = center.distance_squared(eye);
        if distance > (RANGE + 14.0).powi(2) {
            continue;
        }
        let at = cache
            .sections
            .partition_point(|&(d, p)| d.total_cmp(&distance).then(p.cmp(&position)).is_lt());
        if at < MAX_SECTIONS {
            if cache.sections.len() == MAX_SECTIONS {
                cache.sections.pop();
            }
            cache.sections.insert(at, (distance, position));
        }
    }
    status.scanned_sections = cache.sections.len();
    for index in 0..cache.sections.len() {
        let position = cache.sections[index].1;
        let Ok(origin) = position.origin() else {
            continue;
        };
        let edited = journal.revision(position) > 0;
        for (index, &material) in chunks.loaded[&position].data.blocks().iter().enumerate() {
            if !edited && material != LEAVES {
                continue;
            }
            let i = index as i64;
            let p = [
                origin[0] + i % 16,
                origin[1] + i / 16 % 16,
                origin[2] + i / 256,
            ];
            if let Some(candidate) = source(&chunks, &journal, p, eye) {
                let at = cache.sources.partition_point(|s| {
                    s.distance
                        .total_cmp(&candidate.distance)
                        .then(s.cell.cmp(&candidate.cell))
                        .is_lt()
                });
                if at < MAX_DROPS {
                    if cache.sources.len() == MAX_DROPS {
                        cache.sources.pop();
                    }
                    cache.sources.insert(at, candidate);
                }
            }
        }
    }
    status.candidates = cache.sources.len();
    status.refreshes += 1;
}
