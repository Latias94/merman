use super::*;

#[test]
fn composite_source_group_compositing_preserves_the_scene_and_reports_unsupported_depth() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 0.0, -40.0, None);
    let baseline_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let simple = "state Review {\nA --> B\n}\n";
    let nested = "state Review {\nstate Inner {\nA --> B\n}\n}\n";
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html_labels in [false, true] {
            for (graph, declarations, group_compositing) in [
                (simple, "classDef clusters opacity:0.5\n", true),
                (simple, "classDef clusters -webkit-opacity:0.5\n", true),
                (nested, "classDef clusters opacity:0.5\n", true),
                (nested, "classDef root opacity:0.5\n", true),
                (nested, "classDef nodes opacity:0.5\n", layout == "dagre"),
                (simple, "classDef clusters filter:blur(1px)\n", true),
                (simple, "classDef clusters -webkit-filter:blur(1px)\n", true),
                (simple, "classDef clusters fill-opacity:0.5\n", false),
                (
                    simple,
                    "classDef clusters opacity:0.5,opacity:1,filter:blur(1px),filter:none\n",
                    false,
                ),
                (
                    simple,
                    "classDef clusters opacity:0.5,-webkit-opacity:1,filter:blur(1px),-webkit-filter:none\n",
                    false,
                ),
                (
                    simple,
                    "classDef clusters -webkit-opacity:0.5,opacity:1,-webkit-filter:blur(1px),filter:none\n",
                    false,
                ),
                (
                    simple,
                    "classDef clusters opacity:1,-webkit-opacity:0.5\n",
                    true,
                ),
                (
                    simple,
                    "classDef clusters -webkit-opacity:1,opacity:0.5\n",
                    true,
                ),
                (
                    simple,
                    "classDef clusters filter:none,-webkit-filter:blur(1px)\n",
                    true,
                ),
                (
                    simple,
                    "classDef clusters -webkit-filter:none,filter:blur(1px)\n",
                    true,
                ),
                (
                    simple,
                    "classDef translucent opacity:0.5\nclass Review translucent\n",
                    false,
                ),
                ("A --> B\n", "classDef clusters opacity:0.5\n", false),
            ] {
                let source = format!("stateDiagram-v2\n{graph}{declarations}");
                let config = serde_json::json!({
                    "look": "classic", "layout": layout, "htmlLabels": html_labels,
                });
                let context = format!("{layout}/{html_labels}/{source}");
                let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
                    &theme,
                    &source,
                    config.clone(),
                    "site",
                );
                let blocked = html_labels && group_compositing;
                assert_eq!(
                    evidence,
                    StateThemeEvidenceCounts {
                        required: 1,
                        accounted: 1,
                        applied: usize::from(!blocked),
                        not_applicable: 0,
                        residual: usize::from(blocked),
                    },
                    "{context}",
                );
                let doc = roxmltree::Document::parse(&svg).unwrap();
                let shadows = doc
                    .descendants()
                    .filter(|node| node.attribute("data-state-shadow-for").is_some())
                    .count();
                assert_eq!(shadows, if blocked { 0 } else { 2 }, "{context}");
                let mut ids = std::collections::BTreeSet::new();
                for id in doc.descendants().filter_map(|node| node.attribute("id")) {
                    assert!(ids.insert(id), "duplicate {id}: {context}");
                }
                if !blocked {
                    continue;
                }
                let (baseline, _) = render_strict_state_svg_with_theme_and_config(
                    &baseline_theme,
                    &source,
                    config.clone(),
                    "site",
                );
                // No canvas paint is installed. Equal SVG on the transparent canvas
                // preserves the title/body overlap alpha (.5, rather than split .75),
                // including all source selectors and group/filter compositing boundaries.
                assert_eq!(svg, baseline, "{context}");
                for label in doc
                    .descendants()
                    .filter(|node| node.attribute("class") == Some("cluster-label"))
                {
                    let owner = label.parent().unwrap();
                    assert!(owner.attribute("id").is_some(), "{context}");
                    assert!(
                        owner.descendants().any(|node| {
                            node.has_tag_name("rect") && node.attribute("class") == Some("outer")
                        }),
                        "{context}",
                    );
                    assert!(
                        owner.children().any(|node| {
                            node.has_tag_name("rect") && node.attribute("class") == Some("inner")
                        }),
                        "{context}",
                    );
                }

                let parsed = common::install_parse_compatibility(
                    &theme,
                    classic_typed_engine().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                )
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .unwrap();
                assert!(
                    matches!(
                        family::prepare(&parsed, &LayoutOptions::default(), session),
                        Err(merman_render::Error::UnverifiedFamilyTheme {
                            family_id: merman_model::family::DiagramFamilyId::STATE,
                            residual_count: 1,
                        })
                    ),
                    "{context}",
                );
            }
        }
    }
}

