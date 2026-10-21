# ZenUML diagnostic evidence corrections

The source and saved-evidence audit ran at `2026-10-21T03:04:54.782219+00:00`.
This is a retrospective documentation correction. Both original diagnostic gzip files and the
ignored registration files remain unchanged; no samples, bounds or official outcomes were revised.

## Original evidence identities

The original selector-normalization manifest SHA-256 was
`5782f7569adc1ebc2ba7f3c3002f0b6858c170ba8cec0e654fea8e2dbd5a5e85`.
Its ignored `zenuml-id-normalization-20261021/experiment.yaml` SHA-256 remains
`3b638f48d0566963949bd6a1458c7facadba4c61046056896320fb889e0e2acb`.
Both recorded an invalid full candidate revision; the actual Git and runner revision is
`b18c436a20e43ac2c8672592322b254270eefa59`.

The original default-CSS manifest SHA-256 was
`44230a8e5f4d09ad7c874b2c7232dfd615a92040d9ff508f56951568df9028a5`.
Its ignored `zenuml-default-css-fast-path-20261021/experiment.yaml` SHA-256 remains
`1a7d91ad5ad91f157fd34a3e8592838e3279027dc720773e88627386adcde31f`.
Only that registration had an invalid full base revision. The manifest already matched the actual
Git and runner base `ff4c78c3965d2bef101eea040c38b63ef6a2318b`.

## Corrected interpretation

The recorded midnight `created_at_utc` was not a verified event time. Corrected manifests use
`null` and preserve the original placeholder. The available records do not certify that either
registration was frozen before observations. A report's `generated_at` marks initialization,
not registration or sampling completion.

The old `base_median_ns` and `candidate_median_ns` values were rounded means, not medians.
Corrected exact means are `24018.0` to `22785.25` ns for selector normalization and `23126.25`
to `22330.5` ns for default CSS. The saved paired bounds are unchanged.

Both stock reports record an actual absolute tool threshold of `50 us` under `generic_v2`;
the registration prose described an advisory `1 us` screen. Diagnostic classification is always
`diagnostic_advisory`, so this mismatch changes neither the saved estimates nor their outcome.
Neither run establishes formal latency admission, and both observed relative changes are below
10%. The controls' diagnostic bounds span zero; no calibrated non-regression is certified.

Each existing gzip contains only the corresponding stock report. It includes input/capability,
SVG identity and executable-size records, not a separate registration or complete rich-report/error
comparison. Correctness tests are separate evidence. These sequential runs use different baselines
and measurement epochs, so their gains must not be added or used to update the v0.9.0
classification.
