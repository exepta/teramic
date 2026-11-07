//! Opt-in, delayed readback copied at the actual early/late/main pass boundaries.
use bevy::{
    pbr::RenderMeshInstances,
    prelude::*,
    render::{
        batching::gpu_preprocessing::{IndirectBatchSet, IndirectParametersBuffers},
        mesh::{RenderMesh, RenderMeshBufferInfo},
        render_asset::RenderAssets,
        render_phase::{PhaseItem, PhaseItemExtraIndex},
        render_resource::{Buffer, BufferDescriptor, BufferUsages, CommandEncoder, MapMode},
        renderer::{RenderContext, RenderDevice},
    },
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

const MAX_BYTES: u64 = 8 * 1024 * 1024;
const PHASES: usize = 9;

#[derive(Clone, Debug, Default)]
pub struct GpuSubdrawSnapshot {
    pub frame: u64,
    /// Same nine phase indices as DrawCallSnapshot; early/late contributions
    /// are copied separately before their buffers are rebuilt, then summed.
    pub submitted: [u64; PHASES],
    /// Commands with both nonzero vertex/index count and nonzero instance count.
    pub nonempty: [u64; PHASES],
    /// Opaque/mask prepass, opaque/mask deferred, shadow, at each boundary.
    pub early_submitted: [u64; 5],
    pub late_submitted: [u64; 5],
    pub early_nonempty: [u64; 5],
    pub late_nonempty: [u64; 5],
    pub unread_submissions: u64,
    pub readback_bytes: u64,
}

struct Request {
    phase: usize,
    parameters: Buffer,
    offset: u64,
    stride: u64,
    maximum: u32,
    count: Option<(Buffer, u64)>,
}

#[derive(Default)]
struct State {
    requests: Vec<Request>,
    direct: [u64; PHASES],
    direct_nonempty: [u64; PHASES],
    unread: u64,
    reserved: u64,
    staging: Option<Buffer>,
    copied: u64,
    parts: Vec<Part>,
    pending_direct: [u64; 5],
    pending_nonempty: [u64; 5],
    early_direct: [u64; 5],
    early_nonempty: [u64; 5],
    late_direct: [u64; 5],
    late_nonempty: [u64; 5],
}

#[derive(Resource, Clone, Default)]
pub(super) struct Collector {
    capture: Arc<AtomicBool>,
    in_flight: Arc<AtomicBool>,
    frame: Arc<AtomicU64>,
    state: Arc<Mutex<State>>,
    pub last: Arc<Mutex<Option<GpuSubdrawSnapshot>>>,
}

impl Collector {
    pub fn record<P: PhaseItem>(
        &self,
        phase: usize,
        item: &P,
        instances: &RenderMeshInstances,
        meshes: &RenderAssets<RenderMesh>,
        buffers: &IndirectParametersBuffers,
    ) {
        if phase >= PHASES || !self.capture.load(Ordering::Relaxed) {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let PhaseItemExtraIndex::IndirectParametersIndex {
            range,
            batch_set_index,
        } = item.extra_index()
        else {
            state.direct[phase] += 1;
            if phase >= 4 {
                state.pending_direct[phase - 4] += 1;
            }
            if let Some(mesh) = instances
                .mesh_asset_id(item.main_entity())
                .and_then(|id| meshes.get(id))
            {
                let elements = match mesh.buffer_info {
                    RenderMeshBufferInfo::Indexed { count, .. } => count,
                    RenderMeshBufferInfo::NonIndexed => mesh.vertex_count,
                };
                if elements > 0 && !item.batch_range().is_empty() {
                    state.direct_nonempty[phase] += 1;
                    if phase >= 4 {
                        state.pending_nonempty[phase - 4] += 1;
                    }
                }
            } else {
                state.unread += 1;
            }
            return;
        };
        if range.is_empty() {
            return;
        }
        let Some(mesh) = instances
            .mesh_asset_id(item.main_entity())
            .and_then(|id| meshes.get(id))
        else {
            state.unread += 1;
            return;
        };
        let Some(phase_buffers) = buffers.get(&std::any::TypeId::of::<P>()) else {
            state.unread += 1;
            return;
        };
        let indexed = matches!(mesh.buffer_info, RenderMeshBufferInfo::Indexed { .. });
        let (parameters, counts) = if indexed {
            (
                phase_buffers.indexed.data_buffer(),
                phase_buffers.indexed.batch_sets_buffer(),
            )
        } else {
            (
                phase_buffers.non_indexed.data_buffer(),
                phase_buffers.non_indexed.batch_sets_buffer(),
            )
        };
        let Some(parameters) = parameters else {
            state.unread += 1;
            return;
        };
        let count = batch_set_index.and_then(|index| {
            counts.map(|buffer| {
                (
                    buffer.clone(),
                    u64::from(u32::from(index)) * size_of::<IndirectBatchSet>() as u64,
                )
            })
        });
        if batch_set_index.is_some() && count.is_none() {
            state.unread += 1;
            return;
        }
        let stride = if indexed { 20 } else { 16 };
        let maximum = range.end.saturating_sub(range.start);
        let bytes = u64::from(maximum) * stride + u64::from(count.is_some()) * 4;
        if !reserve(&mut state.reserved, bytes) {
            state.unread += 1;
            return;
        }
        state.requests.push(Request {
            phase,
            parameters: parameters.clone(),
            offset: u64::from(range.start) * stride,
            stride,
            maximum,
            count,
        });
    }
}

fn reserve(reserved: &mut u64, bytes: u64) -> bool {
    if bytes == 0 || bytes > MAX_BYTES.saturating_sub(*reserved) {
        return false;
    }
    *reserved += bytes;
    true
}

#[derive(Clone, Copy)]
enum Boundary {
    Early,
    Late,
    Color,
}

fn copy_requests(
    state: &mut State,
    device: &RenderDevice,
    encoder: &mut CommandEncoder,
    boundary: Boundary,
) {
    let prepass = !matches!(boundary, Boundary::Color);
    if prepass {
        for i in 0..5 {
            let direct = std::mem::take(&mut state.pending_direct[i]);
            let nonempty = std::mem::take(&mut state.pending_nonempty[i]);
            match boundary {
                Boundary::Early => {
                    state.early_direct[i] += direct;
                    state.early_nonempty[i] += nonempty;
                }
                Boundary::Late => {
                    state.late_direct[i] += direct;
                    state.late_nonempty[i] += nonempty;
                }
                Boundary::Color => {}
            }
        }
    }
    let requests = std::mem::take(&mut state.requests);
    for request in requests {
        if (request.phase >= 4) != prepass {
            state.requests.push(request);
            continue;
        }
        let size = u64::from(request.maximum) * request.stride;
        let total = size + u64::from(request.count.is_some()) * 4;
        if size == 0
            || total > MAX_BYTES.saturating_sub(state.copied)
            || request.offset + size > request.parameters.size()
            || request
                .count
                .as_ref()
                .is_some_and(|(buffer, offset)| *offset + 4 > buffer.size())
        {
            state.unread += 1;
            continue;
        }
        let staging = state.staging.get_or_insert_with(|| {
            device.create_buffer(&BufferDescriptor {
                label: Some("Terra bounded all-phase draw readback"),
                size: MAX_BYTES,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        encoder.copy_buffer_to_buffer(
            &request.parameters,
            request.offset,
            staging,
            state.copied,
            size,
        );
        let count_offset = if let Some((buffer, source)) = request.count {
            encoder.copy_buffer_to_buffer(&buffer, source, staging, state.copied + size, 4);
            Some((state.copied + size) as usize)
        } else {
            None
        };
        state.parts.push(Part {
            phase: request.phase,
            offset: state.copied as usize,
            stride: request.stride as usize,
            maximum: request.maximum as usize,
            count_offset,
            boundary,
        });
        state.copied += total;
    }
}

fn capture_boundary(collector: &Collector, ctx: &mut RenderContext, boundary: Boundary) {
    if !collector.capture.load(Ordering::Relaxed) {
        return;
    }
    let Ok(mut state) = collector.state.lock() else {
        return;
    };
    let device = ctx.render_device().clone();
    copy_requests(&mut state, &device, ctx.command_encoder(), boundary);
}

pub(super) fn capture_early(collector: Res<Collector>, mut ctx: RenderContext) {
    capture_boundary(&collector, &mut ctx, Boundary::Early);
}

pub(super) fn capture_late(collector: Res<Collector>, mut ctx: RenderContext) {
    capture_boundary(&collector, &mut ctx, Boundary::Late);
}

pub(super) fn capture_color(collector: Res<Collector>, mut ctx: RenderContext) {
    capture_boundary(&collector, &mut ctx, Boundary::Color);
}

struct Part {
    phase: usize,
    offset: usize,
    stride: usize,
    maximum: usize,
    count_offset: Option<usize>,
    boundary: Boundary,
}

fn decode(bytes: &[u8], parts: &[Part], mut sample: GpuSubdrawSnapshot) -> GpuSubdrawSnapshot {
    let word = |offset: usize| {
        bytes
            .get(offset..offset + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
    };
    for part in parts {
        let count = match part.count_offset {
            Some(offset) => match word(offset) {
                Some(count) => (count as usize).min(part.maximum),
                None => {
                    sample.unread_submissions += 1;
                    continue;
                }
            },
            None => part.maximum,
        };
        sample.submitted[part.phase] += count as u64;
        if part.phase >= 4 {
            match part.boundary {
                Boundary::Early => sample.early_submitted[part.phase - 4] += count as u64,
                Boundary::Late => sample.late_submitted[part.phase - 4] += count as u64,
                Boundary::Color => {}
            }
        }
        for index in 0..count {
            let offset = part.offset + index * part.stride;
            if word(offset).is_some_and(|n| n > 0) && word(offset + 4).is_some_and(|n| n > 0) {
                sample.nonempty[part.phase] += 1;
                if part.phase >= 4 {
                    match part.boundary {
                        Boundary::Early => sample.early_nonempty[part.phase - 4] += 1,
                        Boundary::Late => sample.late_nonempty[part.phase - 4] += 1,
                        Boundary::Color => {}
                    }
                }
            }
        }
    }
    sample
}

pub(super) fn readback(
    collector: Res<Collector>,
    device: Res<RenderDevice>,
    mut frame: Local<u64>,
) {
    *frame += 1;
    let _ = device.poll(bevy::render::render_resource::PollType::Poll);
    let capturing = collector.capture.swap(false, Ordering::Relaxed);
    if capturing {
        let state = collector
            .state
            .lock()
            .ok()
            .map(|mut state| std::mem::take(&mut *state));
        if let Some(mut state) = state {
            let mut sample = GpuSubdrawSnapshot {
                frame: collector.frame.load(Ordering::Relaxed),
                submitted: state.direct,
                nonempty: state.direct_nonempty,
                early_submitted: state.early_direct,
                late_submitted: state.late_direct,
                early_nonempty: state.early_nonempty,
                late_nonempty: state.late_nonempty,
                unread_submissions: state.unread,
                ..default()
            };
            // Missing boundary copies are explicitly unavailable. Copying here
            // would read a later camera or reset early/late indirect buffers.
            sample.unread_submissions += state.requests.len() as u64;
            sample.unread_submissions += state.pending_direct.iter().sum::<u64>();
            if let Some(staging) = state.staging.take() {
                let parts = state.parts;
                sample.readback_bytes = state.copied;
                collector.in_flight.store(true, Ordering::Relaxed);
                let buffer = staging.clone();
                let collector = collector.clone();
                let copied = state.copied;
                staging
                    .slice(..copied)
                    .map_async(MapMode::Read, move |result| {
                        if result.is_ok()
                            && let Ok(mapped) = buffer.slice(..copied).get_mapped_range()
                        {
                            let sample = decode(&mapped, &parts, sample);
                            drop(mapped);
                            if let Ok(mut last) = collector.last.lock() {
                                *last = Some(sample);
                            }
                        } else {
                            tracing::warn!("Indirect draw diagnostics readback failed");
                        }
                        if result.is_ok() {
                            buffer.unmap();
                        }
                        collector.in_flight.store(false, Ordering::Relaxed);
                    });
            } else if let Ok(mut last) = collector.last.lock() {
                *last = Some(sample);
            }
        }
    }
    collector.arm(*frame);
}

impl Collector {
    fn arm(&self, frame: u64) {
        // Sparse asynchronous captures never block the render thread or accumulate buffers.
        if frame > 0 && frame.is_multiple_of(60) && !self.in_flight.load(Ordering::Relaxed) {
            self.frame.store(frame.wrapping_add(1), Ordering::Relaxed);
            self.capture.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_count_and_zero_instance_commands_are_distinct_and_counts_are_clamped() {
        let mut bytes = Vec::new();
        for word in [36_u32, 4, 0, 0, 0, 36, 0, 0, 0, 0, 500] {
            bytes.extend(word.to_le_bytes());
        }
        let part = Part {
            phase: 1,
            offset: 0,
            stride: 20,
            maximum: 2,
            count_offset: Some(40),
            boundary: Boundary::Color,
        };
        let sample = decode(&bytes, &[part], default());
        assert_eq!(sample.submitted, [0, 2, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(sample.nonempty, [0, 1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn all_nine_phases_decode_early_late_and_color_without_reusing_later_counts() {
        let mut bytes = Vec::new();
        let mut parts = Vec::new();
        for phase in 0..PHASES {
            for boundary in if phase < 4 {
                vec![Boundary::Color]
            } else {
                vec![Boundary::Early, Boundary::Late]
            } {
                let offset = bytes.len();
                for word in [36_u32, 1, 0, 0, 0] {
                    bytes.extend(word.to_le_bytes());
                }
                parts.push(Part {
                    phase,
                    offset,
                    stride: 20,
                    maximum: 1,
                    count_offset: None,
                    boundary,
                });
            }
        }
        let sample = decode(
            &bytes,
            &parts,
            GpuSubdrawSnapshot {
                frame: 181,
                ..default()
            },
        );
        assert_eq!(sample.frame, 181);
        assert_eq!(sample.submitted, [1, 1, 1, 1, 2, 2, 2, 2, 2]);
        assert_eq!(sample.nonempty, sample.submitted);
        assert_eq!(sample.early_submitted, [1; 5]);
        assert_eq!(sample.late_submitted, [1; 5]);
        assert_eq!(sample.early_nonempty, [1; 5]);
        assert_eq!(sample.late_nonempty, [1; 5]);
        assert_eq!(sample.unread_submissions, 0);
    }

    #[test]
    fn combined_early_late_color_budget_never_exceeds_eight_mib() {
        let mut reserved = 0;
        assert!(!reserve(&mut reserved, 0));
        for _boundary in 0..4 {
            assert!(reserve(&mut reserved, MAX_BYTES / 4));
        }
        assert_eq!(reserved, MAX_BYTES);
        assert!(!reserve(&mut reserved, 4));
        assert!(!reserve(&mut reserved, u64::MAX));
        assert_eq!(reserved, MAX_BYTES);
    }

    #[test]
    fn capture_schedule_keeps_one_delayed_frame_tag_and_waits_for_its_readback() {
        let collector = Collector::default();
        for frame in 0..60 {
            collector.arm(frame);
        }
        assert!(!collector.capture.load(Ordering::Relaxed));
        collector.arm(60);
        assert!(collector.capture.swap(false, Ordering::Relaxed));
        assert_eq!(collector.frame.load(Ordering::Relaxed), 61);
        collector.in_flight.store(true, Ordering::Relaxed);
        for frame in 61..=180 {
            collector.arm(frame);
        }
        assert!(!collector.capture.load(Ordering::Relaxed));
        assert_eq!(collector.frame.load(Ordering::Relaxed), 61);
        collector.in_flight.store(false, Ordering::Relaxed);
        collector.arm(181);
        assert!(!collector.capture.load(Ordering::Relaxed));
        collector.arm(240);
        assert!(collector.capture.load(Ordering::Relaxed));
        assert_eq!(collector.frame.load(Ordering::Relaxed), 241);
    }
}
