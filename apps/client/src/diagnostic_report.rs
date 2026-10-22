use bevy::{diagnostic::DiagnosticsStore, platform::time::Instant};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct NativeDiagnostics {
    started: Option<Instant>,
    series: BTreeMap<String, Samples>,
}

#[derive(Default)]
struct Samples {
    latest: Option<Instant>,
    values: Vec<f64>,
}

impl NativeDiagnostics {
    pub(crate) fn observe(&mut self, diagnostics: &DiagnosticsStore) {
        let Some(started) = self.started else {
            self.started = Some(Instant::now());
            return;
        };
        for diagnostic in diagnostics.iter() {
            let path = diagnostic.path().as_str();
            if !matches!(
                path.rsplit('/').next(),
                Some(
                    "elapsed_gpu"
                        | "vertex_shader_invocations"
                        | "clipper_primitives_out"
                        | "fragment_shader_invocations"
                        | "compute_shader_invocations"
                )
            ) {
                continue;
            }
            let Some(measurement) = diagnostic.measurement() else {
                continue;
            };
            if measurement.time < started || !measurement.value.is_finite() {
                continue;
            }
            let series = self.series.entry(path.to_owned()).or_default();
            // GPU readback is asynchronous: several main frames can see the
            // same measurement, and startup measurements predate this window.
            if series.latest.is_some_and(|time| time >= measurement.time) {
                continue;
            }
            series.latest = Some(measurement.time);
            series.values.push(measurement.value);
        }
    }

    pub(crate) fn report(&self) {
        let mut gpu_timings = 0;
        for (path, series) in &self.series {
            let Some(summary) = summarize(&series.values) else {
                continue;
            };
            let timing = path.ends_with("/elapsed_gpu");
            gpu_timings += usize::from(timing);
            tracing::info!(
                diagnostic = %path,
                samples = series.values.len(),
                unit = if timing { "ms" } else { "count" },
                mean = summary[0],
                p95 = summary[1],
                p99 = summary[2],
                "Native render diagnostic"
            );
        }
        tracing::info!(
            gpu_timing_series = gpu_timings,
            "GPU values describe individual instrumented passes; no total GPU frame time or draw-call count is inferred"
        );
    }
}

fn summarize(values: &[f64]) -> Option<[f64; 3]> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    let quantile = |fraction: f64| sorted[(sorted.len() as f64 * fraction).ceil() as usize - 1];
    Some([
        values.iter().sum::<f64>() / values.len() as f64,
        quantile(0.95),
        quantile(0.99),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::diagnostic::{Diagnostic, DiagnosticMeasurement, DiagnosticPath};
    use std::time::Duration;

    #[test]
    fn sampling_excludes_old_cpu_invalid_and_duplicate_readbacks() {
        let start = Instant::now();
        let path = DiagnosticPath::new("render/water/elapsed_gpu");
        let mut store = DiagnosticsStore::default();
        let mut gpu = Diagnostic::new(path.clone());
        gpu.add_measurement(DiagnosticMeasurement {
            time: start - Duration::from_millis(1),
            value: 100.0,
        });
        store.add(gpu);
        let mut samples = NativeDiagnostics {
            started: Some(start),
            ..Default::default()
        };
        samples.observe(&store);
        assert!(samples.series.is_empty());
        let mut cpu = Diagnostic::new(DiagnosticPath::new("render/water/elapsed_cpu"));
        cpu.add_measurement(DiagnosticMeasurement {
            time: start,
            value: 50.0,
        });
        store.add(cpu);
        for (offset, value) in [(1, 3.0), (2, 5.0), (3, f64::NAN)] {
            store
                .get_mut(&path)
                .unwrap()
                .add_measurement(DiagnosticMeasurement {
                    time: start + Duration::from_millis(offset),
                    value,
                });
            samples.observe(&store);
            samples.observe(&store);
        }
        assert_eq!(samples.series.len(), 1);
        assert_eq!(samples.series[path.as_str()].values, [3.0, 5.0]);
    }

    #[test]
    fn report_uses_nearest_rank_percentiles_and_arithmetic_mean() {
        let values: Vec<_> = (1..=20).rev().map(f64::from).collect();
        assert_eq!(summarize(&values), Some([10.5, 19.0, 20.0]));
        assert_eq!(summarize(&[]), None);
    }

    #[test]
    fn gpu_compute_passes_outside_the_view_render_prefix_are_included() {
        let start = Instant::now();
        let path = DiagnosticPath::new("lightprobe_radiance_map/elapsed_gpu");
        let mut store = DiagnosticsStore::default();
        let mut diagnostic = Diagnostic::new(path.clone());
        diagnostic.add_measurement(DiagnosticMeasurement {
            time: start,
            value: 2.0,
        });
        store.add(diagnostic);
        let mut samples = NativeDiagnostics {
            started: Some(start),
            ..Default::default()
        };
        samples.observe(&store);
        assert_eq!(samples.series[path.as_str()].values, [2.0]);
    }
}
