#![cfg(feature = "ascii")]

use merman::ascii::{
    AsciiError, AsciiOutputOutcome, AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId,
    AsciiResourcePolicy, AsciiViewportPolicy, OverflowPolicy,
};
use merman::model::ParsedSemanticDiagram;
use merman::model::resources::ResourceProfile;
use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};

fn render_model_for(source: &str) -> ParsedSemanticDiagram {
    merman::SemanticParser::new()
        .parse_diagram_sync(source)
        .unwrap()
}

fn render_typed_model(
    model: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> Result<String, AsciiError> {
    let context = merman::model::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic operation context");
    AsciiRenderer::new(options)?.render_parsed(model, &OperationControl::new(), &context, resources)
}

fn deeply_nested_flowchart(depth: usize) -> String {
    let mut lines = vec!["flowchart TB".to_string()];
    for i in 0..depth {
        lines.push(format!("subgraph n{i}"));
    }
    lines.push("A".to_string());
    for _ in 0..depth {
        lines.push("end".to_string());
    }
    lines.join("\n")
}

#[cfg(feature = "diagram-git-graph")]
#[test]
fn gitgraph_facade_preserves_actual_projection_and_keyed_parent_records() {
    let body = "gitGraph LR:\ncommit id:\"A\"\nbranch feature\ncommit id:\"F\"\ncheckout main\ncommit id:\"B\"\nmerge feature id:\"M\" tag:\"v1\"\n";
    for (parallel, width) in [(false, 80), (false, 20), (true, 80)] {
        let source = if parallel {
            format!("---\nconfig:\n  gitGraph:\n    parallelCommits: true\n---\n{body}")
        } else {
            body.to_string()
        };
        let output = Renderer::new()
            .render(RenderRequest::ascii(
                &source,
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::ascii(),
                    viewport: AsciiViewportPolicy::with_max_width(width)
                        .overflow(OverflowPolicy::Fallback),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(output)) = output else {
            panic!("missing ASCII report")
        };
        assert_eq!(
            output.projection.as_str(),
            if !parallel && width == 80 {
                "diagrammatic"
            } else {
                "structured_text"
            }
        );
        assert_eq!(output.fallback.attempted, width == 20);
        assert!(output.emitted_extent.width <= width);
        let records: String = output
            .text
            .split_once("GitGraph records")
            .unwrap()
            .1
            .split_whitespace()
            .collect();
        assert_eq!(records.matches("[commit]").count(), 4);
        assert!(records.contains("parents=[\"B\",\"F\"]"));
    }
}

#[test]
fn renderer_renders_ascii_flowchart_from_mermaid_text() {
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "flowchart LR\nA --> B",
            OperationControl::new(),
            AsciiRequest {
                options: AsciiRenderOptions::ascii(),
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(rendered)) = output else {
        panic!("diagram not detected");
    };

    assert_eq!(
        rendered.text,
        "+---+     +---+\n| A |---->| B |\n+---+     +---+\n"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn facade_fixed_routing_space_respects_vertical_rank_gaps_in_both_directions() {
    use merman::ascii::AsciiGraphRoutingSpace;

    for direction in ["TD", "BT"] {
        let source = format!("flowchart {direction}\nA[Load] -->|submit| B[Save]");
        for gap in [2, 4] {
            let output = Renderer::new()
                .render(RenderRequest::ascii(
                    &source,
                    OperationControl::new(),
                    AsciiRequest {
                        options: AsciiRenderOptions::unicode()
                            .with_node_padding_y(0)
                            .with_graph_padding_y(gap)
                            .with_graph_routing_space(AsciiGraphRoutingSpace::Fixed),
                        ..Default::default()
                    },
                ))
                .unwrap();
            let RenderOutput::Ascii(Some(rendered)) = output else {
                panic!("expected a fixed Flowchart diagram");
            };
            assert_eq!(rendered.emitted_extent.height, 6 + gap, "{}", rendered.text);
            let from = rendered.text.find("Load").unwrap();
            let edge = rendered.text.find("submit").unwrap();
            let to = rendered.text.find("Save").unwrap();
            assert!(if direction == "TD" {
                from < edge && edge < to && rendered.text.contains('▼')
            } else {
                to < edge && edge < from && rendered.text.contains('▲')
            });
        }
    }
}

#[test]
fn facade_report_exposes_bounded_fallback_without_remeasuring_text() {
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "flowchart LR\nA[Alpha] --> B[Beta]",
            OperationControl::new(),
            AsciiRequest {
                options: AsciiRenderOptions::ascii(),
                viewport: AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Fallback),
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(report)) = output else {
        panic!("diagram not detected");
    };
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert!(report.emitted_extent.width <= 5);
    let flattened: String = report
        .text
        .lines()
        .map(|line| line.strip_prefix("  ").unwrap_or(line))
        .collect();
    assert!(flattened.contains("Alpha"));
    assert!(flattened.contains("Beta"));
}

#[test]
fn facade_ascii_keeps_source_label_entities_in_primary_and_narrow_fallback() {
    let source = "flowchart LR\nA[\"`**request** &lt;T&gt;`\"] -->|returns &lt;T&gt;| B[Done]";
    for width in [120, 12] {
        let output = Renderer::new()
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::unicode(),
                    viewport: AsciiViewportPolicy::with_max_width(width)
                        .overflow(OverflowPolicy::Fallback),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(report)) = output else {
            panic!("expected ASCII output");
        };
        assert!(report.emitted_extent.width <= width);
        if width == 120 {
            assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
            assert!(report.text.contains("request <T>"), "{}", report.text);
            assert!(report.text.contains("returns <T>"), "{}", report.text);
            assert!(!report.text.contains("**request**"), "{}", report.text);
        } else {
            assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
            let flattened: String = report
                .text
                .lines()
                .map(|line| line.strip_prefix("  ").unwrap_or(line))
                .collect();
            for expected in ["labelSources", "request", "returns &lt;T&gt;"] {
                assert!(
                    flattened.contains(expected),
                    "missing {expected:?}: {}",
                    report.text
                );
            }
        }
    }
}

#[cfg(all(
    feature = "presentation",
    feature = "diagram-agentflow",
    feature = "diagram-block",
    feature = "diagram-usecase"
))]
#[test]
fn family_ascii_uses_the_same_semantics_from_canonical_and_presentation_artifacts() {
    let sources = [
        (
            "agentflow-beta\nA[Request] --> B[Result]",
            AsciiOutputOutcome::Fallback,
        ),
        (
            "agentflow-beta\nA[Request]@{instruction: inspect, params: {retries: 3}} --> B[Result]",
            AsciiOutputOutcome::Primary,
        ),
        (
            "usecase-beta\nactor Customer(\"Customer\")\nCheckout(\"Place order\")\nCustomer --> Checkout",
            AsciiOutputOutcome::Fallback,
        ),
        (
            "block-beta\ncolumns 3\nA[\"Request\"]:2 space\nB[\"Result\"]:3\nA --> B",
            AsciiOutputOutcome::Fallback,
        ),
    ];
    for (source, narrow_outcome) in sources {
        for width in [10, 80, 100, 120] {
            let request = AsciiRequest {
                options: AsciiRenderOptions::unicode(),
                viewport: AsciiViewportPolicy::with_max_width(width)
                    .overflow(OverflowPolicy::Fallback),
                ..Default::default()
            };
            let renderer = Renderer::new();
            let semantic = renderer
                .parse_semantic(source, OperationControl::new())
                .unwrap()
                .unwrap();
            let presentation = renderer
                .prepare_presentation(source, OperationControl::new())
                .unwrap()
                .unwrap();
            let RenderOutput::Ascii(Some(canonical)) = semantic
                .render(merman::RenderTarget::Ascii(request.clone()))
                .unwrap()
            else {
                panic!("expected canonical ASCII output");
            };
            let RenderOutput::Ascii(Some(full)) = presentation
                .render(merman::RenderTarget::Ascii(request))
                .unwrap()
            else {
                panic!("expected presentation ASCII output");
            };
            assert_eq!(canonical.text, full.text, "width {width}: {source}");
            assert_eq!(canonical.outcome, full.outcome);
            assert_eq!(canonical.projection, full.projection);
            assert_eq!(canonical.emitted_extent, full.emitted_extent);
            assert!(full.emitted_extent.width <= width);
            assert_eq!(
                full.outcome,
                if width == 10 {
                    narrow_outcome
                } else {
                    AsciiOutputOutcome::Primary
                }
            );
        }
    }
}

