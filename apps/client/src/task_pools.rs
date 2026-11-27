use bevy::app::TaskPoolOptions;

pub(crate) fn initialize(
    smoke_test: bool,
    requested: Option<&str>,
) -> Result<TaskPoolOptions, &'static str> {
    let options = options(smoke_test, requested)?;
    // Bevy's three task pools are process-wide singletons. A client which
    // opens its integrated host during bootstrap must select their sizes
    // before the server installs its headless MinimalPlugins.
    options.create_default_pools();
    Ok(options)
}

fn options(smoke_test: bool, requested: Option<&str>) -> Result<TaskPoolOptions, &'static str> {
    let mut options = TaskPoolOptions::default();
    if !smoke_test || requested == Some("interactive") {
        // Leave logical CPUs available to desktop, audio and graphics driver.
        options.max_total_threads =
            interactive_budget(std::thread::available_parallelism().map_or(1, usize::from));
        options.compute.max_threads = 4;
        return Ok(options);
    }
    let Some(requested) = requested.filter(|_| smoke_test) else {
        return Ok(options);
    };
    options.compute.max_threads = requested
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&threads| threads > 0)
        .ok_or(
            "TERRA_COMPUTE_THREADS expects a positive integer or 'interactive' in smoke tests",
        )?;
    Ok(options)
}

fn interactive_budget(available: usize) -> usize {
    available.saturating_sub(2).clamp(1, 10)
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
            assert_eq!(
                actual.max_total_threads,
                interactive_budget(std::thread::available_parallelism().map_or(1, usize::from))
            );
            assert_eq!(actual.compute.max_threads, 4);
        }
    }

    #[test]
    fn smoke_without_an_override_retains_every_default() {
        let actual = options(true, None).unwrap();
        assert_options(&actual, TaskPoolOptions::default().compute.max_threads);
    }

    #[test]
    fn smoke_caps_only_compute_workers_and_preserves_io_and_async() {
        let interactive = options(false, None).unwrap();
        let replay = options(true, Some("interactive")).unwrap();
        assert_eq!(interactive.max_total_threads, replay.max_total_threads);
        assert_policy(&interactive.compute, &replay.compute);
        assert_policy(&interactive.async_compute, &replay.async_compute);
        assert_policy(&interactive.io, &replay.io);
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

    #[test]
    fn integrated_host_uses_the_client_process_worker_budget() {
        use bevy::tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};

        // Other tests may have built Bevy apps first. Exercise each startup
        // mode in a clean process so its global pools have no prior owner.
        const CHILD_MODE: &str = "TERAMIC_TEST_WORKER_BUDGET_MODE";
        let Ok(mode) = std::env::var(CHILD_MODE) else {
            for mode in ["smoke", "interactive"] {
                let result = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "task_pools::tests::integrated_host_uses_the_client_process_worker_budget",
                    ])
                    .env(CHILD_MODE, mode)
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{mode}: {}{}",
                    String::from_utf8_lossy(&result.stdout),
                    String::from_utf8_lossy(&result.stderr)
                );
            }
            return;
        };

        assert!(ComputeTaskPool::try_get().is_none());
        let options = initialize(mode == "smoke", Some("1")).unwrap();
        let _host = terra_server::headless_app(terra_core::GameConfig::default());
        let compute = ComputeTaskPool::get().thread_num();
        if mode == "smoke" {
            assert_eq!(compute, 1);
        } else {
            assert!((1..=4).contains(&compute));
            let total =
                compute + AsyncComputeTaskPool::get().thread_num() + IoTaskPool::get().thread_num();
            // On one- and two-core machines each pool still needs one worker.
            assert!(total <= options.max_total_threads.max(3));
        }
    }
}
