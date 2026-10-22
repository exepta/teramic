use bevy::prelude::*;
use std::time::{Duration, Instant};

#[derive(Resource)]
pub struct AppProcessStats {
    /// Fraction of available logical CPU capacity, 0..100 percent.
    pub cpu_percent: Option<f64>,
    pub resident_bytes: Option<u64>,
    previous: Option<(Instant, u64)>,
    ticks_per_second: Option<f64>,
    cores: usize,
    sampled: Instant,
}

impl Default for AppProcessStats {
    fn default() -> Self {
        Self {
            cpu_percent: None,
            resident_bytes: None,
            previous: None,
            ticks_per_second: None,
            cores: std::thread::available_parallelism().map_or(1, usize::from),
            sampled: Instant::now(),
        }
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<AppProcessStats>()
        .add_systems(Startup, initialize)
        .add_systems(Update, sample);
}

fn initialize(mut stats: ResMut<AppProcessStats>) {
    #[cfg(target_os = "linux")]
    {
        // POSIX exports the kernel's process accounting tick rate; do not assume 100 Hz.
        stats.ticks_per_second = std::process::Command::new("getconf")
            .arg("CLK_TCK")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0);
    }
    read_sample(&mut stats);
}

fn sample(mut stats: ResMut<AppProcessStats>) {
    if stats.sampled.elapsed() >= Duration::from_secs(1) {
        read_sample(&mut stats);
    }
}

fn read_sample(stats: &mut AppProcessStats) {
    let now = Instant::now();
    stats.sampled = now;
    #[cfg(target_os = "linux")]
    {
        stats.resident_bytes = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|text| resident_bytes(&text));
        if let Some(ticks) = std::fs::read_to_string("/proc/self/stat")
            .ok()
            .and_then(|text| cpu_ticks(&text))
        {
            stats.cpu_percent = stats.previous.zip(stats.ticks_per_second).and_then(
                |((previous, previous_ticks), hz)| {
                    cpu_percent(
                        ticks,
                        previous_ticks,
                        now.duration_since(previous).as_secs_f64(),
                        hz,
                        stats.cores,
                    )
                },
            );
            stats.previous = Some((now, ticks));
        } else {
            stats.cpu_percent = None;
            stats.previous = None;
        }
    }
}

fn cpu_ticks(stat: &str) -> Option<u64> {
    // comm may contain spaces and parentheses. Fields after its final ')' start at state (3).
    let (_, fields) = stat.rsplit_once(") ")?;
    let mut fields = fields.split_whitespace().skip(11);
    fields
        .next()?
        .parse::<u64>()
        .ok()?
        .checked_add(fields.next()?.parse::<u64>().ok()?)
}

fn resident_bytes(status: &str) -> Option<u64> {
    let mut value = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace();
    let kib = value.next()?.parse::<u64>().ok()?;
    (value.next()? == "kB")
        .then(|| kib.checked_mul(1024))
        .flatten()
}

fn cpu_percent(ticks: u64, previous: u64, seconds: f64, hz: f64, cores: usize) -> Option<f64> {
    if seconds <= 0.0 || hz <= 0.0 || cores == 0 {
        return None;
    }
    Some(
        (ticks.checked_sub(previous)? as f64 / hz / seconds / cores as f64 * 100.0)
            .clamp(0.0, 100.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_accounting_uses_both_cpu_counters_and_correct_rss_units() {
        assert_eq!(
            cpu_ticks("12 (terra client (dev)) S 0 0 0 0 0 0 0 0 0 0 140 60 0 0"),
            Some(200)
        );
        assert_eq!(
            resident_bytes("Name:\tterra\nVmRSS:\t65536 kB\nVmSize:\t999 kB"),
            Some(64 * 1024 * 1024)
        );
        assert_eq!(cpu_percent(400, 200, 1.0, 100.0, 4), Some(50.0));
        assert_eq!(cpu_percent(1, 200, 1.0, 100.0, 4), None);
        assert_eq!(cpu_ticks("malformed"), None);
        assert_eq!(resident_bytes("VmRSS: 20 MB"), None);
    }
}