#[cfg(feature = "diagram-packet")]
#[test]
fn packet_facade_retains_source_geometry_policy_and_actual_projection() {
    for (width, projection) in [("16", "diagrammatic"), ("65", "structured_text")] {
        let source = format!(
            "---\nconfig:\n  packet:\n    bitsPerRow: {width}\n    bitOrder: descending\n---\npacket\n0-7: \"Header\"\n8-15: \"Body\""
        );
        let output = Renderer::new()
            .render(RenderRequest::ascii(
                &source,
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::ascii(),
                    viewport: AsciiViewportPolicy::with_max_width(80)
                        .overflow(OverflowPolicy::Error),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(output)) = output else {
            panic!("Packet not detected")
        };
        assert_eq!(output.projection.as_str(), projection);
        assert!(output.text.contains("Header"));
        if width == "16" {
            assert!(output.text.contains("7..0"), "{}", output.text);
            assert!(output.text.contains("15..8"));
            assert!(output.text.contains("|     Body      |    Header     |"));
        }
    }
}

#[cfg(all(feature = "presentation", feature = "diagram-gantt"))]
#[test]
fn gantt_ascii_matches_canonical_and_presentation_schedules_records_and_reports() {
    let body = concat!(
        "gantt\ntitle Release plan\naccTitle: Delivery plan\n",
        "dateFormat YYYY-MM-DD\naxisFormat %d\ntodayMarker off\n",
        "section Delivery work\nBuild phase: done, build, 2026-01-01, 2d\n",
        "section Review work\nReview phase: crit, active, review, after build, 1d\n",
        "section Delivery work\nShip phase: milestone, ship, after review, 0d\n",
    );
    let renderer = Renderer::new();
    for compact_mode in [false, true] {
        let source = if compact_mode {
            format!("---\nconfig:\n  gantt:\n    displayMode: compact\n---\n{body}")
        } else {
            body.to_string()
        };
        let mut expected_records = None;
        for width in [59, 60] {
            let semantic = renderer
                .parse_semantic(&source, OperationControl::new())
                .unwrap()
                .unwrap();
            let merman::model::SemanticDiagramModel::Gantt(model) = semantic.model() else {
                panic!("expected canonical Gantt model");
            };
            let scheduled_fields: Vec<_> = model
                .tasks
                .iter()
                .map(|task| {
                    vec![
                        format!("startTime:{}", task.start_ms),
                        format!("endTime:{}", task.end_ms),
                        format!(
                            "renderEndTime:{}",
                            serde_json::to_string(&task.render_end_ms).unwrap()
                        ),
                        format!(
                            "startConstraint:{}",
                            serde_json::to_string(&task.start_constraint).unwrap()
                        ),
                        format!(
                            "endConstraint:{}",
                            serde_json::to_string(&task.end_constraint).unwrap()
                        ),
                        format!("raw:{}", serde_json::to_string(&task.raw).unwrap()),
                    ]
                })
                .collect();
            assert_eq!(scheduled_fields.len(), 3);
            let presentation = renderer
                .prepare_presentation(&source, OperationControl::new())
                .unwrap()
                .unwrap();
            let request = AsciiRequest {
                options: AsciiRenderOptions::unicode(),
                viewport: AsciiViewportPolicy::with_max_width(width)
                    .overflow(OverflowPolicy::Fallback),
                ..Default::default()
            };
            let RenderOutput::Ascii(Some(canonical)) = semantic
                .render(merman::RenderTarget::Ascii(request.clone()))
                .unwrap()
            else {
                panic!("expected canonical Gantt ASCII output");
            };
            let RenderOutput::Ascii(Some(full)) = presentation
                .render(merman::RenderTarget::Ascii(request))
                .unwrap()
            else {
                panic!("expected presentation Gantt ASCII output");
            };
            assert_eq!(
                canonical.text, full.text,
                "compact {compact_mode}, width {width}"
            );
            assert_eq!(canonical.report(), full.report());
            let fallback = !compact_mode && width < 60;
            assert_eq!(full.family, "gantt");
            assert_eq!(full.encoding.as_str(), "plain");
            assert_eq!(full.requested_graph_presentation.as_str(), "strict");
            assert!(!full.compact_attempted);
            assert_eq!(full.fallback.attempted, fallback);
            assert_eq!(full.primary_overflowed, Some(fallback));
            assert_eq!(
                full.outcome,
                if fallback {
                    AsciiOutputOutcome::Fallback
                } else {
                    AsciiOutputOutcome::Primary
                }
            );
            assert_eq!(
                full.projection.as_str(),
                if compact_mode || fallback {
                    "structured_text"
                } else {
                    "diagrammatic"
                }
            );
            assert!(!full.emitted_overflowed);
            assert!(full.emitted_extent.width <= width);
            assert_eq!(
                full.text.contains("Gantt timeline (elapsed time)"),
                !compact_mode && !fallback
            );

            // Remove only presentation continuation indents, preserving authored spaces.
            let mut records: Vec<String> = Vec::new();
            for line in full.text.lines() {
                if let Some(continuation) = line.strip_prefix("  ")
                    && let Some(record) = records.last_mut()
                {
                    record.push_str(continuation);
                } else {
                    records.push(line.to_string());
                }
            }
            let records: Vec<_> = records
                .into_iter()
                .skip_while(|record| !record.starts_with("Keys: T1..Tn"))
                .collect();
            assert!(
                !records.is_empty(),
                "missing complete records: {}",
                full.text
            );
            let task_keys: Vec<_> = records
                .iter()
                .filter(|record| record.contains(" [task] "))
                .map(|record| record.split_once(' ').unwrap().0)
                .collect();
            assert_eq!(task_keys, ["T1", "T2", "T3"]);
            for (index, (id, label, section, start, end)) in [
                (
                    "build",
                    "Build phase",
                    "Delivery work",
                    "2026-01-01",
                    "2026-01-03",
                ),
                (
                    "review",
                    "Review phase",
                    "Review work",
                    "2026-01-03",
                    "2026-01-04",
                ),
                (
                    "ship",
                    "Ship phase",
                    "Delivery work",
                    "2026-01-04",
                    "2026-01-04",
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let key = format!("T{}", index + 1);
                assert!(records.contains(&format!(
                    "{key} [task] id=\"{id}\" label=\"{label}\" section=\"{section}\" sectionIndex={index} order={index}"
                )));
                assert!(records.iter().any(|record| {
                    record.starts_with(&format!("{key} interval (UTC): {start} -> {end}"))
                }));
                let details = records
                    .iter()
                    .find(|record| record.starts_with(&format!("{key} details: {{")))
                    .expect("each task retains its complete details");
                for expected in &scheduled_fields[index] {
                    assert!(
                        details.contains(expected),
                        "missing {expected:?}: {details}"
                    );
                }
                for field in [
                    "type:",
                    "classes:",
                    "active:",
                    "done:",
                    "crit:",
                    "milestone:",
                    "vert:",
                    "prevTaskId:",
                    "processed:true",
                    "manualEndTime:",
                ] {
                    assert!(details.contains(field), "missing {field:?}: {details}");
                }
            }
            let metadata = records
                .iter()
                .find(|record| record.starts_with("gantt details: {"))
                .expect("Gantt retains complete metadata");
            for expected in [
                "title:\"Release plan\"",
                "accTitle:\"Delivery plan\"",
                "sections:[\"Delivery work\",\"Review work\",\"Delivery work\"]",
                "dateFormat:\"YYYY-MM-DD\"",
                "axisFormat:\"%d\"",
                "todayMarker:\"off\"",
                "sourceConfig:",
            ] {
                assert!(
                    metadata.contains(expected),
                    "missing {expected:?}: {metadata}"
                );
            }
            if let Some(expected) = &expected_records {
                assert_eq!(
                    &records, expected,
                    "record reflow must preserve all content"
                );
            } else {
                expected_records = Some(records);
            }
        }
    }
}

#[cfg(feature = "diagram-gantt")]
#[test]
fn gantt_ascii_uses_structured_text_primary_for_empty_zero_span_and_vertical_sources() {
    for source in [
        "gantt\nsection Empty work\n",
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery work\nRelease marker: milestone, marker, 2026-01-01, 0d\n",
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery work\nReview marker: vert, marker, 2026-01-01, 1d\n",
    ] {
        let output = Renderer::new()
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::unicode(),
                    viewport: AsciiViewportPolicy::with_max_width(10)
                        .overflow(OverflowPolicy::Fallback),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(output)) = output else {
            panic!("expected Gantt ASCII output");
        };
        assert_eq!(output.projection.as_str(), "structured_text");
        assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
        assert!(!output.fallback.attempted);
        assert_eq!(output.primary_overflowed, Some(false));
        assert!(!output.emitted_overflowed);
        assert!(output.emitted_extent.width <= 10);
        let compact: String = output.text.split_whitespace().collect();
        assert!(compact.contains("ganttdetails:{"));
        if source.contains("marker,") {
            assert!(compact.contains("T1[task]id=\"marker\""));
            assert!(compact.contains("T1details:{"));
            assert!(compact.contains("T1interval(UTC):2026-01-01->"));
        }
    }
}

#[cfg(all(feature = "presentation", feature = "diagram-pie"))]
#[test]
fn pie_ascii_matches_canonical_and_presentation_records_and_projection() {
    for (dogs, cats, primary_projection) in [(386, 85, "diagrammatic"), (0, 0, "structured_text")] {
        let source = format!("pie showData\ntitle Pets\n\"Dogs\": {dogs}\n\"Cats\": {cats}\n");
        let renderer = Renderer::new();
        for width in [10, 20, 80] {
            let request = AsciiRequest {
                options: AsciiRenderOptions::unicode(),
                viewport: AsciiViewportPolicy::with_max_width(width)
                    .overflow(OverflowPolicy::Fallback),
                ..Default::default()
            };
            let semantic = renderer
                .parse_semantic(&source, OperationControl::new())
                .unwrap()
                .unwrap();
            let presentation = renderer
                .prepare_presentation(&source, OperationControl::new())
                .unwrap()
                .unwrap();
            let RenderOutput::Ascii(Some(canonical)) = semantic
                .render(merman::RenderTarget::Ascii(request.clone()))
                .unwrap()
            else {
                panic!("expected canonical Pie ASCII output");
            };
            let RenderOutput::Ascii(Some(full)) = presentation
                .render(merman::RenderTarget::Ascii(request))
                .unwrap()
            else {
                panic!("expected presentation Pie ASCII output");
            };
            assert_eq!(canonical.text, full.text, "width {width}: {source}");
            assert_eq!(canonical.report(), full.report());
            assert_eq!(full.family, "pie");
            assert_eq!(full.encoding.as_str(), "plain");
            assert_eq!(full.requested_graph_presentation.as_str(), "strict");
            assert!(!full.compact_attempted);
            assert_eq!(full.fallback.attempted, width < 80);
            assert_eq!(full.primary_overflowed, Some(width < 80));
            assert_eq!(
                full.outcome,
                if width < 80 {
                    AsciiOutputOutcome::Fallback
                } else {
                    AsciiOutputOutcome::Primary
                }
            );
            assert_eq!(
                full.projection.as_str(),
                if width < 80 {
                    "structured_text"
                } else {
                    primary_projection
                }
            );
            assert!(!full.emitted_overflowed);
            assert!(full.emitted_extent.width <= width);
            let compact: String = full.text.split_whitespace().collect();
            for expected in [
                format!("P1label=\"Dogs\"value={dogs}"),
                format!("P2label=\"Cats\"value={cats}"),
                "Pets".to_string(),
            ] {
                assert!(
                    compact.contains(&expected),
                    "missing {expected:?}: {}",
                    full.text
                );
            }
            assert!(compact.find("P1label=").unwrap() < compact.find("P2label=").unwrap());
        }
    }
}

#[cfg(all(feature = "presentation", feature = "diagram-c4"))]
#[test]
fn c4_ascii_matches_canonical_and_presentation_records_and_projection() {
    let source = concat!(
        "C4Dynamic\n",
        "Person(user, \"Customer\")\n",
        "Boundary(zone, \"Services\") {\n",
        "Container(api, \"API\", \"Rust\", \"Serves requests\")\n}\n",
        "Rel_Back(api, user, \"Replies\", \"HTTPS\")\n",
    );
    let renderer = Renderer::new();
    for width in [10, 80] {
        let semantic = renderer
            .parse_semantic(source, OperationControl::new())
            .unwrap()
            .unwrap();
        let presentation = renderer
            .prepare_presentation(source, OperationControl::new())
            .unwrap()
            .unwrap();
        let request = AsciiRequest {
            options: AsciiRenderOptions::unicode(),
            viewport: AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
            ..Default::default()
        };
        let RenderOutput::Ascii(Some(canonical)) = semantic
            .render(merman::RenderTarget::Ascii(request.clone()))
            .unwrap()
        else {
            panic!("expected canonical C4 ASCII output");
        };
        let RenderOutput::Ascii(Some(full)) = presentation
            .render(merman::RenderTarget::Ascii(request))
            .unwrap()
        else {
            panic!("expected presentation C4 ASCII output");
        };
        assert_eq!(canonical.text, full.text, "width {width}");
        assert_eq!(canonical.report(), full.report());
        assert_eq!(full.family, "c4");
        assert_eq!(full.encoding.as_str(), "plain");
        assert_eq!(full.requested_graph_presentation.as_str(), "strict");
        assert!(!full.compact_attempted);
        assert_eq!(full.fallback.attempted, width == 10);
        assert_eq!(full.primary_overflowed, Some(width == 10));
        assert_eq!(
            full.projection.as_str(),
            if width == 10 {
                "structured_text"
            } else {
                "diagrammatic"
            }
        );
        assert!(!full.emitted_overflowed);
        assert!(full.emitted_extent.width <= width);
        let compact: String = full.text.split_whitespace().collect();
        for expected in [
            "E1[relationship]type=\"rel_b\"source=[S2]alias=\"api\"target=[S1]alias=\"user\"heads=sourcerender-number=1",
            "S2techn={text:\"Rust\"}",
            "parentBoundary=[B2]alias=\"zone\"",
            "Servesrequests",
            "Replies",
            "HTTPS",
        ] {
            assert!(
                compact.contains(expected),
                "missing {expected:?}: {}",
                full.text
            );
        }
    }
}

#[cfg(all(feature = "presentation", feature = "diagram-architecture"))]
#[test]
fn architecture_ascii_matches_canonical_and_presentation_records_and_projection() {
    let source = concat!(
        "architecture-beta\n",
        "service api(server)[API]\n",
        "service db(database)[Database]\n",
        "api:R --> L:db\n",
    );
    let renderer = Renderer::new();
    for width in [None, Some(10)] {
        let semantic = renderer
            .parse_semantic(source, OperationControl::new())
            .unwrap()
            .unwrap();
        let presentation = renderer
            .prepare_presentation(source, OperationControl::new())
            .unwrap()
            .unwrap();
        let request = AsciiRequest {
            options: AsciiRenderOptions::unicode(),
            viewport: width.map_or_else(AsciiViewportPolicy::unrestricted, |width| {
                AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback)
            }),
            ..Default::default()
        };
        let RenderOutput::Ascii(Some(canonical)) = semantic
            .render(merman::RenderTarget::Ascii(request.clone()))
            .unwrap()
        else {
            panic!("expected canonical Architecture ASCII output");
        };
        let RenderOutput::Ascii(Some(full)) = presentation
            .render(merman::RenderTarget::Ascii(request))
            .unwrap()
        else {
            panic!("expected presentation Architecture ASCII output");
        };
        assert_eq!(canonical.text, full.text, "width {width:?}");
        assert_eq!(canonical.report(), full.report());
        assert_eq!(full.family, "architecture");
        assert_eq!(full.encoding.as_str(), "plain");
        assert_eq!(full.requested_graph_presentation.as_str(), "strict");
        assert!(!full.compact_attempted);
        assert_eq!(full.fallback.attempted, width.is_some());
        assert_eq!(full.primary_overflowed, Some(width.is_some()));
        assert_eq!(
            full.outcome,
            if width.is_some() {
                AsciiOutputOutcome::Fallback
            } else {
                AsciiOutputOutcome::Primary
            }
        );
        assert_eq!(
            full.projection.as_str(),
            if width.is_some() {
                "structured_text"
            } else {
                "diagrammatic"
            }
        );
        assert!(!full.emitted_overflowed);
        if let Some(width) = width {
            assert!(full.emitted_extent.width <= width);
        }
        let compact: String = full.text.split_whitespace().collect();
        for expected in [
            "N1[service]id=\"api\"title=\"API\"",
            "N2[service]id=\"db\"title=\"Database\"",
            "E1lhs=[N1]id=\"api\"lhsDir=\"R\"lhsInto=null",
            "E1rhs=[N2]id=\"db\"rhsDir=\"L\"rhsInto=true",
        ] {
            assert!(
                compact.contains(expected),
                "missing {expected:?}: {}",
                full.text
            );
        }
    }
}

