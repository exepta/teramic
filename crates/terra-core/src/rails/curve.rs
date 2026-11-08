use super::{FIXED_SCALE, Port, Sample};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    sync::{Arc, Mutex, OnceLock},
};

pub const MAX_CURVE_LENGTH: f64 = 32.0;
pub const MAX_CURVE_GRADE: f64 = 0.5;
pub const MIN_CURVE_RADIUS: f64 = 2.5;
pub const MAX_CURVE_SAMPLES: usize = 257;
pub const DYNAMIC_CACHE_LIMIT: usize = 64;

/// Contextual track construction choices. The automatic choice uses the
/// highest-priority valid route reconstructed from the actual world hit.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum BuildChoice {
    #[default]
    Automatic,
    Straight,
    Curve,
}

/// A genuine cubic Bezier in local sixteenth-block coordinates, including its
/// exact two endpoint tangents. The anchor supplies only an integer translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CurveSpec {
    pub points: [[i32; 3]; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveError {
    Coordinates,
    Degenerate,
    TooLong,
    TooSteep,
    TooTight,
    Reverses,
    SelfIntersection,
    SamplingBudget,
}
impl fmt::Display for CurveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Coordinates => "Bezier control points exceed the local rail span.",
            Self::Degenerate => "The rail curve has no stable forward tangent.",
            Self::TooLong => "A rail connection may be at most 32 blocks long.",
            Self::TooSteep => "The rail connection exceeds its maximum grade.",
            Self::TooTight => "The rail connection bends more tightly than its minimum radius.",
            Self::Reverses => "The rail connection reverses against its endpoint chord.",
            Self::SelfIntersection => "The rail connection intersects its own track width.",
            Self::SamplingBudget => "The rail connection exceeds its sampling budget.",
        })
    }
}
impl std::error::Error for CurveError {}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveMetrics {
    pub length: f64,
    pub max_grade: f64,
    pub min_radius: f64,
}
pub(super) struct Analysis {
    pub metrics: CurveMetrics,
    pub parameters: Arc<[f64]>,
    pub distances: Arc<[f64]>,
}
type AnalysisResult = Result<Arc<Analysis>, CurveError>;
type ValidationEntries = VecDeque<(CurveSpec, AnalysisResult)>;
static VALIDATION: OnceLock<Mutex<ValidationEntries>> = OnceLock::new();
const METRIC_CACHE_LIMIT: usize = super::MAX_PIECES + 256;
type MetricResult = Result<CurveMetrics, CurveError>;
#[derive(Default)]
struct MetricCache {
    entries: BTreeMap<CurveSpec, (u64, MetricResult)>,
    ages: BTreeMap<u64, CurveSpec>,
    clock: u64,
}
impl MetricCache {
    fn stamp(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        if self.clock == 0 {
            self.entries.clear();
            self.ages.clear();
            self.clock = 1;
        }
        self.clock
    }
    fn get(&mut self, curve: CurveSpec) -> Option<MetricResult> {
        let (old, result) = self.entries.get(&curve).copied()?;
        let stamp = self.stamp();
        self.ages.remove(&old);
        self.ages.insert(stamp, curve);
        self.entries.insert(curve, (stamp, result));
        Some(result)
    }
    fn insert(&mut self, curve: CurveSpec, result: MetricResult) {
        let stamp = self.stamp();
        if let Some((old, _)) = self.entries.remove(&curve) {
            self.ages.remove(&old);
        } else if self.entries.len() >= METRIC_CACHE_LIMIT
            && let Some((_, oldest)) = self.ages.pop_first()
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(curve, (stamp, result));
        self.ages.insert(stamp, curve);
    }
}
static METRICS: OnceLock<Mutex<MetricCache>> = OnceLock::new();

#[cfg(test)]
thread_local! { static ANALYSIS_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }

