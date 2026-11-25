#![cfg(feature = "ascii")]

use std::fs;
#[cfg(unix)]
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::fd::FromRawFd;
use std::process::{Command, Output, Stdio};

use merman_ascii_test_contracts::ascii_resource_boundary_contract;

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cli");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait cli")
}

fn run_ascii_with_resource_limit(limit_id: &str, max: u64, source: &str) -> Output {
    let override_arg = format!("{limit_id}={max}");
    run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-limit",
            &override_arg,
            "-",
        ],
        source,
    )
}

#[cfg(feature = "diagram-git-graph")]
#[test]
fn gitgraph_cli_reports_tracks_parallel_records_and_complete_width_fallback() {
    let body = "gitGraph LR:\ncommit id:\"A\"\nbranch feature\ncommit id:\"F\"\ncheckout main\ncommit id:\"B\"\nmerge feature id:\"M\" tag:\"v1\"\n";
    for (parallel, width) in [(false, "80"), (false, "20"), (true, "80")] {
        let source = if parallel {
            format!("---\nconfig:\n  gitGraph:\n    parallelCommits: true\n---\n{body}")
        } else {
            body.to_string()
        };
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                "ascii",
                "--ascii-report",
                "--ascii-max-width",
                width,
                "--ascii-overflow",
                "fallback",
                "-",
            ],
            &source,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = assert_plain_ascii_report(&output.stdout);
        assert_eq!(report["family"], "gitGraph");
        assert_eq!(
            report["projection"],
            if !parallel && width == "80" {
                "diagrammatic"
            } else {
                "structured_text"
            }
        );
        assert_eq!(
            report["outcome"],
            if width == "20" { "fallback" } else { "primary" }
        );
        assert_eq!(report["fallback_attempted"], width == "20");
        assert!(report["emitted_width"].as_u64().unwrap() <= width.parse::<u64>().unwrap());
        let records: String = report["text"]
            .as_str()
            .unwrap()
            .split_once("GitGraph records")
            .unwrap()
            .1
            .split_whitespace()
            .collect();
        assert_eq!(records.matches("[commit]").count(), 4);
        assert!(records.contains("tags=[\"v1\"]parents=[\"B\",\"F\"]"));
        if parallel {
            assert!(records.contains("\"parallelCommits\":true"));
        }
    }
    let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .args(["capabilities", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let gitgraph = capabilities["ascii"]["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family["family"] == "gitgraph")
        .unwrap();
    assert_eq!(gitgraph["primary_projection"], "diagrammatic");
    assert_eq!(gitgraph["support_level"], "partial");
}

fn assert_plain_ascii_report(bytes: &[u8]) -> serde_json::Value {
    assert!(
        !bytes.windows(2).any(|window| window == b"\x1b["),
        "report bytes must not contain ANSI escapes: {bytes:?}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(bytes).expect("ASCII report should be one JSON artifact");
    assert_eq!(report["kind"], "ascii");
    assert_eq!(report["encoding"], "plain");
    assert_eq!(report["schema_version"], 4);
    let text = report["text"].as_str().expect("report text");
    assert!(!text.contains('\u{1b}'), "report text must be escape-free");
    report
}

fn assert_ascii_error_report(
    bytes: &[u8],
    expected_code: &str,
    expected_category: &str,
) -> serde_json::Value {
    assert!(
        !bytes.windows(2).any(|window| window == b"\x1b["),
        "error report bytes must not contain ANSI escapes: {bytes:?}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(bytes).expect("ASCII error report should be one JSON artifact");
    assert_eq!(report["kind"], "ascii_error");
    assert_eq!(report["encoding"], "plain");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["error"]["code"], expected_code);
    assert_eq!(report["error"]["category"], expected_category);
    assert!(
        report["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.contains('\u{1b}')),
        "error report message must be terminal-safe: {report}"
    );
    report
}

#[cfg(unix)]
fn open_pty() -> (fs::File, fs::File) {
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes both descriptors on success; each descriptor
    // is immediately transferred into exactly one owned File.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(
        result,
        0,
        "openpty failed: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: successful openpty returned two fresh, owned descriptors.
    unsafe { (fs::File::from_raw_fd(master), fs::File::from_raw_fd(slave)) }
}

#[cfg(unix)]
fn read_pty(mut master: fs::File) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => output.extend_from_slice(&buffer[..read]),
            Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
            Err(error) => panic!("read PTY output: {error}"),
        }
    }
    output
}

#[test]
fn cli_renders_unicode_ascii_output_to_stdout() {
    let output = run_with_stdin(
        &["render", "--format", "unicode", "-"],
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("┌"));
    assert!(stdout.contains("Hello"));
    assert!(stdout.contains("►"));
}

#[test]
fn cli_ascii_diagnostic_does_not_echo_source_or_terminal_controls() {
    let source = "not-a-diagram\u{1b}]8;;https://example.invalid\u{7}link\u{202e}";

    let output = run_with_stdin(&["render", "--format", "ascii", "-"], source);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "failure must not write a payload");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("No Mermaid diagram type detected"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains(source),
        "source leaked into stderr: {stderr:?}"
    );
    for control in ['\u{1b}', '\u{7}', '\u{202e}'] {
        assert!(
            !stderr.contains(control),
            "raw control {control:?} leaked into stderr: {stderr:?}"
        );
    }
}

#[test]
fn cli_clap_diagnostic_normalizes_untrusted_terminal_controls() {
    for value in ["12\rOWNED", "12\u{202e}34"] {
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                "ascii",
                "--ascii-max-width",
                value,
                "-",
            ],
            "flowchart TD\nA --> B",
        );

        assert!(!output.status.success());
        assert!(output.stdout.is_empty(), "failure must not write a payload");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(
            stderr.contains("invalid value"),
            "unexpected stderr:\n{stderr}"
        );
        for control in ['\r', '\u{202e}'] {
            assert!(
                !stderr.contains(control),
                "raw control {control:?} leaked into stderr: {stderr:?}"
            );
        }
    }
}

