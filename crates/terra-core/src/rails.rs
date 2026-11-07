//! Canonical wide-gauge rail geometry and exact, bounded track routing.
mod geometry;
mod network;
mod surfaces;

pub use geometry::{Beam, PartMaterial, Sample, SupportPoint};
pub use network::{DISTANCE_SCALE, Endpoint, RailError, Route, RouteLeg, RouteNetwork};
use serde::{Deserialize, Serialize};
pub use surfaces::Surface;

pub const STRAIGHT: u16 = 196;
pub const CURVE_SMALL: u16 = 197;
pub const CURVE_LARGE: u16 = 198;
pub const SLOPE: u16 = 199;
pub const SLOPE_ENTRY: u16 = 200;
pub const SLOPE_EXIT: u16 = 201;
pub const SWITCH_LEFT: u16 = 202;
pub const SWITCH_RIGHT: u16 = 203;
pub const CROSSING: u16 = 204;
pub const BUFFER: u16 = 205;
pub const MATERIALS: [u16; 10] = [196, 197, 198, 199, 200, 201, 202, 203, 204, 205];
pub const FIXED_SCALE: i64 = 16;
pub const TRACK_WIDTH: f64 = 3.0;
pub const GAUGE: f64 = 2.0;
/// The connection plane is the rail foot, above the wooden sleeper.
pub const DECK_HEIGHT: f64 = 0.125;
pub const RAIL_HEIGHT: f64 = 0.1875;
pub const MAX_COORDINATE: i64 = 999_000;
pub const MAX_PIECES: usize = 8192;
pub const MAX_ROUTE_VISITS: usize = MAX_PIECES * 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    Straight,
    CurveSmall,
    CurveLarge,
    Slope,
    SlopeEntry,
    SlopeExit,
    SwitchLeft,
    SwitchRight,
    Crossing,
    Buffer,
}
impl Kind {
    pub const ALL: [Self; 10] = [
        Self::Straight,
        Self::CurveSmall,
        Self::CurveLarge,
        Self::Slope,
        Self::SlopeEntry,
        Self::SlopeExit,
        Self::SwitchLeft,
        Self::SwitchRight,
        Self::Crossing,
        Self::Buffer,
    ];
    pub const fn material(self) -> u16 {
        STRAIGHT + self as u16
    }
    pub fn from_material(material: u16) -> Option<Self> {
        MATERIALS
            .iter()
            .position(|value| *value == material)
            .map(|index| Self::ALL[index])
    }
    pub const fn is_switch(self) -> bool {
        matches!(self, Self::SwitchLeft | Self::SwitchRight)
    }
    pub const fn key(self) -> &'static str {
        match self {
            Self::Straight => "rail_straight",
            Self::CurveSmall => "rail_curve_small",
            Self::CurveLarge => "rail_curve_large",
            Self::Slope => "rail_slope",
            Self::SlopeEntry => "rail_slope_entry",
            Self::SlopeExit => "rail_slope_exit",
            Self::SwitchLeft => "rail_switch_left",
            Self::SwitchRight => "rail_switch_right",
            Self::Crossing => "rail_crossing",
            Self::Buffer => "rail_buffer",
        }
    }
}
pub fn is_rail(material: u16) -> bool {
    Kind::from_material(material).is_some()
}
pub fn valid_facing(material: u16, facing: u8) -> bool {
    Kind::from_material(material).is_some_and(|kind| facing < if kind.is_switch() { 8 } else { 4 })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub anchor: [i64; 3],
    pub material: u16,
    /// Clockwise yaw about the anchor cell center: 0 = +Z, 1 = +X.
    pub facing: u8,
    pub switched: bool,
}
impl Piece {
    pub fn new(anchor: [i64; 3], material: u16, encoded_facing: u8) -> Option<Self> {
        Self::from_facing(anchor, material, encoded_facing)
    }
    pub fn from_facing(anchor: [i64; 3], material: u16, facing: u8) -> Option<Self> {
        if !valid_facing(material, facing) {
            return None;
        }
        let piece = Self {
            anchor,
            material,
            facing: facing & 3,
            switched: facing & 4 != 0,
        };
        piece.valid().then_some(piece)
    }
    pub fn kind(self) -> Option<Kind> {
        Kind::from_material(self.material)
    }
    pub fn valid(self) -> bool {
        self.facing < 4
            && self
                .kind()
                .is_some_and(|kind| !self.switched || kind.is_switch())
            && self
                .anchor
                .iter()
                .all(|coordinate| coordinate.abs_diff(0) <= (MAX_COORDINATE - 32) as u64)
    }
    pub const fn encoded_facing(self) -> u8 {
        self.facing | if self.switched { 4 } else { 0 }
    }
    /// Complete, cached local geometry after yaw, relative to the anchor block.
    pub fn local_geometry(self) -> &'static [Beam] {
        geometry::layout(self.material, self.encoded_facing())
            .map_or(&[], |layout| layout.beams.as_slice())
    }
    /// Cached visible convex faces. Coplanar overlap is removed geometrically,
    /// with no depth offsets, while beam ownership and UV provenance are retained.
    pub fn local_surfaces(self) -> &'static [Surface] {
        surfaces::local_surfaces(self.material, self.encoded_facing())
    }
    /// World-space beams; use `local_geometry` when translating a whole mesh.
    pub fn geometry(self) -> Vec<Beam> {
        if !self.valid() {
            return vec![];
        }
        self.local_geometry()
            .iter()
            .map(|beam| beam.translated(self.anchor.map(|value| value as f64)))
            .collect()
    }
    /// Cached occupied cells, relative to the anchor; no full bounding-box fill.
    pub fn footprint(self) -> &'static [[i32; 3]] {
        geometry::layout(self.material, self.encoded_facing())
            .map_or(&[], |layout| layout.footprint.as_slice())
    }
    /// Cached local collision/selection boxes, clipped into the requested cell.
    pub fn cell_boxes(self, offset: [i32; 3]) -> &'static [crate::block_shapes::ShapeBox] {
        geometry::layout(self.material, self.encoded_facing())
            .and_then(|layout| layout.cells.get(&offset))
            .map_or(&[], Vec::as_slice)
    }
    pub fn supports(self) -> &'static [SupportPoint] {
        geometry::layout(self.material, self.encoded_facing())
            .map_or(&[], |layout| layout.supports.as_slice())
    }
    /// All physical paths; inactive switch branches remain visible geometry.
    pub fn paths(self) -> Vec<Path> {
        geometry::paths(self)
    }
    /// Only traversable paths for the current switch selection.
    pub fn active_paths(self) -> Vec<Path> {
        self.paths()
            .into_iter()
            .filter(|path| {
                !self.kind().is_some_and(Kind::is_switch) || path.index == u8::from(self.switched)
            })
            .collect()
    }
    pub fn ports(self) -> Vec<Port> {
        geometry::ports(self)
    }
}
pub fn contains_offset(material: u16, facing: u8, offset: [i32; 3]) -> bool {
    geometry::layout(material, facing)
        .is_some_and(|layout| layout.footprint.binary_search(&offset).is_ok())
}
pub fn cell_boxes(
    material: u16,
    facing: u8,
    offset: [i32; 3],
) -> &'static [crate::block_shapes::ShapeBox] {
    geometry::layout(material, facing)
        .and_then(|layout| layout.cells.get(&offset))
        .map_or(&[], Vec::as_slice)
}

/// Exact sixteenth-block coordinates; endpoint matching never uses epsilon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FixedPoint(pub [i64; 3]);
impl FixedPoint {
    pub fn position(self) -> [f64; 3] {
        self.0.map(|value| value as f64 / FIXED_SCALE as f64)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    pub index: u8,
    pub position: FixedPoint,
    /// Primitive outward tangent, including the exact grade where applicable.
    pub tangent: [i16; 3],
    pub connectable: bool,
}
impl Port {
    pub fn connects(self, other: Self) -> bool {
        self.connectable
            && other.connectable
            && self.position == other.position
            && self.tangent.map(|value| -value) == other.tangent
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Path {
    pub piece: Piece,
    pub index: u8,
    pub ports: [u8; 2],
    pub(crate) shape: geometry::PathShape,
}
impl Path {
    /// World-space position and unit tangent, clamped to t in [0, 1].
    pub fn sample(self, t: f64) -> Sample {
        geometry::sample(self, t)
    }
    pub fn length(self) -> f64 {
        geometry::path_length(self)
    }
}

pub type Network = RouteNetwork;
pub type PieceRecord = Piece;

#[cfg(test)]
mod tests;
