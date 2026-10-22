# Intermediate Analysis: First Optimization Build

This is an intermediate investigation record, not a successful final performance
acceptance. Source: the six `*-before.log` and six `*-after.log` files in `logs/`,
parsed by `summarize.mjs`. Each pair uses the same seed, route, sight distance and
60-second offline benchmark. None of these twelve runs enables rain or snowfall:
`snow` identifies the terrain/climate fixture, not an active snow-weather test.

## Comparability Limits

The desktop window manager changed the physical rendering size. `user12` is an
exact 3828 x 2043 pair. The other after runs use 3828 x 2112, while their before
runs use 3828 x 2043, except `user20` before at 3840 x 2055. These are useful
diagnostic comparisons, not exact GPU parity measurements. Animations are disabled
in these twelve runs. The later `*-native-final` validation runs use different
resolution, TAA, animation and duration settings and are not interchangeable.

## Frame And Memory Results

Values show before -> intermediate after. RSS is resident memory at the summary,
not a measured peak. Frame percentiles are milliseconds.

| Fixture | Average FPS | p95 | p99 | RSS MiB | Initial load seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| user12 | 77.60 -> 77.48 | 15.63 -> 15.56 | 17.23 -> 18.46 | 1084.6 -> 1061.8 | 6.63 -> 6.56 |
| user20 | 55.28 -> 55.69 | 22.64 -> 23.92 | 25.47 -> 27.53 | 1605.9 -> 1630.5 | 17.54 -> 16.43 |
| water12 | 61.42 -> 60.72 | 18.43 -> 19.19 | 19.98 -> 26.84 | 946.3 -> 979.6 | 7.47 -> 7.66 |
| water20 | 52.91 -> 52.72 | 23.64 -> 24.01 | 26.04 -> 29.75 | 1465.6 -> 1384.6 | 19.55 -> 19.96 |
| snow12 | 74.77 -> 73.79 | 15.58 -> 15.99 | 17.08 -> 18.70 | 1083.9 -> 1058.9 | 6.37 -> 6.58 |
| snow20 | 60.90 -> 62.54 | 20.96 -> 20.80 | 24.03 -> 24.29 | 1577.3 -> 1557.5 | 15.28 -> 15.74 |

No meaningful general FPS improvement is demonstrated. All six p99 values are
worse in this first after matrix. No runtime error or panic was logged in these
twelve completed runs.

## Geometry Check

Loaded sections, rendered sections and water tiles match within all six pairs.
Mesh counts also match except user20 (22919 -> 22916) and snow20 (21481 -> 21480).
Represented props match except user20 (118783 -> 118779) and snow20
(155526 -> 155518), with respectively three and one fewer prop batches. Visible
mesh entity counts differ with the different render aspect ratios, except the
exact-resolution user12 pair, which matches at 5445.

## CPU Scopes

Mean milliseconds per invocation, including startup and streaming. Nested scopes
must not be added to their parent scope.

| Scope | user20 | water20 | snow20 |
| --- | ---: | ---: | ---: |
| terrain.dispatch | 4.299 -> 4.262 | 3.962 -> 3.703 | 3.547 -> 2.889 |
| props.dispatch | 1.743 -> 1.516 | 1.171 -> 0.929 | 1.048 -> 0.681 |
| props.upload | 1.349 -> 1.174 | 1.232 -> 0.847 | 0.889 -> 0.505 |
| lights.select | 0.662 -> 0.579 | 0.646 -> 0.430 | 0.527 -> 0.299 |
| water.upload | 0.706 -> 0.711 | 0.592 -> 0.435 | 0.128 -> 0.130 |
| terrain.upload | 0.364 -> 0.301 | 0.269 -> 0.136 | 0.554 -> 0.193 |

The CPU changes reduce some repeated work, but the expensive user20 terrain path
is effectively unchanged. Broad resource invalidation and moving-focus queue
rebuilds need direct attribution rather than another speculative threshold change.
The instrumented follow-up adds terrain invalidation counters, separate queue/
appearance/coastal refresh scopes, and streaming section/water-patch publication
scopes. These counters are not present in this first matrix.

## GPU Scopes

The water views are dominated by the transmissive pass:

| Fixture | Opaque GPU ms | Transmissive GPU ms |
| --- | ---: | ---: |
| water12 | 1.613 -> 1.641 | 6.724 -> 6.695 |
| water20 | 1.996 -> 2.016 | 8.085 -> 8.018 |
| user20 | 4.097 -> 4.114 | 0.051 -> 0.048 |
| snow20 | 3.917 -> 3.937 | 0.115 -> 0.112 |

These pass timings are GPU diagnostic means, not whole-frame GPU time. The first
after matrix does not demonstrate a water GPU improvement. The dedicated native
raymarch A/B test must be assessed separately with matching settings.