#[test]
fn source_filter_ownership_prevents_depth_without_suppressing_unaffected_nodes() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 5.0, 6.0, None);
    for html_labels in [false, true] {
        for (declarations, blocked, rejected) in [
            ("classDef node filter:none\n", &["A", "B"][..], false),
            (
                "classDef node -webkit-filter:none\n",
                &["A", "B"][..],
                false,
            ),
            (
                "classDef filtered filter:none\nclass A filtered\n",
                &["A"][..],
                false,
            ),
            ("style A filter:none\n", &["A"][..], false),
            (
                "classDef filtered -webkit-filter:none\nclass A filtered\n",
                &["A"][..],
                false,
            ),
            ("style A -webkit-filter:none\n", &["A"][..], false),
            (
                "classDef filtered filter:blur(0px)\nclass A filtered\n",
                &["A"][..],
                false,
            ),
            ("classDef node filter:url(#source-filter)\n", &[][..], true),
            (
                "classDef filtered filter:url(#source-filter)\nclass A filtered\n",
                &[][..],
                true,
            ),
            ("style A filter:url(#source-filter)\n", &[][..], true),
            (
                "classDef node -webkit-filter:url(#source-filter)\n",
                &[][..],
                true,
            ),
            (
                "style A -webkit-filter:url(#source-filter)\n",
                &[][..],
                true,
            ),
            (
                "style A filter:none,filter:url(#source-filter)\n",
                &["A"][..],
                true,
            ),
            (
                "style A filter:none,-webkit-filter:url(#source-filter)\n",
                &["A"][..],
                true,
            ),
            (
                "style A -webkit-filter:none,filter:url(#source-filter)\n",
                &["A"][..],
                true,
            ),
        ] {
            let config = serde_json::json!({
                "look": "classic",
                "layout": "dagre",
                "htmlLabels": html_labels,
            });
            let parsed = common::install_parse_compatibility(
                &theme,
                classic_typed_engine().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                ),
            )
            .parse_diagram_for_render_model_sync(
                &format!("stateDiagram-v2\nA --> B\n{declarations}"),
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
                .begin_session_with_theme(&theme)
                .unwrap();
            let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            for id in ["A", "B"] {
                let node_id = format!("state-{id}-");
                let body = doc
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .attribute("id")
                                .is_some_and(|value| value.contains(&node_id))
                    })
                    .unwrap();
                let body_id = body.attribute("id").unwrap();
                let depth_count = doc
                    .descendants()
                    .filter(|node| node.attribute("data-state-shadow-for") == Some(body_id))
                    .count();
                let source_owned = blocked.contains(&id);
                assert_eq!(
                    depth_count,
                    usize::from(!source_owned),
                    "{html_labels}: {id}: {declarations}",
                );
                let rect = body
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                assert_eq!(
                    rect.attribute("style")
                        .unwrap_or_default()
                        .contains("filter:"),
                    source_owned,
                    "{html_labels}: {id}: {declarations}",
                );
                assert!(rect.attribute("filter").is_none());
            }
            assert!(!rendered.svg().contains("source-filter"));
            let expected_filters = 2 - blocked.len();
            let filter_count = doc
                .descendants()
                .filter(|node| {
                    node.has_tag_name("filter")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.ends_with("-theme-effect-state-hard-shadow"))
                })
                .count();
            assert_eq!(filter_count, expected_filters);
            drop(doc);
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(evidence.applied_count(), usize::from(expected_filters != 0));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(expected_filters == 0)
            );
            assert_eq!(evidence.theme_residual_count(), 0);
            assert_eq!(evidence.source_residual_count() != 0, rejected);
            let receipt =
                merman_render::__private::family_native_filter_receipt(completion.report());
            assert_eq!(receipt.is_some(), expected_filters != 0);
            if let Some(receipt) = receipt {
                assert_eq!(receipt.filter_count() as usize, expected_filters);
                assert_eq!(receipt.reference_count() as usize, expected_filters);
            }
        }
    }
}