#[cfg(all(feature = "presentation", feature = "diagram-zenuml"))]
#[test]
fn zenuml_ascii_matches_canonical_and_presentation_records_and_projection() {
    let source = "zenuml\nAlice->Bob: Hello\nBob-->Alice: Reply\n";
    let renderer = Renderer::new();
    for width in [None, Some(10)] {
        let semantic = renderer
            .parse_semantic(source, OperationControl::new())
            .unwrap()
            .unwrap();
        let presentation = renderer
            .prepare_presentation(source, OperationControl::new())
            .unwrap()
            .unwrap();
        let request = AsciiRequest {
            options: AsciiRenderOptions::unicode(),
            viewport: width.map_or_else(AsciiViewportPolicy::unrestricted, |width| {
                AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback)
            }),
            ..Default::default()
        };
        let RenderOutput::Ascii(Some(canonical)) = semantic
            .render(merman::RenderTarget::Ascii(request.clone()))
            .unwrap()
        else {
            panic!("expected canonical ZenUML ASCII output");
        };
        let RenderOutput::Ascii(Some(full)) = presentation
            .render(merman::RenderTarget::Ascii(request))
            .unwrap()
        else {
            panic!("expected presentation ZenUML ASCII output");
        };
        assert_eq!(canonical.text, full.text, "width {width:?}");
        assert_eq!(canonical.report(), full.report());
        assert_eq!(full.family, "zenuml");
        assert_eq!(full.encoding.as_str(), "plain");
        assert_eq!(full.requested_graph_presentation.as_str(), "strict");
        assert!(!full.compact_attempted);
        assert_eq!(full.fallback.attempted, width.is_some());
        assert_eq!(full.primary_overflowed, Some(width.is_some()));
        assert_eq!(
            full.outcome,
            if width.is_some() {
                AsciiOutputOutcome::Fallback
            } else {
                AsciiOutputOutcome::Primary
            }
        );
        assert_eq!(
            full.projection.as_str(),
            if width.is_some() {
                "structured_text"
            } else {
                "diagrammatic"
            }
        );
        assert!(!full.emitted_overflowed);
        if let Some(width) = width {
            assert!(full.emitted_extent.width <= width);
        }
        let compact: String = full.text.split_whitespace().collect();
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
            assert!(
                compact.contains(expected),
                "missing {expected:?}: {}",
                full.text
            );
        }
    }
}

