//! Local diagnostics for the authoritative planner, never part of the wire/save format.
use terra_core::building_jobs::BuildError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSiteCause {
    InvalidOrigin,
    MissingMembership,
    PermissionDenied,
    Unloaded,
    StaleOverride,
    NonPreparableTerrain,
    GeneratedStructure,
    Container,
    ObjectState,
    Fluid,
    ReservedByJob,
    UnsupportedGround,
    InvalidTemplate,
}

impl std::fmt::Display for BuildSiteCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidOrigin => "invalid origin or orientation",
            Self::MissingMembership => "owner is not a settlement member",
            Self::PermissionDenied => "cell is outside authorized building claims",
            Self::Unloaded => "cell has no loaded voxel data",
            Self::StaleOverride => "saved override and loaded voxel disagree",
            Self::NonPreparableTerrain => "occupied cell cannot be prepared as terrain",
            Self::GeneratedStructure => "cell belongs to a generated structure",
            Self::Container => "cell contains stored inventory",
            Self::ObjectState => "cell contains a placed object",
            Self::Fluid => "cell contains fluid",
            Self::ReservedByJob => "another active construction job reserves this area",
            Self::UnsupportedGround => "required foundation support is air",
            Self::InvalidTemplate => "template is unavailable or invalid",
        })
    }
}

/// First rejection from the actual planner, not a second estimated validation.
/// `position` is the rejected cell, or the reserving job's anchor for job conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildSiteIssue {
    pub error: BuildError,
    pub cause: BuildSiteCause,
    pub position: Option<[i64; 3]>,
    pub material: Option<u16>,
    pub saved_material: Option<u16>,
    pub conflicting_job: Option<u64>,
}

impl BuildSiteIssue {
    pub(super) const fn new(error: BuildError, cause: BuildSiteCause) -> Self {
        Self {
            error,
            cause,
            position: None,
            material: None,
            saved_material: None,
            conflicting_job: None,
        }
    }

    pub(super) const fn at(
        error: BuildError,
        cause: BuildSiteCause,
        position: [i64; 3],
        material: Option<u16>,
        saved_material: Option<u16>,
    ) -> Self {
        Self {
            position: Some(position),
            material,
            saved_material,
            ..Self::new(error, cause)
        }
    }

    pub(super) const fn reserved(job: u64, origin: [i64; 3]) -> Self {
        Self {
            position: Some(origin),
            conflicting_job: Some(job),
            ..Self::new(BuildError::Blocked, BuildSiteCause::ReservedByJob)
        }
    }
}

impl std::fmt::Display for BuildSiteIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error, self.cause)?;
        if let Some(position) = self.position {
            write!(
                f,
                "; position={position:?} material={:?} saved_override={:?}",
                self.material, self.saved_material
            )?;
        }
        if let Some(job) = self.conflicting_job {
            write!(f, "; conflicting_job={job}")?;
        }
        Ok(())
    }
}

impl std::error::Error for BuildSiteIssue {}