#[test]
fn source_filter_aliases_keep_the_last_valid_winner_and_authored_spelling() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 5.0, 6.0, None);
    for html_labels in [false, true] {
        for origin in ["structural", "assigned", "inline"] {
            for (declarations, expected) in [
                (
                    "filter:blur(1px),-webkit-filter:none,filter:blur(0px)",
                    "filter:blur(0px) !important",
                ),
                (
                    "-webkit-filter:blur(1px),filter:none,-webkit-filter:blur(0px)",
                    "-webkit-filter:blur(0px) !important",
                ),
                (
                    "filter:blur(0px),-webkit-filter:url(#source-filter)",
                    "filter:blur(0px) !important",
                ),
            ] {
                let source_style = match origin {
                    "structural" => format!("classDef node {declarations}"),
                    "assigned" => format!("classDef filtered {declarations}\nclass A filtered"),
                    _ => format!("style A {declarations}"),
                };
                let source = format!("stateDiagram-v2\nA --> B\n{source_style}\n");
                let (svg, _) = render_strict_state_svg_with_theme_and_config(
                    &theme,
                    &source,
                    serde_json::json!({"look": "classic", "htmlLabels": html_labels}),
                    "site",
                );
                let doc = roxmltree::Document::parse(&svg).unwrap();
                let body = doc
                    .descendants()
                    .find(|node| {
                        node.attribute("id")
                            .is_some_and(|id| id.contains("state-A-"))
                    })
                    .unwrap();
                let rect = body
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                let filters: Vec<_> = rect
                    .attribute("style")
                    .unwrap_or_default()
                    .split(';')
                    .filter(|declaration| {
                        declaration.starts_with("filter:")
                            || declaration.starts_with("-webkit-filter:")
                    })
                    .collect();
                assert_eq!(
                    filters,
                    [expected],
                    "html={html_labels}, {origin}: {declarations}",
                );
                assert!(!svg.contains("source-filter"));
            }
        }
    }
}