#[cfg(all(feature = "presentation", feature = "diagram-requirement"))]
#[test]
fn requirement_ascii_matches_canonical_and_presentation_records_and_projection() {
    let source = concat!(
        "---\ntitle: Traceability\n---\n",
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
        let renderer = Renderer::new();
        for width in [4, 10, 80] {
            let semantic = renderer
                .parse_semantic(&source, OperationControl::new())
                .unwrap()
                .unwrap();
            let presentation = renderer
                .prepare_presentation(&source, OperationControl::new())
                .unwrap()
                .unwrap();
            let request = AsciiRequest {
                options: AsciiRenderOptions::unicode(),
                viewport: AsciiViewportPolicy::with_max_width(width)
                    .overflow(OverflowPolicy::Fallback),
                ..Default::default()
            };
            let RenderOutput::Ascii(Some(canonical)) = semantic
                .render(merman::RenderTarget::Ascii(request.clone()))
                .unwrap()
            else {
                panic!("expected canonical Requirement ASCII output");
            };
            let RenderOutput::Ascii(Some(full)) = presentation
                .render(merman::RenderTarget::Ascii(request))
                .unwrap()
            else {
                panic!("expected presentation Requirement ASCII output");
            };
            assert_eq!(canonical.text, full.text, "width {width}: {source}");
            assert_eq!(canonical.report(), full.report());
            assert_eq!(full.family, "requirement");
            let fallback_expected = width == 4 && primary_projection == "diagrammatic";
            assert_eq!(
                full.outcome,
                if fallback_expected {
                    AsciiOutputOutcome::Fallback
                } else {
                    AsciiOutputOutcome::Primary
                }
            );
            assert_eq!(
                full.projection.as_str(),
                if fallback_expected {
                    "structured_text"
                } else {
                    primary_projection
                }
            );
            assert!(full.emitted_extent.width <= width);
            let compact: String = full.text.split_whitespace().collect();
            for expected in [
                "name=\"req\"id=\"R-1\"",
                "text=\"Loginrequired\"risk=\"High\"verifyMethod=\"Test\"",
                "N1[element]name=\"sys\"type=\"service\"docRef=\"spec.md\"",
                "Traceability",
            ] {
                assert!(
                    compact.contains(expected),
                    "missing {expected:?}: {}",
                    full.text
                );
            }
            if primary_projection == "structured_text" {
                assert!(
                    compact
                        .contains("source=[](unresolved)name=\"missing\"target=[R1]name=\"req\"")
                );
            } else {
                assert!(compact.contains("source=[N1]name=\"sys\"target=[R1]name=\"req\""));
            }
        }
    }
}

