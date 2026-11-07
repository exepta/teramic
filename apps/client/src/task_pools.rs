use bevy::app::TaskPoolOptions;

pub(crate) fn options(
    smoke_test: bool,
    requested: Option<&str>,
) -> Result<TaskPoolOptions, &'static str> {
    let mut options = TaskPoolOptions::default();
    let Some(requested) = requested.filter(|_| smoke_test) else {
        return Ok(options);
    };
    options.compute.max_threads = requested
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&threads| threads > 0)
        .ok_or("TERRA_COMPUTE_THREADS expects a positive integer in smoke tests")?;
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::TaskPoolThreadAssignmentPolicy;

    fn assert_policy(
        actual: &TaskPoolThreadAssignmentPolicy,
        expected: &TaskPoolThreadAssignmentPolicy,
    ) {
        assert_eq!(actual.min_threads, expected.min_threads);
        assert_eq!(actual.max_threads, expected.max_threads);
        assert_eq!(actual.percent, expected.percent);
        assert!(actual.on_thread_spawn.is_none());
        assert!(actual.on_thread_destroy.is_none());
    }

    fn assert_options(actual: &TaskPoolOptions, compute_cap: usize) {
        let mut expected = TaskPoolOptions::default();
        expected.compute.max_threads = compute_cap;
        assert_eq!(actual.min_total_threads, expected.min_total_threads);
        assert_eq!(actual.max_total_threads, expected.max_total_threads);
        assert_policy(&actual.io, &expected.io);
        assert_policy(&actual.async_compute, &expected.async_compute);
        assert_policy(&actual.compute, &expected.compute);
    }

    #[test]
    fn production_ignores_even_invalid_smoke_overrides() {
        for requested in [None, Some("8"), Some("0"), Some("invalid"), Some("")] {
            let actual = options(false, requested).unwrap();
            assert_options(&actual, TaskPoolOptions::default().compute.max_threads);
        }
    }

    #[test]
    fn smoke_without_an_override_retains_every_default() {
        let actual = options(true, None).unwrap();
        assert_options(&actual, TaskPoolOptions::default().compute.max_threads);
    }

    #[test]
    fn smoke_caps_only_compute_workers_and_preserves_io_and_async() {
        for (requested, cap) in [("6", 6), ("8", 8), ("12", 12), (" 8 ", 8), ("1", 1)] {
            assert_options(&options(true, Some(requested)).unwrap(), cap);
        }
    }

    #[test]
    fn malformed_smoke_caps_are_rejected() {
        for requested in [
            "0",
            "-1",
            "8.0",
            "invalid",
            "",
            " ",
            "184467440737095516160",
        ] {
            assert!(options(true, Some(requested)).is_err(), "{requested:?}");
        }
    }
}
