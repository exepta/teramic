use super::{CHUNK_SIZE, ChunkPos, SAMPLE_COUNT, VoxelError, VoxelSample, sample_index};

/// Samples remain private so index, density, revision and ownership checks cannot be bypassed.
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelChunk {
    position: ChunkPos,
    revision: u64,
    samples: Vec<VoxelSample>,
}
impl VoxelChunk {
    pub fn new(position: ChunkPos) -> Self {
        Self {
            position,
            revision: 0,
            samples: vec![VoxelSample::AIR; SAMPLE_COUNT],
        }
    }
    /// Generation supplies halos from the same global sample function as their owners.
    pub fn from_samples(position: ChunkPos, samples: Vec<VoxelSample>) -> Result<Self, VoxelError> {
        if samples.len() != SAMPLE_COUNT {
            return Err(VoxelError::InvalidSampleCount(samples.len()));
        }
        if samples.iter().any(|s| !s.density.is_finite()) {
            return Err(VoxelError::NonFiniteDensity);
        }
        position.global_sample([32; 3])?;
        Ok(Self {
            position,
            revision: 0,
            samples,
        })
    }
    pub fn position(&self) -> ChunkPos {
        self.position
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn samples(&self) -> &[VoxelSample] {
        &self.samples
    }
    pub fn sample(&self, local: [usize; 3]) -> Result<VoxelSample, VoxelError> {
        Ok(self.samples[sample_index(local)?])
    }
    pub fn set_sample(&mut self, local: [usize; 3], value: VoxelSample) -> Result<(), VoxelError> {
        let index = sample_index(local)?;
        if local.contains(&(CHUNK_SIZE as usize)) {
            return Err(VoxelError::HaloWrite(local));
        }
        self.write(index, value)
    }
    /// World synchronization copies an authoritative owner's value into a neighbor's halo.
    /// This is the only mutation allowed on upper boundary samples.
    pub fn sync_halo(&mut self, global: [i64; 3], value: VoxelSample) -> Result<(), VoxelError> {
        let origin = self.position.global_sample([0; 3])?;
        let mut local = [0; 3];
        for axis in 0..3 {
            let offset = global[axis]
                .checked_sub(origin[axis])
                .ok_or(VoxelError::CoordinateOverflow)?;
            if !(0..=i64::from(CHUNK_SIZE)).contains(&offset) {
                return Err(VoxelError::NotHalo);
            }
            local[axis] = offset as usize;
        }
        if !local.contains(&(CHUNK_SIZE as usize)) {
            return Err(VoxelError::NotHalo);
        }
        self.write(sample_index(local)?, value)
    }
    fn write(&mut self, index: usize, value: VoxelSample) -> Result<(), VoxelError> {
        if !value.density.is_finite() {
            return Err(VoxelError::NonFiniteDensity);
        }
        if self.samples[index] != value {
            let revision = self
                .revision
                .checked_add(1)
                .ok_or(VoxelError::RevisionOverflow)?;
            self.samples[index] = value;
            self.revision = revision;
        }
        Ok(())
    }
}

#[cfg(test)]
impl VoxelChunk {
    pub(super) fn set_revision_for_test(&mut self, revision: u64) {
        self.revision = revision;
    }
}