#[cfg(all(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-state",
    feature = "diagram-xychart"
))]
#[test]
fn renderer_renders_shipped_ascii_reference_diagram_families() {
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

    for (source, expected) in cases {
        let output = Renderer::new()
            .with_parse_options(merman::ParseOptions::strict())
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::ascii(),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(rendered)) = output else {
            panic!("diagram not detected");
        };

        assert!(
            rendered.text.contains(expected),
            "expected {expected:?} in rendered output:\n{}",
            rendered.text
        );
    }
}

#[cfg(all(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-state",
    feature = "diagram-xychart"
))]
#[test]
fn model_backend_renders_shipped_typed_models_with_caller_operation_state() {
    let options = AsciiRenderOptions::ascii();

    let class_model = render_model_for("classDiagram\nclass Animal");
    let rendered = render_typed_model(&class_model, options, AsciiResourcePolicy::default())
        .expect("class model should render");
    assert!(rendered.contains("Animal"));

    let er_model = render_model_for("erDiagram\nCUSTOMER");
    let rendered = render_typed_model(&er_model, options, AsciiResourcePolicy::default())
        .expect("ER model should render");
    assert!(rendered.contains("CUSTOMER"));

    let state_model = render_model_for("stateDiagram-v2\n[*] --> Ready");
    let rendered = render_typed_model(&state_model, options, AsciiResourcePolicy::default())
        .expect("State model should render");
    assert!(rendered.contains("Ready"));

    let xychart_model = render_model_for(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
    );
    let rendered = render_typed_model(&xychart_model, options, AsciiResourcePolicy::default())
        .expect("XYChart model should render");
    assert!(rendered.contains("###"));
}

