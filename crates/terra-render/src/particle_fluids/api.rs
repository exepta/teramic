//! Bounded authoritative input. All coordinates are relative to `origin`.
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_voxel::SectionPos;

pub const REST_SIDE: usize = 17;
pub const REST_NODES: usize = REST_SIDE * REST_SIDE * REST_SIDE;

#[derive(Clone, Debug)]
pub struct FluidFilm {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
    pub kind: u32,
}
#[derive(Clone, Debug)]
pub struct FarFluidSection {
    pub epoch: u64,
    pub fields: [Vec<f32>; 4],
    pub films: Vec<FluidFilm>,
    /// Worker-built reservoir shoreline/current geometry; never camera-relative.
    pub published_water: Option<terra_world::water::WaterPatch>,
}
#[derive(Resource, Default)]
pub struct FarFluidSections {
    pub revision: u64,
    pub sections: HashMap<SectionPos, FarFluidSection>,
    /// Main-thread volume samples, excluding already prepared water geometry.
    pub sampled_cells: u64,
    /// Prepared reservoir patches accepted without voxel re-sampling.
    pub body_patches: u64,
}
#[derive(Resource, Clone, Debug)]
pub struct RestFluidGrid {
    pub origin: [f32; 3],
    pub revision: u64,
    pub fields: [Vec<f32>; 4],
    pub films: Vec<FluidFilm>,
}
impl Default for RestFluidGrid {
    fn default() -> Self {
        Self {
            origin: [0.; 3],
            revision: 0,
            fields: std::array::from_fn(|_| vec![-1.; REST_NODES]),
            films: Vec::new(),
        }
    }
}

pub const MAX_PARTICLES: usize = 8192;
pub const MAX_COLLIDERS: usize = 1024;
pub const MAX_PATHS: usize = 512;
pub const DOMAIN_SIZE: f32 = 16.0;
pub const WATER: u32 = 0;
pub const BLOOD: u32 = 1;
pub const GALLE: u32 = 2;
pub const TAR: u32 = 3;
pub const INACTIVE: u32 = u32::MAX;
pub const ALL_KINDS: u32 = 15;

/// One persistent GPU slot. Only a generation change resets its position/velocity.
#[derive(Clone, Copy, Debug)]
pub struct FluidParticleSeed {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub kind: u32,
    pub generation: u32,
    /// Horizontal origin/radius limit; radius<=0 means unlimited within the domain.
    pub flow_origin: [f32; 3],
    pub flow_radius: f32,
}
impl Default for FluidParticleSeed {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            velocity: [0.; 3],
            kind: INACTIVE,
            generation: 0,
            flow_origin: [0.; 3],
            flow_radius: 0.,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FluidCollider {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
    /// Bit0 water, bit1 blood, bit2 Galle, bit3 tar.
    pub blocked_kinds: u32,
}

/// Legal tar coating, including its exact vertical thickness above prepared path.
#[derive(Clone, Copy, Debug)]
pub struct FluidPath {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}

#[derive(Resource, Clone, Debug)]
pub struct ParticleFluidInput {
    pub enabled: bool,
    pub revision: u64,
    /// World minimum corner of the16m cube. Seeds and shapes are relative to it.
    pub origin: [f32; 3],
    /// Stable slot order; unused slots have kind=INACTIVE. Length<=MAX_PARTICLES.
    pub particles: Vec<FluidParticleSeed>,
    pub solids: Vec<FluidCollider>,
    pub paths: Vec<FluidPath>,
    /// Authoritative cubic metres; water/blood/Galle quantum1/64, tar1/1024.
    pub target_mass: [f32; 4],
    /// Diagnostics only: copy GPU positions once after this many dispatch frames.
    pub readback_after_frames: Option<u64>,
}
impl Default for ParticleFluidInput {
    fn default() -> Self {
        Self {
            enabled: true,
            revision: 0,
            origin: [0.; 3],
            particles: Vec::new(),
            solids: Vec::new(),
            paths: Vec::new(),
            target_mass: [0.; 4],
            readback_after_frames: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParticleFluidReadback {
    pub frame: u64,
    /// Executed fixed-step physics time at the buffer copy, not callback time.
    pub simulated_seconds: f32,
    /// xyz=domain-relative position, w=kind (-1 for an inactive slot).
    pub particles: Vec<[f32; 4]>,
    /// Diagnostic GPU velocities at the same buffer copy as positions.
    pub velocities: Vec<[f32; 3]>,
}
#[derive(Resource, Clone, Debug, Default)]
pub struct ParticleFluidStatus {
    pub enabled: bool,
    pub gpu_ready: bool,
    pub active_particles: u32,
    pub requested_particles: u32,
    pub gpu_frames: u64,
    /// Time actually integrated by the GPU at 120 Hz, independent of render FPS.
    pub simulated_seconds: f32,
    pub error: Option<String>,
    pub readback: Option<ParticleFluidReadback>,
}

pub const fn particle_volume(kind: u32) -> f32 {
    if kind == TAR { 1. / 1024. } else { 1. / 64. }
}