#[test]
fn structural_ancestor_filters_follow_native_and_html_selectors_in_flat_and_nested_roots() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 5.0, 6.0, None);
    for html_labels in [false, true] {
        for nested in [false, true] {
            for class_id in ["root", "nodes"] {
                for (declarations, emitted_filter, rejected) in [
                    ("filter:none", Some(("filter", "none")), false),
                    (
                        "-webkit-filter:none",
                        Some(("-webkit-filter", "none")),
                        false,
                    ),
                    ("filter:blur(0px)", Some(("filter", "blur(0px)")), false),
                    (
                        "-webkit-filter:blur(0px)",
                        Some(("-webkit-filter", "blur(0px)")),
                        false,
                    ),
                    ("filter:url(#source-filter)", None, true),
                    ("-webkit-filter:url(#source-filter)", None, true),
                    (
                        "filter:none,filter:url(#source-filter)",
                        Some(("filter", "none")),
                        true,
                    ),
                    (
                        "filter:none,-webkit-filter:url(#source-filter)",
                        Some(("filter", "none")),
                        true,
                    ),
                    (
                        "-webkit-filter:none,filter:url(#source-filter)",
                        Some(("-webkit-filter", "none")),
                        true,
                    ),
                ] {
                    let graph = if nested {
                        "state Container {\nA --> B\n}\nC --> D\n"
                    } else {
                        "A --> B\nC --> D\n"
                    };
                    let source = format!(
                        "stateDiagram-v2\n{graph}style A fill:#ffe500,fill-opacity:0.5\nclassDef {class_id} {declarations}\n"
                    );
                    let parsed = common::install_parse_compatibility(
                        &theme,
                        classic_typed_engine().with_site_config(
                            merman_core::presentation::SourcePresentationConfig::from_value(
                                serde_json::json!({
                                    "look": "classic",
                                    "layout": "dagre",
                                    "htmlLabels": html_labels,
                                }),
                            ),
                        ),
                    )
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                    let session = RenderEnvironment::deterministic()
                        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
                        .begin_session_with_theme(&theme)
                        .unwrap();
                    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
                        .unwrap()
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                        .unwrap();
                    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let context =
                        format!("html={html_labels}, nested={nested}, {class_id}: {declarations}");
                    let root_count = doc
                        .descendants()
                        .filter(|node| {
                            node.attribute("class") == Some("root")
                                && node.attribute("data-state-depth").is_none()
                        })
                        .count();
                    assert_eq!(root_count > 1, nested, "{context}");
                    if let Some((property, filter)) = emitted_filter {
                        let selector = if html_labels {
                            format!(".{class_id}>*{{{property}:{filter}!important;}}")
                        } else {
                            format!(".{class_id} rect{{{property}:{filter}!important;}}")
                        };
                        assert!(rendered.svg().contains(&selector), "{context}");
                    }
                    assert!(!rendered.svg().contains("source-filter"), "{context}");
                    let source_owned = !html_labels && emitted_filter.is_some();
                    let incompatible_compositing = html_labels
                        && nested
                        && emitted_filter.is_some_and(|(_, value)| value == "blur(0px)");
                    let has_depth = !source_owned && !incompatible_compositing;
                    let expected_filters = if has_depth { 4 } else { 0 };
                    for id in ["A", "B", "C", "D"] {
                        let node_id = format!("state-{id}-");
                        let bodies: Vec<_> = doc
                            .descendants()
                            .filter(|node| {
                                node.has_tag_name("g")
                                    && node
                                        .attribute("id")
                                        .is_some_and(|value| value.contains(&node_id))
                            })
                            .collect();
                        assert_eq!(bodies.len(), 1, "{id}: {context}");
                        let body = bodies[0];
                        assert!(
                            !body
                                .attribute("class")
                                .unwrap()
                                .split_whitespace()
                                .any(|assigned| assigned == class_id),
                            "the ancestor class must not be explicitly assigned: {context}"
                        );
                        assert_eq!(
                            body.parent().unwrap().attribute("class"),
                            Some("nodes"),
                            "{id}: {context}",
                        );
                        let rect = body
                            .children()
                            .find(|node| node.has_tag_name("rect"))
                            .unwrap();
                        assert!(rect.attribute("filter").is_none(), "{id}: {context}");
                        let style = rect.attribute("style").unwrap_or_default();
                        assert!(!style.contains("filter:"), "{id}: {context}");
                        if id == "A" {
                            assert!(style.contains("fill-opacity:0.5"), "{context}");
                        }
                        assert_eq!(
                            doc.descendants()
                                .filter(|node| {
                                    node.attribute("data-state-shadow-for") == body.attribute("id")
                                })
                                .count(),
                            usize::from(has_depth),
                            "{id}: {context}",
                        );
                    }
                    let filter_count = doc
                        .descendants()
                        .filter(|node| {
                            node.has_tag_name("filter")
                                && node.attribute("id").is_some_and(|id| {
                                    id.ends_with("-theme-effect-state-hard-shadow")
                                })
                        })
                        .count();
                    assert_eq!(filter_count, expected_filters, "{context}");
                    drop(doc);
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.required_count(), 1, "{context}");
                    assert_eq!(evidence.accounted_count(), 1, "{context}");
                    assert_eq!(
                        evidence.applied_count(),
                        usize::from(has_depth),
                        "{context}",
                    );
                    assert_eq!(
                        evidence.not_applicable_count(),
                        usize::from(source_owned),
                        "{context}",
                    );
                    assert_eq!(
                        evidence.theme_residual_count(),
                        usize::from(incompatible_compositing),
                        "{context}",
                    );
                    assert_eq!(
                        completion
                            .report()
                            .theme_diagnostics()
                            .iter()
                            .filter(|diagnostic| {
                                diagnostic.code() == "unsupported-effect"
                                    && diagnostic.target() == Some(ThemeTarget::State)
                            })
                            .count(),
                        usize::from(incompatible_compositing),
                        "{context}",
                    );
                    assert_eq!(evidence.source_residual_count() != 0, rejected, "{context}");
                    let receipt =
                        merman_render::__private::family_native_filter_receipt(completion.report());
                    assert_eq!(receipt.is_some(), has_depth, "{context}");
                    if let Some(receipt) = receipt {
                        assert_eq!(
                            receipt.filter_count() as usize,
                            expected_filters,
                            "{context}",
                        );
                        assert_eq!(
                            receipt.reference_count() as usize,
                            expected_filters,
                            "{context}",
                        );
                    }
                }
            }
        }
    }
}