#[test]
fn renderer_uses_ascii_options_for_padding() {
    let mut options = AsciiRenderOptions::ascii().with_node_padding(1);
    options.graph_padding_x = 2;
    options.graph_padding_y = 1;
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "graph LR\nA --> B",
            OperationControl::new(),
            AsciiRequest {
                options,
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(rendered)) = output else {
        panic!("diagram not detected");
    };

    assert_eq!(
        rendered.text,
        "+---+  +---+\n|   |  |   |\n| A |->| B |\n|   |  |   |\n+---+  +---+\n"
    );
}

#[test]
fn canonical_ascii_renderer_applies_flowchart_node_label_wrapping() {
    let source = "flowchart TD\nA[\"Alpha Beta Gamma Delta\"]";
    let options = AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(8);
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            AsciiRequest {
                options,
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(rendered)) = output else {
        panic!("diagram not detected");
    };

    for expected in ["Alpha", "Beta", "Gamma", "Delta"] {
        assert!(
            rendered.text.contains(expected),
            "missing {expected:?}:\n{}",
            rendered.text
        );
    }
    assert!(
        !rendered.text.contains("Alpha Beta Gamma Delta"),
        "{}",
        rendered.text
    );
}

#[test]
fn renderer_renders_sequence_with_unicode_defaults() {
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello",
            OperationControl::new(),
            AsciiRequest::default(),
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(rendered)) = output else {
        panic!("diagram not detected");
    };

    assert!(rendered.text.contains("┌"));
    assert!(rendered.text.contains("Hello"));
    assert!(rendered.text.contains("►"));
}