#[test]
fn cli_renders_sequence_mirrored_actors_when_requested() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--sequence-mirror-actors",
            "-",
        ],
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("┌─┴─┐     ┌─┴─┐"),
        "expected mirrored bottom participant boxes:\n{stdout}"
    );
}

#[test]
fn cli_allows_charset_override_for_text_output() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--ascii-charset",
            "ascii",
            "-",
        ],
        "flowchart TD\nA[Hello] --> B[World]",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("+"), "{stdout}");
    assert!(!stdout.contains("┌"), "{stdout}");
}

#[test]
fn cli_allows_xychart_plot_dimension_overrides() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--xychart-vertical-plot-height",
            "8",
            "--xychart-category-band-width",
            "5",
            "-",
        ],
        r#"xychart
title "Sales"
x-axis [Jan, Feb]
y-axis 0 --> 10
bar [2, 8]
"#,
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Sales"), "{stdout}");
    assert!(stdout.contains("Jan"), "{stdout}");
}

#[test]
fn cli_renders_shipped_reference_diagram_families_to_stdout() {
    let cases = [
        ("classDiagram\nclass Animal", "Animal"),
        ("erDiagram\nCUSTOMER", "CUSTOMER"),
        (
            r#"xychart
title "Sales"
x-axis [Jan, Feb]
y-axis 0 --> 10
bar [2, 8]
"#,
            "Sales",
        ),
    ];

    for (input, expected) in cases {
        let output = run_with_stdin(&["render", "--format", "ascii", "-"], input);

        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(
            stdout.contains(expected),
            "expected {expected:?} in stdout:\n{stdout}"
        );
    }
}

