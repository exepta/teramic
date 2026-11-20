use super::ground;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use std::collections::BTreeSet;
use terra_core::{
    building_jobs::BuildArea,
    kingdom::CLAIM_CHUNK_SIZE,
    work_areas::{WorkAreaKind, WorkAreaMark},
};
use terra_world::{edits::EditJournal, streaming::ChunkStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum ContourKind {
    Work,
    Storage,
    Residential,
    Building,
    BuildBorder,
}

impl ContourKind {
    pub const ALL: [Self; 5] = [
        Self::Work,
        Self::Storage,
        Self::Residential,
        Self::Building,
        Self::BuildBorder,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Work => "Saved work area contour: work",
            Self::Storage => "Saved work area contour: storage",
            Self::Residential => "Saved work area contour: residential",
            Self::Building => "Saved work area contour: building",
            Self::BuildBorder => "Build chunk border contour",
        }
    }
    pub fn color(self) -> Color {
        let [r, g, b] = match self {
            Self::Work => [0.25, 1.0, 0.38],
            Self::Storage => [0.20, 0.70, 1.0],
            Self::Residential => [1.0, 0.84, 0.20],
            Self::Building => [1.0, 0.45, 0.10],
            Self::BuildBorder => [1.0, 0.10, 0.08],
        };
        Color::srgb(r, g, b)
    }
}