#[test]
fn renderer_returns_no_ascii_when_no_diagram_is_detected() {
    let error = Renderer::new()
        .with_parse_options(merman::ParseOptions::lenient())
        .render(RenderRequest::ascii(
            "this is just prose",
            OperationControl::new(),
            AsciiRequest::default(),
        ))
        .expect_err("a missing ASCII diagram should be a typed facade error");

    assert!(matches!(error, merman::RenderError::NoDiagram));
}

#[test]
fn render_ascii_model_handles_deep_flowchart_subgraph_chain_with_small_stack() {
    const DEPTH: usize = 512;
    let source = deeply_nested_flowchart(DEPTH);
    let model = render_model_for(&source);
    let error = render_typed_model(
        &model,
        AsciiRenderOptions::ascii(),
        AsciiResourcePolicy::default(),
    )
    .expect_err("the Interactive profile should reject nesting beyond its public limit");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxNestingDepth
                && details.actual == 257
                && details.max == 256
    ));

    let handle = std::thread::Builder::new()
        .name("ascii-deep-flowchart-subgraph".to_string())
        // The debug facade path keeps several fixed-size render frames alive at once. Keep the
        // constrained-stack signal while leaving enough room for those frames across toolchains.
        .stack_size(256 * 1024)
        .spawn(move || {
            let options = AsciiRenderOptions::ascii();
            let control = merman::OperationControl::new();
            let mut resources =
                AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
            resources
                .apply_limit(AsciiResourceLimitId::MaxGridCells, 10_000_000)
                .expect("valid ASCII grid override");
            let context = merman::model::runtime::RuntimePolicy::deterministic()
                .begin_operation()
                .expect("deterministic operation context");
            let rendered = AsciiRenderer::new(options)
                .expect("ASCII options should validate")
                .render_parsed(&model, &control, &context, resources)
                .expect("deep Flowchart ASCII render should not return an error");
            assert!(rendered.contains('A'));
        })
        .expect("spawn deep Flowchart ASCII render test");
    handle
        .join()
        .expect("deep Flowchart ASCII render should not overflow the stack");
}