#[test]
fn family_reports_keep_geometry_and_complete_width_fallback() {
    for (family, source, fields) in [
        (
            "architecture",
            "architecture-beta\nservice api(server)[API]\nservice db(database)[Database]\napi:R --> L:db\n",
            &["API", "Database", "api", "db"][..],
        ),
        (
            "usecase",
            "usecase-beta\nactor Customer(\"Customer\")\nCheckout(\"Place order\")\nCustomer --> Checkout\n",
            &["Customer", "Place order"][..],
        ),
        (
            "block",
            "block-beta\ncolumns 3\nA[\"Request\"]:2 space\nB[\"Result\"]:3\nA --> B\n",
            &["Request", "Result"][..],
        ),
        (
            "c4",
            "C4Container\nPerson(user, \"Customer\")\nBoundary(zone, \"Services\") {\nContainer(api, \"API\", \"Rust\", \"Serves requests\")\n}\nRel_Back(api, user, \"Replies\", \"HTTPS\")\n",
            &[
                "Customer",
                "Services",
                "API",
                "Rust",
                "Serves requests",
                "Replies",
                "HTTPS",
            ][..],
        ),
    ] {
        for width in [10, 80] {
            let max_width = width.to_string();
            let output = run_with_stdin(
                &[
                    "render",
                    "-",
                    "--format",
                    "unicode",
                    "--ascii-report",
                    "--ascii-color",
                    "plain",
                    "--ascii-max-width",
                    &max_width,
                    "--ascii-overflow",
                    "fallback",
                ],
                source,
            );
            assert!(
                output.status.success(),
                "{family}/{width}: {:?}",
                output.stderr
            );
            let report = assert_plain_ascii_report(&output.stdout);
            assert_eq!(report["family"], family);
            assert_eq!(report["layout_profile"], "canonical");
            assert_eq!(report["requested_graph_presentation"], "strict");
            assert_eq!(report["compact_attempted"], false);
            assert_eq!(report["emitted_overflowed"], false);
            assert!(report["emitted_width"].as_u64().unwrap() <= width);
            assert_eq!(
                report["projection"],
                if width == 10 {
                    "structured_text"
                } else {
                    "diagrammatic"
                }
            );
            assert_eq!(
                report["outcome"],
                if width == 10 { "fallback" } else { "primary" }
            );
            let compact: String = report["text"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .collect();
            for field in fields {
                let expected: String = field.split_whitespace().collect();
                assert!(compact.contains(&expected), "{family}/{width}: {report}");
            }
            if width == 10 {
                let relationships: &[&str] = if family == "usecase" {
                    &[
                        "E1:N1[open]--solidassociation--N2[point]",
                        "id=\"Checkout\"",
                    ]
                } else if family == "block" {
                    &[
                        "start=\"A\"",
                        "end=\"B\"",
                        "end-marker=\"arrow_point\"",
                        "columns=3",
                        "span=2",
                        "type=\"space\"",
                        "children=[",
                    ]
                } else if family == "architecture" {
                    &[
                        "N1[service]id=\"api\"title=\"API\"",
                        "N2[service]id=\"db\"title=\"Database\"",
                        "E1lhs=[N1]id=\"api\"lhsDir=\"R\"lhsInto=null",
                        "E1rhs=[N2]id=\"db\"rhsDir=\"L\"rhsInto=true",
                    ]
                } else {
                    &[
                        "E1[relationship]type=\"rel_b\"source=[S2]alias=\"api\"target=[S1]alias=\"user\"heads=source",
                        "S2techn={text:\"Rust\"}",
                        "parentBoundary=[B2]alias=\"zone\"",
                    ]
                };
                for expected in relationships {
                    assert!(compact.contains(expected), "missing {expected:?}: {report}");
                }
            }
        }
    }
}

#[test]
fn agentflow_reports_distinguish_spatial_records_and_complete_width_fallback() {
    for (source, projection) in [
        ("agentflow-beta\nA[Request] --> B[Result]\n", "diagrammatic"),
        (
            "agentflow-beta\nA[Request]@{instruction: inspect, params: {retries: 3}} --> B[Result]\n",
            "structured_text",
        ),
    ] {
        for width in [10, 120] {
            let limit = width.to_string();
            let output = run_with_stdin(
                &[
                    "render",
                    "-",
                    "--format",
                    "unicode",
                    "--ascii-report",
                    "--ascii-color",
                    "plain",
                    "--ascii-max-width",
                    &limit,
                    "--ascii-overflow",
                    "fallback",
                    "--output",
                    "-",
                ],
                source,
            );
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report = assert_plain_ascii_report(&output.stdout);
            assert_eq!(report["family"], "agentflow");
            assert_eq!(
                report["projection"],
                if width == 10 {
                    "structured_text"
                } else {
                    projection
                }
            );
            assert!(!report["emitted_overflowed"].as_bool().unwrap());
            assert!(report["emitted_width"].as_u64().unwrap() <= width);
            let text = report["text"].as_str().unwrap();
            let compact: String = text.split_whitespace().collect();
            assert!(compact.contains("Request") && compact.contains("Result"));
            if source.contains("instruction") {
                assert!(compact.contains("inspect") && compact.contains("retries"));
                assert!(compact.contains("[sequence-->]source=[N1]id=\"A\"target=[N2]id=\"B\""));
            }
        }
    }
}

#[cfg(feature = "diagram-gantt")]
#[test]
fn gantt_cli_capabilities_admit_only_canonical_strict_output_and_plain_fallback() {
    let output = run_with_stdin(&["capabilities", "--json"], "");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ascii = &capabilities["ascii"];
    let gantt = ascii["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family["family"] == "gantt")
        .expect("Gantt should be in the total ASCII capability matrix");
    assert_eq!(gantt["support_level"], "partial");
    assert_eq!(gantt["semantic_coverage"], "partial");
    assert_eq!(gantt["primary_projection"], "diagrammatic");
    assert_eq!(gantt["structured_text_fallback"], true);
    assert_eq!(gantt["layout_profiles"], serde_json::json!(["canonical"]));
    assert_eq!(gantt["graph_presentations"], serde_json::json!(["strict"]));
    assert_eq!(
        gantt["width_profiles"],
        serde_json::json!(["unicode", "cjk"])
    );
    assert_eq!(
        gantt["encodings"],
        serde_json::json!(["plain", "ansi16", "ansi256", "truecolor", "html"])
    );
    assert_eq!(gantt["fallback_encodings"], serde_json::json!(["plain"]));
    assert!(
        ascii["detected_type_mappings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mapping| mapping["detected_type"] == "gantt" && mapping["family"] == "gantt")
    );
}

#[cfg(feature = "diagram-gantt")]
#[test]
fn gantt_cli_reports_distinguish_timeline_compact_records_and_width_fallback() {
    for compact_mode in [false, true] {
        let prefix = if compact_mode {
            "---\nconfig:\n  gantt:\n    displayMode: compact\n---\n"
        } else {
            ""
        };
        let source = format!(
            "{prefix}gantt\ndateFormat YYYY-MM-DD\nsection Delivery work\nBuild phase: done, build, 2026-01-01, 2d\nReview phase: crit, active, review, after build, 1d\n"
        );
        for width in [59_u64, 60] {
            let limit = width.to_string();
            let output = run_with_stdin(
                &[
                    "render",
                    "-",
                    "--format",
                    "unicode",
                    "--ascii-report",
                    "--ascii-color",
                    "plain",
                    "--ascii-max-width",
                    &limit,
                    "--ascii-overflow",
                    "fallback",
                ],
                &source,
            );
            assert!(
                output.status.success(),
                "compact {compact_mode}, width {width}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report = assert_plain_ascii_report(&output.stdout);
            let fallback = !compact_mode && width < 60;
            assert_eq!(report["family"], "gantt");
            assert_eq!(report["layout_profile"], "canonical");
            assert_eq!(report["requested_graph_presentation"], "strict");
            assert_eq!(report["requested_max_width"], width);
            assert_eq!(report["compact_attempted"], false);
            assert_eq!(report["fallback_capability"], "available");
            assert_eq!(report["fallback_attempted"], fallback);
            assert_eq!(report["primary_overflowed"], fallback);
            assert_eq!(report["emitted_overflowed"], false);
            assert!(report["emitted_width"].as_u64().unwrap() <= width);
            assert_eq!(
                report["fallback_reason"],
                if fallback {
                    serde_json::json!("primary_overflow")
                } else {
                    serde_json::Value::Null
                }
            );
            assert_eq!(
                report["outcome"],
                if fallback { "fallback" } else { "primary" }
            );
            assert_eq!(
                report["projection"],
                if compact_mode || fallback {
                    "structured_text"
                } else {
                    "diagrammatic"
                }
            );
            let text = report["text"].as_str().unwrap();
            assert_eq!(
                text.contains("Gantt timeline (elapsed time)"),
                !compact_mode && !fallback
            );
            let compact: String = text.split_whitespace().collect();
            for expected in [
                "T1[task]id=\"build\"",
                "T2[task]id=\"review\"",
                "T1interval(UTC):2026-01-01->2026-01-03",
                "T2interval(UTC):2026-01-03->2026-01-04",
                "T1details:{",
                "T2details:{",
                "startConstraint:{\"kind\":\"after\",\"dependencyIds\":[\"build\"]}",
                "ganttdetails:{",
            ] {
                assert!(compact.contains(expected), "missing {expected:?}: {report}");
            }
            if compact_mode {
                assert!(compact.contains("sourceConfig:{\"gantt\":{\"displayMode\":\"compact\"}}"));
            }
        }
    }
}

#[cfg(feature = "diagram-zenuml")]
#[test]
fn zenuml_cli_capabilities_admit_only_canonical_strict_output_and_plain_fallback() {
    let output = run_with_stdin(&["capabilities", "--json"], "");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ascii = &capabilities["ascii"];
    let zenuml = ascii["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family["family"] == "zenuml")
        .expect("ZenUML should be in the total ASCII capability matrix");
    assert_eq!(zenuml["support_level"], "partial");
    assert_eq!(zenuml["semantic_coverage"], "partial");
    assert_eq!(zenuml["primary_projection"], "diagrammatic");
    assert_eq!(zenuml["structured_text_fallback"], true);
    assert_eq!(zenuml["layout_profiles"], serde_json::json!(["canonical"]));
    assert_eq!(zenuml["graph_presentations"], serde_json::json!(["strict"]));
    assert_eq!(
        zenuml["width_profiles"],
        serde_json::json!(["unicode", "cjk"])
    );
    assert_eq!(
        zenuml["encodings"],
        serde_json::json!(["plain", "ansi16", "ansi256", "truecolor", "html"])
    );
    assert_eq!(zenuml["fallback_encodings"], serde_json::json!(["plain"]));
    assert!(
        ascii["detected_type_mappings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mapping| mapping["detected_type"] == "zenuml" && mapping["family"] == "zenuml")
    );
}

#[cfg(feature = "diagram-zenuml")]
#[test]
fn zenuml_cli_reports_keep_the_basic_scene_and_complete_width_fallback() {
    let source = "zenuml\nAlice->Bob: Hello\nBob-->Alice: Reply\n";
    for width in [None, Some(10_u64)] {
        let mut args = vec![
            "render",
            "-",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-color",
            "plain",
            "--ascii-overflow",
            "fallback",
        ];
        if width.is_some() {
            args.extend(["--ascii-max-width", "10"]);
        }
        let output = run_with_stdin(&args, source);
        assert!(
            output.status.success(),
            "width {width:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = assert_plain_ascii_report(&output.stdout);
        assert_eq!(report["family"], "zenuml");
        assert_eq!(report["layout_profile"], "canonical");
        assert_eq!(report["requested_graph_presentation"], "strict");
        assert_eq!(report["requested_max_width"], serde_json::json!(width));
        assert_eq!(report["compact_attempted"], false);
        assert_eq!(report["fallback_capability"], "available");
        assert_eq!(report["fallback_attempted"], width.is_some());
        assert_eq!(report["primary_overflowed"], width.is_some());
        assert_eq!(report["emitted_overflowed"], false);
        assert_eq!(
            report["fallback_reason"],
            if width.is_some() {
                serde_json::json!("primary_overflow")
            } else {
                serde_json::Value::Null
            }
        );
        assert_eq!(
            report["projection"],
            if width.is_some() {
                "structured_text"
            } else {
                "diagrammatic"
            }
        );
        assert_eq!(
            report["outcome"],
            if width.is_some() {
                "fallback"
            } else {
                "primary"
            }
        );
        if let Some(width) = width {
            assert!(report["emitted_width"].as_u64().unwrap() <= width);
        }
        let compact: String = report["text"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .collect();
        for expected in [
            "P1[participant]name=\"Alice\"",
            "P2[participant]name=\"Bob\"",
            "S1[message]id=\"zenuml-statement-0\"parent=nullsection=nullfrom=[P1]name=\"Alice\"to=[P2]name=\"Bob\"style=asynchronousassignment=nulllabel=\"Hello\"bodyCount=0",
            "S2[return]id=\"zenuml-statement-1\"parent=nullsection=nullfrom=[P2]name=\"Bob\"to=[P1]name=\"Alice\"label=\"Reply\"",
            "S1details:{",
            "S2details:{",
            "explicit_from:\"Alice\"",
            "explicit_to:\"Alice\"",
            "span:{",
            "occurrences:[",
        ] {
            assert!(compact.contains(expected), "missing {expected:?}: {report}");
        }
    }
}

#[cfg(feature = "diagram-pie")]
#[test]
fn pie_cli_capabilities_admit_only_canonical_strict_output_and_plain_fallback() {
    let output = run_with_stdin(&["capabilities", "--json"], "");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ascii = &capabilities["ascii"];
    let pie = ascii["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family["family"] == "pie")
        .expect("Pie should be in the total ASCII capability matrix");
    assert_eq!(pie["support_level"], "partial");
    assert_eq!(pie["semantic_coverage"], "partial");
    assert_eq!(pie["primary_projection"], "diagrammatic");
    assert_eq!(pie["structured_text_fallback"], true);
    assert_eq!(pie["layout_profiles"], serde_json::json!(["canonical"]));
    assert_eq!(pie["graph_presentations"], serde_json::json!(["strict"]));
    assert_eq!(pie["width_profiles"], serde_json::json!(["unicode", "cjk"]));
    assert_eq!(
        pie["encodings"],
        serde_json::json!(["plain", "ansi16", "ansi256", "truecolor", "html"])
    );
    assert_eq!(pie["fallback_encodings"], serde_json::json!(["plain"]));
    assert!(
        ascii["detected_type_mappings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mapping| mapping["detected_type"] == "pie" && mapping["family"] == "pie")
    );
}

#[cfg(feature = "diagram-pie")]
#[test]
fn pie_cli_reports_keep_actual_projection_and_complete_keyed_records() {
    for (dogs, cats, primary_projection) in [(386, 85, "diagrammatic"), (0, 0, "structured_text")] {
        let source = format!("pie showData\ntitle Pets\n\"Dogs\": {dogs}\n\"Cats\": {cats}\n");
        for width in [10, 20, 80] {
            let limit = width.to_string();
            let output = run_with_stdin(
                &[
                    "render",
                    "-",
                    "--format",
                    "unicode",
                    "--ascii-report",
                    "--ascii-color",
                    "plain",
                    "--ascii-max-width",
                    &limit,
                    "--ascii-overflow",
                    "fallback",
                    "--ascii-graph-presentation",
                    "strict",
                ],
                &source,
            );
            assert!(
                output.status.success(),
                "width {width}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report = assert_plain_ascii_report(&output.stdout);
            assert_eq!(report["family"], "pie");
            assert_eq!(report["layout_profile"], "canonical");
            assert_eq!(report["requested_graph_presentation"], "strict");
            assert_eq!(report["requested_max_width"], width);
            assert_eq!(report["compact_attempted"], false);
            assert_eq!(report["fallback_capability"], "available");
            assert_eq!(report["fallback_attempted"], width < 80);
            assert_eq!(
                report["fallback_reason"],
                if width < 80 {
                    serde_json::json!("primary_overflow")
                } else {
                    serde_json::Value::Null
                }
            );
            assert_eq!(report["primary_overflowed"], width < 80);
            assert_eq!(
                report["outcome"],
                if width < 80 { "fallback" } else { "primary" }
            );
            assert_eq!(
                report["projection"],
                if width < 80 {
                    "structured_text"
                } else {
                    primary_projection
                }
            );
            assert_eq!(report["emitted_overflowed"], false);
            assert!(report["emitted_width"].as_u64().unwrap() <= width);
            assert_eq!(
                report["lossiness"],
                if width < 80 && primary_projection == "diagrammatic" {
                    "presentation_only"
                } else {
                    "none"
                }
            );
            let text = report["text"].as_str().unwrap();
            let compact: String = text.split_whitespace().collect();
            for expected in [
                format!("P1label=\"Dogs\"value={dogs}"),
                format!("P2label=\"Cats\"value={cats}"),
                "Pets".to_string(),
            ] {
                assert!(
                    compact.contains(&expected),
                    "missing {expected:?}: {report}"
                );
            }
            assert!(compact.find("P1label=").unwrap() < compact.find("P2label=").unwrap());
        }
    }
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_cli_capabilities_admit_canonical_graphs_and_complete_plain_records() {
    let output = run_with_stdin(&["capabilities", "--json"], "");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ascii = &capabilities["ascii"];
    let requirement = ascii["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|family| family["family"] == "requirement")
        .expect("Requirement should be in the total ASCII capability matrix");
    assert_eq!(requirement["support_level"], "partial");
    assert_eq!(requirement["semantic_coverage"], "partial");
    assert_eq!(requirement["primary_projection"], "diagrammatic");
    assert_eq!(requirement["structured_text_fallback"], true);
    assert_eq!(
        requirement["layout_profiles"],
        serde_json::json!(["canonical"])
    );
    assert_eq!(
        requirement["graph_presentations"],
        serde_json::json!(["strict", "adaptive"])
    );
    assert_eq!(
        requirement["width_profiles"],
        serde_json::json!(["unicode", "cjk"])
    );
    assert_eq!(
        requirement["encodings"],
        serde_json::json!(["plain", "ansi16", "ansi256", "truecolor", "html"])
    );
    assert_eq!(
        requirement["fallback_encodings"],
        serde_json::json!(["plain"])
    );
    assert!(
        ascii["detected_type_mappings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mapping| {
                mapping["detected_type"] == "requirement" && mapping["family"] == "requirement"
            })
    );
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_cli_reports_distinguish_spatial_records_and_complete_width_fallback() {
    let source = concat!(
        "requirementDiagram\n",
        "requirement req {\nid: R-1\ntext: Login required\nrisk: high\nverifymethod: test\n}\n",
        "element sys {\ntype: service\ndocref: spec.md\n}\n",
        "sys - satisfies -> req\n",
    );
    for (source, primary_projection) in [
        (source.to_string(), "diagrammatic"),
        (
            source.replace("sys - satisfies", "missing - satisfies"),
            "structured_text",
        ),
    ] {
        for (width, presentation) in [
            (4, "strict"),
            (10, "strict"),
            (80, "strict"),
            (80, "adaptive"),
        ] {
            let limit = width.to_string();
            let output = run_with_stdin(
                &[
                    "render",
                    "-",
                    "--format",
                    "unicode",
                    "--ascii-report",
                    "--ascii-color",
                    "plain",
                    "--ascii-max-width",
                    &limit,
                    "--ascii-overflow",
                    "fallback",
                    "--ascii-graph-presentation",
                    presentation,
                ],
                &source,
            );
            assert!(
                output.status.success(),
                "{width}/{presentation}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report = assert_plain_ascii_report(&output.stdout);
            assert_eq!(report["family"], "requirement");
            assert_eq!(report["layout_profile"], "canonical");
            assert_eq!(report["requested_graph_presentation"], presentation);
            assert_eq!(report["compact_attempted"], false);
            let fallback_expected = width == 4 && primary_projection == "diagrammatic";
            assert_eq!(report["fallback_attempted"], fallback_expected);
            assert_eq!(
                report["outcome"],
                if fallback_expected {
                    "fallback"
                } else {
                    "primary"
                }
            );
            assert_eq!(
                report["projection"],
                if fallback_expected {
                    "structured_text"
                } else {
                    primary_projection
                }
            );
            assert_eq!(report["emitted_overflowed"], false);
            assert!(report["emitted_width"].as_u64().unwrap() <= width);
            let text = report["text"].as_str().unwrap();
            let compact: String = text.split_whitespace().collect();
            for expected in [
                "name=\"req\"id=\"R-1\"",
                "text=\"Loginrequired\"risk=\"High\"verifyMethod=\"Test\"",
                "N1[element]name=\"sys\"type=\"service\"docRef=\"spec.md\"",
                "E1[\"satisfies\"]",
            ] {
                assert!(compact.contains(expected), "missing {expected:?}: {report}");
            }
            let endpoints = if primary_projection == "structured_text" {
                "source=[](unresolved)name=\"missing\"target=[R1]name=\"req\""
            } else {
                "source=[N1]name=\"sys\"target=[R1]name=\"req\""
            };
            assert!(
                compact.contains(endpoints),
                "missing {endpoints:?}: {report}"
            );
        }
    }
}

#[test]
fn cli_renders_plain_ascii_output_to_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("diagram.txt");
    let out_arg = out.to_string_lossy().into_owned();

    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--output",
            out_arg.as_str(),
            "-",
        ],
        "flowchart TB\nsubgraph one\nA((Start)) -- go --> B[(DB)]\nend",
    );
    assert!(output.status.success(), "stderr: {:?}", output.stderr);

    let text = fs::read_to_string(out).expect("read ascii output");
    assert!(text.contains("one"));
    assert!(text.contains("Start"));
    assert!(text.contains("go"));
    assert!(text.contains("DB"));
    assert!(text.contains("/"));
}

#[test]
fn ascii_report_is_plain_for_pipe_file_and_host_color_overrides() {
    let source = "flowchart LR\nA[Hello] --> B[World]";
    let piped = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-color",
            "auto",
            "-",
        ],
        source,
    );
    assert!(piped.status.success(), "stderr: {:?}", piped.stderr);
    assert_plain_ascii_report(&piped.stdout);

    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.txt");
    let report_arg = report_path.to_string_lossy().into_owned();
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args([
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--ascii-color",
            "auto",
            "--output",
            report_arg.as_str(),
            "-",
        ])
        .env("CLICOLOR_FORCE", "1")
        .env("COLORTERM", "truecolor")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(source.as_bytes())
        .expect("write stdin");
    let file_output = child.wait_with_output().expect("wait for CLI");
    assert!(
        file_output.status.success(),
        "stderr: {:?}",
        file_output.stderr
    );
    assert_plain_ascii_report(&fs::read(report_path).expect("read report file"));
}

#[test]
fn ascii_report_rejects_explicit_styled_output() {
    for color in ["ansi16", "ansi256", "truecolor", "html"] {
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                "unicode",
                "--ascii-report",
                "--ascii-color",
                color,
                "-",
            ],
            "flowchart LR\nA[Hello] --> B[World]",
        );

        assert!(
            !output.status.success(),
            "{color} report unexpectedly succeeded"
        );
        assert!(
            output.stdout.is_empty(),
            "failed report must emit no payload"
        );
        let report = assert_ascii_error_report(
            &output.stderr,
            "merman.cli.ascii_report.requires_plain",
            "usage",
        );
        assert_eq!(report["error"]["details"]["field"], "ascii_color");
    }
}

#[test]
fn ascii_report_width_failure_is_typed_json_without_partial_stdout() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-max-width",
            "5",
            "--ascii-overflow",
            "error",
            "-",
        ],
        "flowchart LR\nA[Hello] --> B[World]",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "failed report must not publish a partial success artifact"
    );
    let report =
        assert_ascii_error_report(&output.stderr, "merman.ascii.width_overflow", "content");
    assert_eq!(report["error"]["details"]["requested_max_width"], 5);
    assert!(
        report["error"]["details"]["actual_width"]
            .as_u64()
            .is_some_and(|actual| actual > 5)
    );
    assert_eq!(report["error"]["details"]["width_profile"], "unicode");
}

#[cfg(unix)]
#[test]
fn ascii_report_is_plain_on_a_real_terminal_stdout() {
    let (master, slave) = open_pty();
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args([
            "render",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-color",
            "auto",
            "-",
        ])
        .env("COLORTERM", "truecolor")
        .env("TERM", "xterm-256color")
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI with PTY stdout");
    let reader = std::thread::spawn(move || read_pty(master));
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"flowchart LR\nA[Hello] --> B[World]")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for CLI");
    let stdout = reader.join().expect("join PTY reader");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert_plain_ascii_report(&stdout);
}

#[test]
fn auto_color_honors_process_environment_before_rendering() {
    let source = "flowchart LR\nA[Hello] --> B[World]";
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let run = |no_color: &str| {
        let mut child = Command::new(exe)
            .args([
                "render",
                "--format",
                "unicode",
                "--ascii-color",
                "auto",
                "-",
            ])
            .env("CLICOLOR_FORCE", "1")
            .env("TERM", "xterm-256color")
            .env("NO_COLOR", no_color)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn CLI");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(source.as_bytes())
            .expect("write stdin");
        child.wait_with_output().expect("wait for CLI")
    };

    let empty_no_color = run("");
    assert!(
        empty_no_color.status.success(),
        "stderr: {:?}",
        empty_no_color.stderr
    );
    assert!(
        empty_no_color
            .stdout
            .windows(2)
            .any(|bytes| bytes == b"\x1b["),
        "an empty NO_COLOR value must not disable forced color"
    );

    let set_no_color = run("1");
    assert!(
        set_no_color.status.success(),
        "stderr: {:?}",
        set_no_color.stderr
    );
    assert!(
        !set_no_color
            .stdout
            .windows(2)
            .any(|bytes| bytes == b"\x1b["),
        "a non-empty NO_COLOR value must disable color"
    );
}

#[test]
fn ascii_resource_failure_explains_budget_recovery() {
    let output =
        run_ascii_with_resource_limit("max_ascii_layout_work_units", 1, "flowchart LR\nA --> B");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    for hint in [
        "--resource-limit max_ascii_layout_work_units=",
        "first rejected charge",
        "--resource-profile unbounded-for-trusted-input",
        "--operation-timeout-ms 30000",
    ] {
        assert!(stderr.contains(hint), "missing {hint:?}:\n{stderr}");
    }
}

#[test]
fn ascii_long_help_explains_resource_choices() {
    let output = Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .args(["render", "--help"])
        .output()
        .expect("render long help");
    assert!(output.status.success(), "{:?}", output.stderr);
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    for example in [
        "--resource-limit max_ascii_layout_work_units=",
        "--resource-profile unbounded-for-trusted-input",
        "--operation-timeout-ms 30000",
    ] {
        assert!(help.contains(example), "missing {example:?}:\n{help}");
    }
    assert!(help.contains("all policy budgets"), "{help}");
}

#[test]
fn ascii_report_resource_failure_preserves_stable_limit_details() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--resource-limit",
            "max_ascii_output_bytes=1",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let report =
        assert_ascii_error_report(&output.stderr, "merman.ascii.resource_limit", "content");
    let resource = &report["error"]["details"]["resource_limit"];
    assert_eq!(resource["id"], "max_ascii_output_bytes");
    assert_eq!(resource["phase"], "ascii_output");
    assert_eq!(resource["maximum"], 1);
    assert_eq!(resource["cause"], "ceiling");
    assert_eq!(resource["profile"], "trusted-native");
    assert!(resource["actual"].as_u64().is_some_and(|actual| actual > 1));
    assert!(
        report["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("--resource-limit max_ascii_output_bytes=")),
        "{report}"
    );
}

#[test]
fn ascii_report_source_limit_failure_preserves_stable_limit_details() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--resource-limit",
            "max_source_bytes=5",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let report =
        assert_ascii_error_report(&output.stderr, "merman.input.resource_limit", "content");
    let resource = &report["error"]["details"]["resource_limit"];
    assert_eq!(resource["id"], "max_source_bytes");
    assert_eq!(resource["phase"], "source");
    assert_eq!(resource["maximum"], 5);
    assert_eq!(resource["cause"], "ceiling");
    assert_eq!(resource["profile"], "trusted-native");
    assert!(resource["actual"].as_u64().is_some_and(|actual| actual > 5));
}

#[test]
fn ascii_report_model_limit_failure_preserves_stable_limit_details() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--ascii-report",
            "--resource-limit",
            "max_model_items=1",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let report =
        assert_ascii_error_report(&output.stderr, "merman.input.resource_limit", "content");
    let resource = &report["error"]["details"]["resource_limit"];
    assert_eq!(resource["id"], "max_model_items");
    assert_eq!(resource["phase"], "layout_model");
    assert_eq!(resource["maximum"], 1);
    assert_eq!(resource["cause"], "ceiling");
    assert_eq!(resource["profile"], "trusted-native");
    assert!(resource["actual"].as_u64().is_some_and(|actual| actual > 1));
}

#[cfg(unix)]
#[test]
fn auto_color_observes_a_real_terminal_stdout() {
    let (master, slave) = open_pty();
    let exe = assert_cmd::cargo_bin!("merman-cli");
    let mut child = Command::new(exe)
        .args([
            "render",
            "--format",
            "unicode",
            "--ascii-color",
            "auto",
            "-",
        ])
        .env("TERM", "xterm-256color")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI with PTY stdout");
    let reader = std::thread::spawn(move || read_pty(master));
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"flowchart LR\nA[Hello] --> B[World]")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for CLI");
    let stdout = reader.join().expect("join PTY reader");
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(
        stdout.windows(2).any(|bytes| bytes == b"\x1b["),
        "terminal stdout should enable ANSI color: {stdout:?}"
    );
}