impl From<WorkAreaKind> for ContourKind {
    fn from(kind: WorkAreaKind) -> Self {
        match kind {
            WorkAreaKind::Work => Self::Work,
            WorkAreaKind::Storage => Self::Storage,
            WorkAreaKind::Residential => Self::Residential,
            WorkAreaKind::BuildingOrigin => Self::Building,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Contour {
    pub kind: ContourKind,
    pub from: [i64; 2],
    pub to: [i64; 2],
    pub height_hint: i64,
}

fn rectangle(out: &mut Vec<Contour>, kind: ContourKind, min: [i64; 2], max: [i64; 2], y: i64) {
    let corners = [min, [max[0], min[1]], max, [min[0], max[1]]];
    for i in 0..4 {
        out.push(Contour {
            kind,
            from: corners[i],
            to: corners[(i + 1) % 4],
            height_hint: y,
        });
    }
}

pub(super) fn append_area(out: &mut Vec<Contour>, mark: WorkAreaMark) {
    if !mark.bounds.valid() {
        return;
    }
    rectangle(
        out,
        mark.kind.into(),
        [mark.bounds.min[0], mark.bounds.min[2]],
        [mark.bounds.max[0] + 1, mark.bounds.max[2] + 1],
        mark.bounds.min[1],
    );
}

pub(super) fn near_area(area: BuildArea, focus: [f32; 3], radius: f32) -> bool {
    near_rectangle(
        [area.min[0], area.min[2]],
        [area.max[0] + 1, area.max[2] + 1],
        focus,
        radius,
    )
}

fn near_rectangle(min: [i64; 2], max: [i64; 2], focus: [f32; 3], radius: f32) -> bool {
    let dx = (focus[0] - focus[0].clamp(min[0] as f32, max[0] as f32)).abs();
    let dz = (focus[2] - focus[2].clamp(min[1] as f32, max[1] as f32)).abs();
    dx * dx + dz * dz <= radius * radius
}

pub(super) fn append_claims(
    out: &mut Vec<Contour>,
    claims: &BTreeSet<[i64; 2]>,
    focus: [f32; 3],
    radius: f32,
) {
    for &[x, z] in claims {
        let (Some(x0), Some(z0)) = (
            x.checked_mul(CLAIM_CHUNK_SIZE),
            z.checked_mul(CLAIM_CHUNK_SIZE),
        ) else {
            continue;
        };
        let (Some(x1), Some(z1)) = (
            x0.checked_add(CLAIM_CHUNK_SIZE),
            z0.checked_add(CLAIM_CHUNK_SIZE),
        ) else {
            continue;
        };
        if !near_rectangle([x0, z0], [x1, z1], focus, radius) {
            continue;
        }
        let sides = [
            ([-1, 0], [x0, z0], [x0, z1]),
            ([1, 0], [x1, z0], [x1, z1]),
            ([0, -1], [x0, z0], [x1, z0]),
            ([0, 1], [x0, z1], [x1, z1]),
        ];
        for (delta, from, to) in sides {
            let neighbor = x.checked_add(delta[0]).zip(z.checked_add(delta[1]));
            if neighbor.is_some_and(|(x, z)| claims.contains(&[x, z])) {
                continue;
            }
            out.push(Contour {
                kind: ContourKind::BuildBorder,
                from,
                to,
                height_hint: focus[1].floor() as i64,
            });
        }
    }
}

pub(super) fn pulse(time: f32, kind: usize) -> f32 {
    let time = if time.is_finite() { time } else { 0.0 };
    0.76 + 0.15 * (time.rem_euclid(std::f32::consts::TAU * 20.0) * 1.25 + kind as f32 * 0.93).sin()
}

/// The shader integrates density inside these bounds; their faces are invisible.
/// A low ceiling keeps claims legible without turning them into tall fences.
pub(super) const MIST_HEIGHT: f32 = 1.2;
pub(super) const VERTICES_PER_BANK: usize = 8;
const BOX_CORNERS: [[f32; 3]; VERTICES_PER_BANK] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
const BOX_INDICES: [u32; 36] = [
    0, 3, 2, 0, 2, 1, 4, 5, 6, 4, 6, 7, 0, 4, 7, 0, 7, 3, 1, 2, 6, 1, 6, 5, 0, 1, 5, 0, 5, 4, 3, 7,
    6, 3, 6, 2,
];

#[derive(Default)]
pub(super) struct Geometry {
    pub positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    strengths: Vec<[f32; 2]>,
    indices: Vec<u32>,
    pub segments: usize,
}

impl Geometry {
    pub fn placeholder_mesh() -> Mesh {
        let mut geometry = Self::default();
        geometry.bank(Vec3::ZERO, Vec3::splat(0.01), [0.0; 2], 0.01);
        geometry.mesh()
    }

    pub fn from_contours(
        contours: &[Contour],
        kind: ContourKind,
        chunks: &ChunkStore,
        journal: &EditJournal,
        remaining: &mut usize,
    ) -> Self {
        let mut result = Self::default();
        for contour in contours.iter().filter(|c| c.kind == kind) {
            let length =
                contour.from[0].abs_diff(contour.to[0]) + contour.from[1].abs_diff(contour.to[1]);
            if length == 0 {
                continue;
            }
            let dx = (contour.to[0] - contour.from[0]).signum() as f32;
            let dz = (contour.to[1] - contour.from[1]).signum() as f32;
            for index in 0..length {
                if *remaining == 0 {
                    return result;
                }
                // Limit attempted samples too: unloaded land cannot consume an
                // unbounded scan while returning no drawable geometry.
                *remaining -= 1;
                let x = contour.from[0] as f32 + index as f32 * dx;
                let z = contour.from[1] as f32 + index as f32 * dz;
                let a = sample(chunks, journal, x, z, dx, dz, contour.height_hint);
                let b = sample(chunks, journal, x + dx, z + dz, dx, dz, contour.height_hint);
                let (Some(a), Some(b)) = (a, b) else { continue };
                if (a - b).abs() > 1.5 {
                    continue;
                }
                result.segment([x, a, z], [x + dx, b, z + dz], [-dz, dx]);
            }
        }
        result
    }

    fn segment(&mut self, from: [f32; 3], to: [f32; 3], normal: [f32; 2]) {
        let from = Vec3::from_array(from);
        let to = Vec3::from_array(to);
        let delta = to - from;
        let center = (from + to) * 0.5 + Vec3::Y * MIST_HEIGHT * 0.5;
        let half = Vec3::new(
            1.0 - normal[0].abs() * 0.22,
            MIST_HEIGHT * 0.5 + delta.y.abs(),
            1.0 - normal[1].abs() * 0.22,
        );
        self.bank(
            center,
            half,
            [delta.x * delta.y, delta.z * delta.y],
            MIST_HEIGHT * 0.5,
        );
    }

    /// Unit-density bank used by construction contours.
    pub(super) fn bank(
        &mut self,
        center: Vec3,
        half: Vec3,
        slope: [f32; 2],
        profile_half_height: f32,
    ) {
        self.weighted_bank(center, half, slope, profile_half_height, 1.0);
    }

    /// Ambient banks smoothly blend their optical density by biome and range.
    pub(super) fn weighted_bank(
        &mut self,
        center: Vec3,
        half: Vec3,
        slope: [f32; 2],
        profile_half_height: f32,
        strength: f32,
    ) {
        let offset = self.positions.len() as u32;
        for corner in BOX_CORNERS {
            self.positions
                .push((center + Vec3::from_array(corner) * half).to_array());
            // These attributes carry a per-proxy analytic density domain. They
            // are read by our own vertex shader, never by standard PBR normals.
            self.normals.push(center.to_array());
            self.colors
                .push(half.extend(profile_half_height).to_array());
            self.uvs.push(slope);
            self.strengths.push([strength, 0.0]);
        }
        self.indices.extend(BOX_INDICES.map(|index| offset + index));
        self.segments += 1;
    }

    pub fn mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, self.strengths);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

fn sample(
    chunks: &ChunkStore,
    journal: &EditJournal,
    x: f32,
    z: f32,
    dx: f32,
    dz: f32,
    hint: i64,
) -> Option<f32> {
    let side = |sign: f32| {
        ground::height(
            chunks,
            journal,
            (x - dz * sign * 0.26).floor() as i64,
            (z + dx * sign * 0.26).floor() as i64,
            hint,
        )
    };
    Some(side(-1.0)?.max(side(1.0)?))
}