#[test]
fn host_recipe_preserves_typed_auto_outcomes_and_hard_failures() {
    use merman::RenderError;
    use merman::ascii::AsciiLayoutProfile;

    let source = "flowchart LR\nA[Read the complete Mermaid source] --> B[Return the complete rendered artifact]";
    let renderer = Renderer::new();
    let options = AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Auto);
    let request = |width, overflow| AsciiRequest {
        options,
        viewport: AsciiViewportPolicy::with_max_width(width).overflow(overflow),
        ..Default::default()
    };
    for (width, overflow, expected) in [
        (80, OverflowPolicy::Error, AsciiOutputOutcome::Primary),
        (40, OverflowPolicy::Allow, AsciiOutputOutcome::WideAllowed),
        (40, OverflowPolicy::Fallback, AsciiOutputOutcome::Fallback),
    ] {
        let output = renderer
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                request(width, overflow),
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(report)) = output else {
            panic!("expected ASCII report")
        };
        assert_eq!(report.outcome, expected);
        assert_eq!(report.requested_layout_profile, AsciiLayoutProfile::Auto);
        assert_eq!(report.layout_profile, Some(AsciiLayoutProfile::Compact));
        assert!(report.compact_attempted);
        assert!(!report.text.contains('\u{1b}'));
        let json = serde_json::to_value(report.report()).unwrap();
        assert_eq!(json["text"], report.text);
        assert_eq!(json["emitted_width"], report.emitted_extent.width);
        assert_eq!(json["outcome"], expected.as_str());
        assert_eq!(
            report.fallback.attempted,
            expected == AsciiOutputOutcome::Fallback
        );
    }
    let error = renderer
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            request(40, OverflowPolicy::Error),
        ))
        .unwrap_err();
    assert!(matches!(
        error,
        RenderError::Ascii(AsciiError::WidthOverflow { max_width: 40, .. })
    ));

    let cancelled = OperationControl::new();
    cancelled.cancel();
    let error = renderer
        .render(RenderRequest::ascii(
            source,
            cancelled,
            request(40, OverflowPolicy::Fallback),
        ))
        .unwrap_err();
    assert!(matches!(error, RenderError::Cancelled(_)));

    let mut limited = request(40, OverflowPolicy::Fallback);
    limited
        .resources
        .apply_limit(AsciiResourceLimitId::MaxGridCells, 1)
        .unwrap();
    let error = renderer
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            limited,
        ))
        .unwrap_err();
    assert!(matches!(error, RenderError::ResourceLimitExceeded(_)));
}

#[test]
fn terminal_palette_recipe_preserves_geometry_and_rejects_styled_fallback() {
    use merman::RenderError;
    use merman::ascii::{AsciiColorMode, AsciiColorTheme, AsciiRgb, AsciiTerminalPalette};

    let source = "flowchart LR\nA[Host] --> B[Text]";
    let renderer = Renderer::new();
    let theme = AsciiColorTheme::from_terminal_palette(AsciiTerminalPalette::new(
        AsciiRgb::new(229, 231, 235),
        AsciiRgb::new(15, 23, 42),
    ));
    let mut plain = None;
    for mode in [AsciiColorMode::Plain, AsciiColorMode::TrueColor] {
        let options = AsciiRenderOptions::unicode()
            .with_color_mode(mode)
            .with_color_theme(theme);
        let output = renderer
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                AsciiRequest {
                    options,
                    viewport: AsciiViewportPolicy::with_max_width(80)
                        .overflow(OverflowPolicy::Error),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(report)) = output else {
            panic!("expected ASCII report")
        };
        if mode == AsciiColorMode::Plain {
            assert!(!report.text.contains('\u{1b}'));
            plain = Some(report.emitted_extent);
        } else {
            assert_eq!(Some(report.emitted_extent), plain);
            assert_eq!(report.metadata().encoding, "truecolor");
            assert!(report.text.contains("\u{1b}[38;2;229;231;235m"));
            let error = renderer
                .render(RenderRequest::ascii(
                    source,
                    OperationControl::new(),
                    AsciiRequest {
                        options,
                        viewport: AsciiViewportPolicy::with_max_width(80)
                            .overflow(OverflowPolicy::Fallback),
                        ..Default::default()
                    },
                ))
                .unwrap_err();
            assert!(matches!(
                error,
                RenderError::Ascii(AsciiError::InvalidOption {
                    field: "ascii_viewport.overflow",
                    ..
                })
            ));
        }
    }
}

#[test]
fn explicit_ascii_request_is_independent_of_terminal_environment() {
    const CHILD_FLAG: &str = "MERMAN_TEST_TERMINAL_ENV_CHILD";
    const MARKER: &str = "MERMAN_HOST_REPORT:";
    if std::env::var_os(CHILD_FLAG).is_some() {
        let output = Renderer::new()
            .render(RenderRequest::ascii(
                "flowchart LR\nA[Host] --> B[Artifact]",
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::unicode(),
                    viewport: AsciiViewportPolicy::with_max_width(80),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(report)) = output else {
            panic!("expected ASCII report")
        };
        assert!(!report.text.contains('\u{1b}'));
        println!(
            "\n{MARKER}{}",
            serde_json::to_string(&report.report()).unwrap()
        );
        return;
    }

    // Configure child environments instead of mutating process-global state in parallel tests.
    let environments = [
        vec![],
        vec![
            ("TERM", "dumb"),
            ("NO_COLOR", "1"),
            ("COLUMNS", "10"),
            ("LINES", "1"),
        ],
        vec![
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("CLICOLOR_FORCE", "1"),
            ("COLUMNS", "200"),
        ],
    ];
    let mut reports = Vec::new();
    for environment in environments {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "explicit_ascii_request_is_independent_of_terminal_environment",
                "--nocapture",
            ])
            .env(CHILD_FLAG, "1");
        for key in [
            "TERM",
            "NO_COLOR",
            "COLORTERM",
            "CLICOLOR",
            "CLICOLOR_FORCE",
            "COLUMNS",
            "LINES",
        ] {
            command.env_remove(key);
        }
        command.envs(environment);
        let output = command.output().unwrap();
        assert!(output.status.success(), "child failed: {:?}", output);
        let stdout = String::from_utf8(output.stdout).unwrap();
        let report = stdout
            .lines()
            .find_map(|line| line.strip_prefix(MARKER))
            .expect("child must emit its report");
        reports.push(serde_json::from_str::<serde_json::Value>(report).unwrap());
    }
    assert_eq!(reports[0], reports[1]);
    assert_eq!(reports[0], reports[2]);
}
