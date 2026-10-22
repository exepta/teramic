use super::SAMPLE_COUNT;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VoxelError {
    #[error("sample coordinate {0:?} outside 0..33")]
    OutOfBounds([usize; 3]),
    #[error("chunk/global coordinate exceeds i64 range")]
    CoordinateOverflow,
    #[error("expected {SAMPLE_COUNT} samples, got {0}")]
    InvalidSampleCount(usize),
    #[error("density must be finite")]
    NonFiniteDensity,
    #[error("sample {0:?} is a halo; write through its canonical owning chunk")]
    HaloWrite([usize; 3]),
    #[error("sample is not a halo of this chunk")]
    NotHalo,
    #[error("chunk revision exhausted")]
    RevisionOverflow,
}