#[test]
fn cli_applies_the_selected_resource_profile_to_text_output() {
    let input = format!(
        "%% {}\nflowchart TD\nA[Hello] --> B[World]\n",
        "x".repeat(1024 * 1024)
    );

    let constrained = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-profile",
            "constrained",
            "-",
        ],
        &input,
    );
    assert!(!constrained.status.success());
    let stderr = String::from_utf8(constrained.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("max_source_bytes"), "{stderr}");

    let trusted = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-profile",
            "trusted-native",
            "-",
        ],
        &input,
    );
    assert!(trusted.status.success(), "stderr: {:?}", trusted.stderr);
}

#[test]
fn cli_reports_a_representative_ascii_resource_boundary_with_stable_typed_context() {
    let case = ascii_resource_boundary_contract()
        .transport_representatives
        .cli_trusted_native;
    let exact = case.exact;

    let exact_output = run_ascii_with_resource_limit(&case.id, exact, &case.source);
    assert!(
        exact_output.status.success(),
        "exact {} boundary failed: {}",
        case.id,
        String::from_utf8_lossy(&exact_output.stderr),
    );
    assert!(
        !exact_output.stdout.is_empty(),
        "{} produced no output",
        case.id
    );

    let below_output = run_ascii_with_resource_limit(&case.id, exact - 1, &case.source);
    assert!(
        !below_output.status.success(),
        "one-below {} boundary unexpectedly succeeded",
        case.id
    );
    let stderr = String::from_utf8(below_output.stderr).expect("stderr should be utf8");
    let expected_prefix = format!(
        "ASCII resource limit `{}` exceeded during `{}`: actual {}",
        case.id, case.phase, exact
    );
    assert!(stderr.contains(&expected_prefix), "{stderr}");
    assert!(
        stderr.contains(&format!("maximum {} (profile `trusted-native`)", exact - 1)),
        "{stderr}"
    );
}

