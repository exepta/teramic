//! One-way sprint flight into newly generated terrain, followed by a stationary recovery.
use super::*;
use terra_render::{RenderStatus, RenderedSections, TerrainVisibility};
use terra_voxel::{SECTION_SIZE, global_to_section};
use terra_world::streaming::WorldGeneration;

pub(super) const DISTANCE: f32 = 3_600.0;

#[derive(Default)]
pub(super) struct Frontier {
    heights: Vec<f32>,
    arrived: bool,
    max_y: Option<f32>,
    interval_start: Option<Instant>,
    previous: Option<Instant>,
    phase: Option<&'static str>,
    frame_ms: Vec<f64>,
}

impl Frontier {
    pub(super) fn with_heights(profile: Option<&str>) -> Result<Self, String> {
        let Some(profile) = profile else {
            return Ok(Self::default());
        };
        let heights: Vec<f32> = serde_json::from_str(profile).map_err(|e| {
            format!("TERRA_FRONTIER_HEIGHTS must contain a JSON array of elevations: {e}")
        })?;
        if !(2..=4097).contains(&heights.len())
            || !heights
                .iter()
                .all(|y| y.is_finite() && (-100.0..=344.0).contains(y))
        {
            return Err("TERRA_FRONTIER_HEIGHTS requires 2 through 4097 elevations within the playable height range".into());
        }
        Ok(Self {
            heights,
            ..Default::default()
        })
    }

    pub(super) fn flight_y(&self, traveled: f32) -> Option<f32> {
        let last = self.heights.len().checked_sub(1)?;
        let index = (traveled / DISTANCE).clamp(0.0, 1.0) * last as f32;
        let i = index.floor() as usize;
        let low = self.heights[i];
        Some(low + (self.heights[(i + 1).min(last)] - low) * index.fract())
    }

    pub(super) fn target(&mut self, distance: f32) -> (u64, &'static str, f32) {
        self.arrived |= distance >= DISTANCE - 1.0;
        (
            0,
            if self.arrived { "settle" } else { "outbound" },
            DISTANCE,
        )
    }

    pub(super) fn sample(
        &mut self,
        world: &World,
        now: Instant,
        phase: &'static str,
        position: Vec3,
        authoritative_x: f32,
    ) {
        self.max_y = Some(self.max_y.unwrap_or(position.y).max(position.y));
        let previous = self.previous.replace(now);
        if self.phase != Some(phase) {
            self.phase = Some(phase);
            self.interval_start = Some(now);
            self.frame_ms.clear();
            return;
        }
        if let Some(previous) = previous {
            self.frame_ms
                .push(now.duration_since(previous).as_secs_f64() * 1000.0);
        }
        let start = *self.interval_start.get_or_insert(now);
        let elapsed = now.duration_since(start).as_secs_f64();
        if elapsed < 5.0 {
            return;
        }
        let store = world.resource::<ChunkStore>();
        let render = world.resource::<RenderStatus>();
        let presented = world.resource::<RenderedSections>();
        let generation = world.resource::<WorldGeneration>();
        let visibility = world.get_resource::<TerrainVisibility>();
        // Probe the surface in the 5×5 columns around the player, including rows
        // ahead of the flight. Empty sections at cruise altitude do not count.
        let sample_columns = |first_x: i64| {
            let mut loaded = 0_u8;
            let mut ready = 0_u8;
            for dx in first_x..first_x + 5 {
                for dz in -2..=2 {
                    let x = position.x.floor() as i64 + dx * SECTION_SIZE as i64;
                    let z = position.z.floor() as i64 + dz * SECTION_SIZE as i64;
                    let Some(surface) = store.surface_column_at(x, z) else {
                        continue;
                    };
                    let section = global_to_section([x, surface.height.floor() as i64, z]).0;
                    let data = store.loaded.get(&section);
                    loaded += u8::from(data.is_some());
                    ready += u8::from(presented.section_presented(section, data, visibility));
                }
            }
            (loaded, ready)
        };
        let (loaded, ready) = sample_columns(-2);
        let (loaded_ahead, ready_ahead) = sample_columns(4); // 64–128 m into the flight path
        let [_, p95_ms, p99_ms, max_ms] =
            crate::performance_soak::frame_summary(&mut self.frame_ms);
        let ground = store
            .surface_column_at(position.x.floor() as i64, position.z.floor() as i64)
            .map(|surface| surface.height);
        tracing::info!(
            phase, y = position.y, max_flight_y = ?self.max_y, ground_y = ?ground,
            clearance_m = ?ground.map(|y| f64::from(position.y) - y),
            generation_ready = generation.status == terra_world::streaming::GenerationStatus::Ready,
            terrain_ready = render.ready,
            x = position.x,
            authoritative_x,
            interval_seconds = elapsed,
            frames = self.frame_ms.len(),
            fps = self.frame_ms.len() as f64 / elapsed,
            p95_ms,
            p99_ms,
            max_ms,
            surface_probes = 25,
            loaded_surfaces = loaded,
            presented_surfaces = ready,
            loaded_ahead,
            presented_ahead = ready_ahead,
            loaded_sections = generation.loaded_sections,
            wanted_sections = generation.wanted_sections,
            pending_generation = generation.pending_jobs,
            pending_meshes = render.pending_jobs,
            pending_mesh_columns = render.pending_columns.len(),
            completed_meshes = render.completed_mesh_jobs,
            "Frontier flight progress"
        );
        self.frame_ms.clear();
        self.interval_start = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flight_altitude_interpolates_by_position_and_validates_the_profile() {
        let route = Frontier::with_heights(Some("[70, 100, 75]")).unwrap();
        assert_eq!(route.flight_y(-40.0), Some(70.0));
        assert_eq!(route.flight_y(DISTANCE * 0.25), Some(85.0));
        assert_eq!(route.flight_y(DISTANCE + 20.0), Some(75.0));
        assert_eq!(Frontier::default().flight_y(123.0), None);
        for invalid in ["[]", "[25]", "[15, 400]", "[{},{}]"] {
            assert!(Frontier::with_heights(Some(invalid)).is_err());
        }
    }

    #[test]
    fn frontier_stops_at_the_destination_and_keeps_hovering_after_correction() {
        let mut flight = Frontier::default();
        assert_eq!(flight.target(1_000.0), (0, "outbound", DISTANCE));
        assert_eq!(flight.target(DISTANCE - 0.5), (0, "settle", DISTANCE));
        assert_eq!(flight.target(DISTANCE - 2.0), (0, "settle", DISTANCE));
    }
}