impl CurveSpec {
    pub fn new(points: [[i32; 3]; 4]) -> Option<Self> {
        let curve = Self { points };
        curve.valid().then_some(curve)
    }
    pub fn valid(self) -> bool {
        self.validate().is_ok()
    }
    pub fn validate(self) -> Result<CurveMetrics, CurveError> {
        let cache = METRICS.get_or_init(|| Mutex::new(MetricCache::default()));
        if let Some(result) = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(self)
        {
            return result;
        }
        let result = self.analysis().map(|analysis| analysis.metrics);
        cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(self, result);
        result
    }
    pub fn parameters(self) -> Result<Arc<[f64]>, CurveError> {
        self.analysis().map(|analysis| analysis.parameters.clone())
    }
    pub fn length(self) -> Option<f64> {
        self.validate().ok().map(|metrics| metrics.length)
    }
    pub fn parameter_at_distance(self, distance: f64) -> f64 {
        let Ok(analysis) = self.analysis() else {
            return 0.0;
        };
        let distance = if distance.is_finite() {
            distance.clamp(0.0, analysis.metrics.length)
        } else {
            0.0
        };
        let index = analysis
            .distances
            .partition_point(|value| *value < distance);
        if index == 0 {
            return 0.0;
        }
        if index >= analysis.distances.len() {
            return 1.0;
        }
        let previous = analysis.distances[index - 1];
        let fraction = (distance - previous) / (analysis.distances[index] - previous);
        (index as f64 - 1.0 + fraction) / (MAX_CURVE_SAMPLES - 1) as f64
    }

    /// Connect two existing outward-facing ports; the new segment faces each
    /// one in the opposite direction. Control offsets are integer multiples of
    /// the primitive tangents, so quantization never changes the endpoint grade.
    pub fn between(start: Port, end: Port, anchor: [i64; 3]) -> Option<Self> {
        Self::between_checked(start, end, anchor).ok()
    }