#[test]
fn cli_rejects_duplicate_generic_and_legacy_ascii_grid_overrides() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-limit",
            "max_ascii_grid_cells=100",
            "--ascii-max-grid-cells",
            "100",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("specified by both"), "{stderr}");
}

#[cfg(feature = "diagram-packet")]
#[test]
fn packet_reports_distinguish_spatial_text_and_viewport_fallback_output() {
    for (bits_per_row, max_width, projection, outcome) in [
        ("16", "80", "diagrammatic", "primary"),
        ("65", "80", "structured_text", "primary"),
        ("32", "32", "structured_text", "fallback"),
    ] {
        let source = format!(
            "---\nconfig:\n  packet:\n    bitsPerRow: {bits_per_row}\n    bitOrder: descending\n---\npacket\n0-7: \"Header\"\n8-15: \"Body\""
        );
        let output = run_with_stdin(
            &[
                "render",
                "--format",
                "ascii",
                "--ascii-report",
                "--ascii-max-width",
                max_width,
                "--ascii-overflow",
                "fallback",
                "-",
            ],
            &source,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = assert_plain_ascii_report(&output.stdout);
        assert_eq!(report["family"], "packet");
        assert_eq!(report["projection"], projection);
        assert_eq!(report["outcome"], outcome);
        assert!(report["text"].as_str().unwrap().contains("Header"));
        if projection == "diagrammatic" {
            assert!(report["text"].as_str().unwrap().contains("7..0"));
        }
    }
}

#[cfg(feature = "svg")]
#[test]
fn text_admission_ignores_svg_layout_limits_in_combined_builds() {
    let output = run_with_stdin(
        &[
            "render",
            "--format",
            "ascii",
            "--resource-profile",
            "constrained",
            "--resource-limit",
            "max_layout_work_units=10000000",
            "-",
        ],
        "flowchart LR\nA --> B",
    );

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
}

#[test]
fn auto_layout_report_selects_compact_without_changing_overflow_policy() {
    let source = "flowchart LR\nA --> B --> C --> D";
    let render = |profile: &str, max_width: &str, overflow: &str| {
        run_with_stdin(
            &[
                "render",
                "-",
                "--format",
                "unicode",
                "--ascii-report",
                "--ascii-layout-profile",
                profile,
                "--ascii-max-width",
                max_width,
                "--ascii-overflow",
                overflow,
            ],
            source,
        )
    };
    let compact = render("compact", "1000", "allow");
    assert!(compact.status.success(), "stderr: {:?}", compact.stderr);
    let compact = assert_plain_ascii_report(&compact.stdout);
    let width = compact["primary_width"]
        .as_u64()
        .expect("primary extent width")
        .to_string();
    let automatic = render("auto", &width, "error");
    assert!(automatic.status.success(), "stderr: {:?}", automatic.stderr);
    let report = assert_plain_ascii_report(&automatic.stdout);
    assert_eq!(report["requested_layout_profile"], "auto");
    assert_eq!(report["layout_profile"], "compact");
    assert_eq!(report["compact_attempted"], true);
    assert_eq!(report["primary_overflowed"], false);
    assert_eq!(report["emitted_overflowed"], false);
    assert_eq!(report["requested_graph_presentation"], "strict");
    assert_eq!(report["text"], compact["text"]);

    let wide = render("auto", "1000", "error");
    assert!(wide.status.success(), "stderr: {:?}", wide.stderr);
    let report = assert_plain_ascii_report(&wide.stdout);
    assert_eq!(report["layout_profile"], "canonical");
    assert_eq!(report["compact_attempted"], false);

    let overflow = render("auto", "1", "error");
    assert!(!overflow.status.success());
    assert!(overflow.stdout.is_empty());
}

#[test]
fn auto_layout_requires_an_explicit_width() {
    let output = run_with_stdin(
        &[
            "render",
            "-",
            "--format",
            "unicode",
            "--ascii-layout-profile",
            "auto",
        ],
        "flowchart LR\nA --> B",
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("width"));
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn fixed_flowchart_routing_space_is_applied_to_cli_reports() {
    let output = run_with_stdin(
        &[
            "render",
            "-",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-node-padding-y",
            "0",
            "--ascii-graph-padding-y",
            "2",
            "--ascii-graph-routing-space",
            "fixed",
        ],
        "flowchart TD\nA[Load] -->|submit| B[Save]",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = assert_plain_ascii_report(&output.stdout);
    assert_eq!(report["primary_height"], 8);
    assert_eq!(report["emitted_height"], 8);
    assert!(report["text"].as_str().unwrap().contains("submit"));
}

#[test]
fn adaptive_graph_report_distinguishes_missing_geometry_from_width_overflow() {
    let source = include_str!(
        "../../merman-ascii/tests/testdata/local-semantic/flowchart/scene_planning/dense-minimized.mmd"
    );
    let output = run_with_stdin(
        &[
            "render",
            "-",
            "--format",
            "unicode",
            "--ascii-report",
            "--ascii-graph-presentation",
            "adaptive",
            "--ascii-max-width",
            "80",
            "--ascii-overflow",
            "fallback",
            "--output",
            "-",
        ],
        source,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = assert_plain_ascii_report(&output.stdout);
    assert_eq!(report["requested_graph_presentation"], "adaptive");
    assert_eq!(report["emitted_overflowed"], false);
    assert!(report["emitted_width"].as_u64().unwrap() <= 80);
    if report["primary_width"].is_null() {
        assert!(report["primary_height"].is_null());
        assert!(report["layout_profile"].is_null());
        assert!(report["primary_overflowed"].is_null());
        assert_eq!(report["projection"], "structured_text");
        assert!(matches!(
            report["fallback_reason"].as_str(),
            Some("routing_search_exhausted" | "port_capacity_exceeded")
        ));
    }
}

#[test]
fn adaptive_graph_style_admission_is_reported_before_route_work() {
    let output = run_with_stdin(
        &[
            "render",
            "-",
            "--format",
            "unicode",
            "--ascii-graph-presentation",
            "adaptive",
            "--ascii-color",
            "ansi16",
            "--resource-limit",
            "max_ascii_layout_work_units=1",
        ],
        "flowchart LR\nA --> B",
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("graph_presentation") || diagnostic.contains("adaptive"),
        "{diagnostic}"
    );
    assert!(
        !diagnostic.contains("exceeded"),
        "style admission precedes route admission: {diagnostic}"
    );
}

#[test]
fn node_vertical_padding_changes_rendered_report_without_collapsing_rank_gaps() {
    let source = "flowchart TD\nA[Write code] --> B[Open PR] --> C[Merge]";
    for format in ["ascii", "unicode"] {
        let default = run_with_stdin(
            &["render", "-", "--format", format, "--ascii-report"],
            source,
        );
        assert!(default.status.success(), "stderr: {:?}", default.stderr);
        let default = assert_plain_ascii_report(&default.stdout);
        assert_eq!(default["emitted_height"], 13);

        let spacious = run_with_stdin(
            &[
                "render",
                "-",
                "--format",
                format,
                "--ascii-report",
                "--ascii-node-padding",
                "1",
                "--ascii-graph-padding-y",
                "5",
            ],
            source,
        );
        assert!(spacious.status.success(), "stderr: {:?}", spacious.stderr);
        let spacious = assert_plain_ascii_report(&spacious.stdout);
        assert_eq!(spacious["emitted_height"], 25);

        let compact_nodes = run_with_stdin(
            &[
                "render",
                "-",
                "--format",
                format,
                "--ascii-report",
                "--ascii-node-padding",
                "1",
                "--ascii-graph-padding-y",
                "5",
                "--ascii-node-padding-y",
                "0",
            ],
            source,
        );
        assert!(
            compact_nodes.status.success(),
            "stderr: {:?}",
            compact_nodes.stderr
        );
        let compact_nodes = assert_plain_ascii_report(&compact_nodes.stdout);
        assert_eq!(compact_nodes["emitted_height"], 19);
        assert_eq!(compact_nodes["emitted_width"], default["emitted_width"]);
        assert_eq!(compact_nodes["projection"], "diagrammatic");
        let rendered = compact_nodes["text"].as_str().expect("rendered text");
        for label in ["Write code", "Open PR", "Merge"] {
            assert_eq!(rendered.matches(label).count(), 1);
        }
        assert_eq!(
            rendered
                .matches(if format == "ascii" { "v" } else { "▼" })
                .count(),
            2
        );
    }
}