    /// Fit endpoint tangents with a bounded set of independent handle lengths.
    /// The historical equal-handle curve is tried first, preserving successful
    /// connections; asymmetric turns can then use the available approach length.
    pub fn between_checked(start: Port, end: Port, anchor: [i64; 3]) -> Result<Self, CurveError> {
        if !start.connectable || !end.connectable || start.position == end.position {
            return Err(CurveError::Degenerate);
        }
        let start_tangent =
            primitive(start.tangent.map(i32::from)).ok_or(CurveError::Degenerate)?;
        let end_tangent = primitive(end.tangent.map(i32::from)).ok_or(CurveError::Degenerate)?;
        let point = |port: Port| -> Result<[i32; 3], CurveError> {
            let mut result = [0; 3];
            for (axis, coordinate) in result.iter_mut().enumerate() {
                *coordinate = i32::try_from(
                    port.position.0[axis]
                        .checked_sub(
                            anchor[axis]
                                .checked_mul(FIXED_SCALE)
                                .ok_or(CurveError::Coordinates)?,
                        )
                        .ok_or(CurveError::Coordinates)?,
                )
                .map_err(|_| CurveError::Coordinates)?;
            }
            Ok(result)
        };
        let first = point(start)?;
        let last = point(end)?;
        let chord = std::array::from_fn(|axis| f64::from(last[axis]) - f64::from(first[axis]));
        let distance = norm(chord);
        if distance > MAX_CURVE_LENGTH * FIXED_SCALE as f64 + 1e-8 {
            return Err(CurveError::TooLong);
        }
        if distance < 0.5 * FIXED_SCALE as f64 {
            return Err(CurveError::Degenerate);
        }
        let start_vector = start_tangent.map(f64::from);
        let end_vector = end_tangent.map(f64::from);
        if dot(start_vector, chord) < -1e-8 || dot(end_vector, chord) > 1e-8 {
            return Err(CurveError::Reverses);
        }
        let handles = |vector: [f64; 3], projection: f64| {
            let magnitude = norm(vector);
            let mut candidates = vec![
                distance * 0.4,
                projection * 0.85,
                projection * 0.4,
                projection * 0.2,
                projection * 1.25,
                distance * 0.6,
            ];
            // A flat-ended one-block rise over six blocks needs two-block
            // handles: shorter handles bend too tightly, longer ones climb
            // too steeply through the middle of the transition.
            if chord[1].abs() > 1e-8 {
                candidates.insert(1, projection / 3.0);
            }
            let mut result = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                let handle = (candidate / magnitude).round().max(1.0) as i32;
                if !result.contains(&handle) {
                    result.push(handle);
                }
            }
            result
        };
        let start_handles = handles(start_vector, dot(start_vector, chord) / norm(start_vector));
        let end_handles = handles(end_vector, -dot(end_vector, chord) / norm(end_vector));
        let mut first_error = None;
        for start_handle in start_handles {
            for &end_handle in &end_handles {
                let mut second = [0; 3];
                let mut third = [0; 3];
                for axis in 0..3 {
                    second[axis] = first[axis]
                        .checked_add(
                            i32::from(start_tangent[axis])
                                .checked_mul(start_handle)
                                .ok_or(CurveError::Coordinates)?,
                        )
                        .ok_or(CurveError::Coordinates)?;
                    third[axis] = last[axis]
                        .checked_add(
                            i32::from(end_tangent[axis])
                                .checked_mul(end_handle)
                                .ok_or(CurveError::Coordinates)?,
                        )
                        .ok_or(CurveError::Coordinates)?;
                }
                let curve = Self {
                    points: [first, second, third, last],
                };
                match curve.validate() {
                    Ok(_) => return Ok(curve),
                    Err(error) => {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        Err(first_error.unwrap_or(CurveError::Degenerate))
    }
    pub fn sample(self, t: f64) -> Sample {
        let (position, derivative, _) = self.evaluate(t);
        let speed = norm(derivative);
        Sample {
            position,
            tangent: if speed > 1e-12 {
                derivative.map(|value| value / speed)
            } else {
                [0.0; 3]
            },
        }
    }
    pub(super) fn endpoint_tangent(self, end: bool) -> Option<[i16; 3]> {
        let (first, second) = if end {
            (self.points[3], self.points[2])
        } else {
            (self.points[0], self.points[1])
        };
        primitive(std::array::from_fn(|axis| {
            first[axis].checked_sub(second[axis]).unwrap_or(i32::MIN)
        }))
    }
    pub(super) fn analysis(self) -> AnalysisResult {
        let cache = VALIDATION.get_or_init(|| Mutex::new(VecDeque::new()));
        {
            let mut entries = cache
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(index) = entries.iter().position(|(curve, _)| *curve == self) {
                let entry = entries.remove(index).expect("located curve analysis");
                let result = entry.1.clone();
                entries.push_back(entry);
                return result;
            }
        }
        let result = self.analyze().map(Arc::new);
        let mut entries = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if entries.len() >= DYNAMIC_CACHE_LIMIT * 4 {
            entries.pop_front();
        }
        entries.push_back((self, result.clone()));
        result
    }
    fn evaluate(self, t: f64) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let u = 1.0 - t;
        let points = self
            .points
            .map(|point| point.map(|value| f64::from(value) / FIXED_SCALE as f64));
        let position = std::array::from_fn(|axis| {
            u * u * u * points[0][axis]
                + 3.0 * u * u * t * points[1][axis]
                + 3.0 * u * t * t * points[2][axis]
                + t * t * t * points[3][axis]
        });
        let derivative = std::array::from_fn(|axis| {
            3.0 * u * u * (points[1][axis] - points[0][axis])
                + 6.0 * u * t * (points[2][axis] - points[1][axis])
                + 3.0 * t * t * (points[3][axis] - points[2][axis])
        });
        let second = std::array::from_fn(|axis| {
            6.0 * u * (points[2][axis] - 2.0 * points[1][axis] + points[0][axis])
                + 6.0 * t * (points[3][axis] - 2.0 * points[2][axis] + points[1][axis])
        });
        (position, derivative, second)
    }
    fn analyze(self) -> Result<Analysis, CurveError> {
        #[cfg(test)]
        ANALYSIS_COUNT.with(|count| count.set(count.get() + 1));
        if self
            .points
            .iter()
            .flatten()
            .any(|value| value.unsigned_abs() > 32 * 16 + 16)
            || self.points[0].iter().any(|value| !(0..=16).contains(value))
            || self.endpoint_tangent(false).is_none()
            || self.endpoint_tangent(true).is_none()
        {
            return Err(CurveError::Coordinates);
        }
        let start = self.sample(0.0).position;
        let end = self.sample(1.0).position;
        let chord = subtract(end, start);
        if norm(chord) < 0.5 {
            return Err(CurveError::Degenerate);
        }
        if norm(chord) > MAX_CURVE_LENGTH + 1e-8 {
            return Err(CurveError::TooLong);
        }
        let mut max_grade: f64 = 0.0;
        let mut min_radius = f64::INFINITY;
        let mut samples = Vec::with_capacity(MAX_CURVE_SAMPLES);
        let mut distances = Vec::with_capacity(MAX_CURVE_SAMPLES);
        let mut length = 0.0;
        for index in 0..MAX_CURVE_SAMPLES {
            let t = index as f64 / (MAX_CURVE_SAMPLES - 1) as f64;
            let (position, tangent, second) = self.evaluate(t);
            let speed = norm(tangent);
            let horizontal = tangent[0].hypot(tangent[2]);
            if speed < 0.0625 || horizontal < 0.0625 {
                return Err(CurveError::Degenerate);
            }
            if dot(tangent, chord) < -1e-8 {
                return Err(CurveError::Reverses);
            }
            let grade = tangent[1].abs() / horizontal;
            max_grade = max_grade.max(grade);
            if grade > MAX_CURVE_GRADE + 1e-8 {
                return Err(CurveError::TooSteep);
            }
            let turn = norm(cross(tangent, second));
            if turn > 1e-10 {
                let radius = speed.powi(3) / turn;
                min_radius = min_radius.min(radius);
                if radius < MIN_CURVE_RADIUS - 1e-8 {
                    return Err(CurveError::TooTight);
                }
            }
            if let Some(previous) = samples.last() {
                length += norm(subtract(position, *previous));
            }
            samples.push(position);
            distances.push(length);
        }
        if length < 1.0 {
            return Err(CurveError::Degenerate);
        }
        if length > MAX_CURVE_LENGTH + 1e-6 {
            return Err(CurveError::TooLong);
        }
        for first in 0..samples.len() {
            for second in first + 1..samples.len() {
                if distances[second] - distances[first] <= 4.0 {
                    continue;
                }
                let delta = subtract(samples[second], samples[first]);
                if delta[0].hypot(delta[2]) < 2.8 && delta[1].abs() < 1.0 {
                    return Err(CurveError::SelfIntersection);
                }
            }
        }
        let mut parameters = vec![0.0];
        let mut pending = vec![(0.0, 1.0)];
        while let Some((first, last)) = pending.pop() {
            let start = self.sample(first);
            let end = self.sample(last);
            let middle = self.sample((first + last) * 0.5);
            let error = norm(subtract(
                middle.position,
                std::array::from_fn(|axis| (start.position[axis] + end.position[axis]) * 0.5),
            ));
            if norm(subtract(start.position, end.position)) > 0.25
                || error > 0.002
                || dot(start.tangent, end.tangent) < 0.998
            {
                if parameters.len() + pending.len() + 2 >= MAX_CURVE_SAMPLES {
                    return Err(CurveError::SamplingBudget);
                }
                let half = (first + last) * 0.5;
                pending.push((half, last));
                pending.push((first, half));
            } else {
                parameters.push(last);
            }
        }
        Ok(Analysis {
            metrics: CurveMetrics {
                length,
                max_grade,
                min_radius,
            },
            parameters: parameters.into(),
            distances: distances.into(),
        })
    }
}
fn primitive(vector: [i32; 3]) -> Option<[i16; 3]> {
    fn gcd(mut first: u32, mut second: u32) -> u32 {
        while second != 0 {
            (first, second) = (second, first % second);
        }
        first
    }
    let divisor = vector.into_iter().map(i32::unsigned_abs).fold(0, gcd);
    if divisor == 0 {
        return None;
    }
    let mut result = [0; 3];
    for axis in 0..3 {
        result[axis] = i16::try_from(i64::from(vector[axis]) / i64::from(divisor)).ok()?;
    }
    Some(result)
}
fn subtract(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| first[axis] - second[axis])
}
fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
    (0..3).map(|axis| first[axis] * second[axis]).sum()
}
fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}
fn cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_curve_validation_and_route_length_do_not_resample_after_shape_cache_eviction() {
        let curve = CurveSpec::new([[8, 2, 0], [8, 2, 67], [8, 2, 311], [8, 2, 501]]).unwrap();
        let length = curve.length().unwrap();
        // More transient shapes than the detailed-analysis cache can retain.
        for offset in 0..DYNAMIC_CACHE_LIMIT * 4 + 8 {
            let transient = CurveSpec {
                points: [
                    [8, 2, 0],
                    [8, 2, 32 + offset as i32],
                    [8, 2, 350],
                    [8, 2, 480],
                ],
            };
            transient.parameters().unwrap();
        }
        let count = ANALYSIS_COUNT.with(std::cell::Cell::get);
        for _ in 0..1000 {
            assert!(curve.valid());
            assert_eq!(curve.length(), Some(length));
            assert!(
                super::super::Piece::new_curve([0; 3], curve)
                    .unwrap()
                    .valid()
            );
        }
        assert_eq!(ANALYSIS_COUNT.with(std::cell::Cell::get), count);
        let cache = METRICS.get().unwrap().lock().unwrap();
        assert!(cache.entries.len() <= METRIC_CACHE_LIMIT);
    }
}
