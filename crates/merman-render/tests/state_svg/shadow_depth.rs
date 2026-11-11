use super::*;
use merman_render::diagram_theme::{EffectColorSpace, EffectComposition};

fn depth_theme(
    composition: EffectComposition,
    color_space: EffectColorSpace,
    second_input: EffectInput,
    offset: f32,
    clear: bool,
) -> DiagramTheme {
    let graph = EffectGraph::new(
        "body-depth",
        [
            EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: offset,
                offset_y: offset,
                blur_radius: 0.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#f32888").unwrap(),
            },
            EffectPrimitive::DropShadow {
                input: second_input,
                offset_x: offset / 2.0,
                offset_y: offset / 2.0,
                blur_radius: 0.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#191620").unwrap(),
            },
        ],
    )
    .unwrap()
    .with_color_space(color_space)
    .with_composition(composition)
    .unwrap();
    let mut spec = state_theme_spec().with_effects(
        DiagramEffectSet::default()
            .with_graph(graph)
            .unwrap()
            .with_binding(EffectBinding::new(ThemeTarget::State, "body-depth").unwrap())
            .unwrap(),
    );
    if clear {
        spec = spec.with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::State,
            ThemeStylePatch {
                effects: ThemeEffectPatch {
                    effect: Specified::Clear,
                },
                ..ThemeStylePatch::default()
            },
        )));
    }
    DiagramThemeCompiler::new().compile(spec).unwrap()
}

fn render_depth(
    source: &str,
    config: serde_json::Value,
    theme: Option<&DiagramTheme>,
    debug: &SvgDebugOptions,
    policy: RenderResourcePolicy,
) -> merman_render::Result<(String, usize)> {
    let engine = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config));
    let engine = if let Some(theme) = theme {
        common::install_parse_compatibility(theme, engine)
    } else {
        engine
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic().with_resource_policy(policy);
    let session = if let Some(theme) = theme {
        environment.begin_session_with_theme(theme).unwrap()
    } else {
        environment.begin_session().unwrap()
    };
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let (svg, report) = artifact
        .render_svg(&SvgRenderOptions::default(), debug)?
        .into_completion()
        .into_output_and_report();
    Ok((svg, report.session_report().layout_work_units()))
}

fn config(layout: &str, html: bool) -> serde_json::Value {
    serde_json::json!({"theme":"default", "look":"classic", "layout":layout, "htmlLabels":html})
}

fn final_inputs<'a>(filter: roxmltree::Node<'a, '_>) -> Vec<&'a str> {
    filter
        .children()
        .rfind(roxmltree::Node::is_element)
        .unwrap()
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|node| node.attribute("in").unwrap())
        .collect()
}

fn scene_ancestry<'a>(node: roxmltree::Node<'a, '_>) -> Vec<(&'a str, Option<&'a str>)> {
    node.ancestors()
        .skip(1)
        .filter_map(|ancestor| {
            let class = ancestor.attribute("class")?;
            matches!(class, "root" | "nodes").then_some((class, ancestor.attribute("transform")))
        })
        .collect()
}

#[test]
fn shadows_stay_below_arrows_and_bodies_without_changing_routes() {
    let source = "stateDiagram-v2\n[*] --> Checking\nChecking --> Decision: checks complete\nDecision --> Checking: needs detail\nstyle Checking fill:#ffe500,fill-opacity:0.5,opacity:0.75,stroke-width:2.6px\n";
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html in [false, true] {
            let cleared = depth_theme(
                EffectComposition::Independent,
                EffectColorSpace::Srgb,
                EffectInput::SourceGraphic,
                6.0,
                true,
            );
            let (baseline, _) = render_depth(
                source,
                config(layout, html),
                Some(&cleared),
                &SvgDebugOptions::default(),
                RenderResourcePolicy::default(),
            )
            .unwrap();
            let baseline_doc = roxmltree::Document::parse(&baseline).unwrap();
            for offset in [6.0, -6.0] {
                let theme = depth_theme(
                    EffectComposition::Independent,
                    EffectColorSpace::Srgb,
                    EffectInput::SourceGraphic,
                    offset,
                    false,
                );
                let (svg, _) = render_depth(
                    source,
                    config(layout, html),
                    Some(&theme),
                    &SvgDebugOptions::default(),
                    RenderResourcePolicy::default(),
                )
                .unwrap();
                let doc = roxmltree::Document::parse(&svg).unwrap();
                let root = doc
                    .descendants()
                    .find(|n| {
                        n.attribute("class") == Some("root")
                            && n.children().any(|child| {
                                matches!(
                                    child.attribute("class"),
                                    Some("edgePaths" | "edges edgePaths")
                                )
                            })
                    })
                    .unwrap();
                let depth = doc
                    .descendants()
                    .find(|n| n.attribute("data-state-depth") == Some("true"))
                    .unwrap();
                let context = format!("{layout}/{html}/{offset}");
                assert_eq!(depth.parent(), root.parent(), "{context}");
                assert_eq!(depth.next_sibling_element(), Some(root), "{context}");
                // roxmltree ranges are half-open; adjacent scene groups share this boundary.
                assert_eq!(depth.range().end, root.range().start, "{context}");
                let last_shadow = depth
                    .descendants()
                    .filter(|node| node.attribute("data-state-shadow-for").is_some())
                    .map(|node| node.range().end)
                    .max()
                    .unwrap();
                assert_eq!(
                    depth
                        .children()
                        .filter(roxmltree::Node::is_element)
                        .map(|n| n.attribute("class").unwrap())
                        .collect::<Vec<_>>(),
                    ["clusters", "nodes"]
                );
                let groups: Vec<_> = root
                    .children()
                    .filter(roxmltree::Node::is_element)
                    .map(|n| n.attribute("class").unwrap())
                    .collect();
                assert_eq!(
                    groups,
                    [
                        "clusters",
                        if layout == "elk" {
                            "edges edgePaths"
                        } else {
                            "edgePaths"
                        },
                        "edgeLabels",
                        "nodes"
                    ]
                );
                for before in baseline_doc
                    .descendants()
                    .filter(|n| n.attribute("data-edge") == Some("true"))
                {
                    let after = doc
                        .descendants()
                        .find(|n| {
                            n.attribute("data-edge") == Some("true")
                                && n.attribute("data-id") == before.attribute("data-id")
                        })
                        .unwrap();
                    assert!(last_shadow < after.range().start, "{context}");
                    for attr in ["d", "data-points", "marker-end", "style"] {
                        assert_eq!(
                            after.attribute(attr),
                            before.attribute(attr),
                            "{layout} {html} {offset} {attr}"
                        );
                    }
                }
                let bodies = root
                    .children()
                    .find(|n| n.attribute("class") == Some("nodes"))
                    .unwrap();
                for body in bodies
                    .children()
                    .filter(|n| n.attribute("class") == Some("node statediagram-state"))
                {
                    assert!(last_shadow < body.range().start, "{context}");
                    let id = body.attribute("id").unwrap();
                    let shadow = doc
                        .descendants()
                        .find(|n| n.attribute("data-state-shadow-for") == Some(id))
                        .unwrap();
                    assert_eq!(shadow.attribute("transform"), body.attribute("transform"));
                    assert!(shadow.attribute("id").is_none());
                    assert!(
                        !shadow
                            .descendants()
                            .any(|n| n.has_tag_name("text") || n.has_tag_name("foreignObject"))
                    );
                    let rect = body.children().find(|n| n.has_tag_name("rect")).unwrap();
                    let shadow_rect = shadow.children().find(|n| n.has_tag_name("rect")).unwrap();
                    assert!(rect.attribute("filter").is_none());
                    for attr in ["style", "x", "y", "width", "height", "rx", "ry"] {
                        assert_eq!(shadow_rect.attribute(attr), rect.attribute(attr));
                    }
                }
                for filter in doc.descendants().filter(|n| {
                    n.has_tag_name("filter")
                        && n.attribute("id")
                            .is_some_and(|id| id.ends_with("-theme-effect-body-depth"))
                }) {
                    assert_eq!(
                        final_inputs(filter),
                        ["merman-shadow-0-shadow", "merman-shadow-1-shadow"]
                    );
                }
                assert_eq!(state_effect_filter_regions(&svg).len(), 2);
            }
        }
    }
}

#[test]
fn structural_ancestor_opacity_reaches_body_and_depth_in_the_same_source_context() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 5.0, 6.0, None);
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html in [false, true] {
            for nested in [false, true] {
                for declarations in [
                    "opacity:0",
                    "opacity:0.4,fill-opacity:0.5,stroke-opacity:0.25",
                ] {
                    let graph = if nested {
                        "state Container {\nA --> B\n}\nC --> D\n"
                    } else {
                        "A --> B\nC --> D\n"
                    };
                    let source = format!(
                        "stateDiagram-v2\n{graph}classDef root fill-opacity:0.75\nclassDef nodes {declarations}\nclassDef painted fill-opacity:0.25\nclass A painted\nstyle A fill:#ffe500,opacity:0.75,stroke-width:2.6px,rx:7px\n"
                    );
                    let (svg, _) = render_depth(
                        &source,
                        config(layout, html),
                        Some(&theme),
                        &SvgDebugOptions::default(),
                        RenderResourcePolicy::default(),
                    )
                    .unwrap();
                    let context = format!("{layout}/{html}/{nested}/{declarations}");
                    let selector = if html { ".nodes>*{" } else { ".nodes rect{" };
                    let opacity = declarations.split(',').next().unwrap();
                    assert!(
                        svg.contains(&format!("{selector}{opacity}!important;")),
                        "{context}"
                    );
                    let doc = roxmltree::Document::parse(&svg).unwrap();
                    if html && nested && layout == "dagre" {
                        // .nodes>* composites the entire nested root. Keep the source
                        // scene unchanged rather than asserting ancestry of split groups.
                        let (baseline, _) = render_depth(
                            &source,
                            config(layout, html),
                            None,
                            &SvgDebugOptions::default(),
                            RenderResourcePolicy::default(),
                        )
                        .unwrap();
                        assert_eq!(svg, baseline, "{context}");
                        assert!(!svg.contains("data-state-shadow-for="), "{context}");
                        continue;
                    }
                    for id in ["A", "B", "C", "D"] {
                        let body = doc
                            .descendants()
                            .find(|node| {
                                node.has_tag_name("g")
                                    && node.attribute("id").is_some_and(|dom_id| {
                                        dom_id.contains(&format!("state-{id}-"))
                                    })
                            })
                            .unwrap();
                        let shadow = doc
                            .descendants()
                            .find(|node| {
                                node.attribute("data-state-shadow-for") == body.attribute("id")
                            })
                            .unwrap();
                        assert_eq!(
                            shadow.attribute("class"),
                            body.attribute("class"),
                            "{id}: {context}"
                        );
                        assert_eq!(
                            shadow.attribute("transform"),
                            body.attribute("transform"),
                            "{id}: {context}"
                        );
                        // Native .nodes rect and HTML .nodes>* must reach the same terminal
                        // or compositing group, including each enclosing nested root.
                        assert_eq!(
                            shadow.parent().unwrap().attribute("class"),
                            Some("nodes"),
                            "{id}: {context}"
                        );
                        assert_eq!(
                            body.parent().unwrap().attribute("class"),
                            Some("nodes"),
                            "{id}: {context}"
                        );
                        let ancestors = scene_ancestry(body);
                        assert_eq!(scene_ancestry(shadow), ancestors, "{id}: {context}");
                        let expected_owners =
                            if layout == "dagre" && nested && matches!(id, "A" | "B") {
                                2
                            } else {
                                1
                            };
                        for class in ["root", "nodes"] {
                            assert_eq!(
                                ancestors
                                    .iter()
                                    .filter(|(owner, _)| *owner == class)
                                    .count(),
                                expected_owners,
                                "{id}/{class}: {context}"
                            );
                        }
                        let face = body
                            .children()
                            .find(|node| node.has_tag_name("rect"))
                            .unwrap();
                        let mask = shadow
                            .children()
                            .find(|node| node.has_tag_name("rect"))
                            .unwrap();
                        for attribute in ["class", "style", "x", "y", "width", "height", "rx", "ry"]
                        {
                            assert_eq!(
                                mask.attribute(attribute),
                                face.attribute(attribute),
                                "{id}/{attribute}: {context}"
                            );
                        }
                        assert!(face.attribute("filter").is_none(), "{id}: {context}");
                        assert!(mask.attribute("filter").is_some(), "{id}: {context}");
                    }
                }
            }
        }
    }
}

#[test]
fn negative_child_depth_stays_behind_composite_labels_across_nested_roots() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 0.0, -40.0, None);
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html in [false, true] {
            for source in [
                "stateDiagram-v2\nstate Review {\nA --> B\n}\n",
                "stateDiagram-v2\nstate Review {\nstate Inner {\nA --> B\n}\n}\nReview --> Done: finished\n",
            ] {
                let (baseline, _) = render_depth(
                    source,
                    config(layout, html),
                    None,
                    &SvgDebugOptions::default(),
                    RenderResourcePolicy::default(),
                )
                .unwrap();
                let (svg, _) = render_depth(
                    source,
                    config(layout, html),
                    Some(&theme),
                    &SvgDebugOptions::default(),
                    RenderResourcePolicy::default(),
                )
                .unwrap();
                let context = format!("{layout}/{html}/{source}");
                let baseline_doc = roxmltree::Document::parse(&baseline).unwrap();
                let doc = roxmltree::Document::parse(&svg).unwrap();
                let last_depth = doc
                    .descendants()
                    .filter(|node| node.attribute("data-state-shadow-for").is_some())
                    .map(|node| node.range().end)
                    .max()
                    .unwrap();
                assert!(
                    doc.descendants()
                        .any(|node| node.has_tag_name("feOffset")
                            && node.attribute("dy") == Some("-40")),
                    "{context}"
                );
                let labels: Vec<_> = doc
                    .descendants()
                    .filter(|node| node.attribute("class") == Some("cluster-label"))
                    .collect();
                assert_eq!(
                    labels.len(),
                    if source.contains("Inner") { 2 } else { 1 },
                    "{context}"
                );
                // The -40px shadow crosses Review's/Inner's title band. Every composite
                // label must paint after all depth, not only the depth in its own root.
                for label in labels {
                    assert!(last_depth < label.range().start, "{context}");
                    let owner = label.parent().unwrap().attribute("id").unwrap();
                    let before = baseline_doc
                        .descendants()
                        .find(|node| {
                            node.attribute("class") == Some("cluster-label")
                                && node.parent().and_then(|parent| parent.attribute("id"))
                                    == Some(owner)
                        })
                        .unwrap();
                    assert_eq!(&svg[label.range()], &baseline[before.range()], "{context}");
                }
                for edge in doc
                    .descendants()
                    .filter(|node| node.attribute("data-edge") == Some("true"))
                {
                    assert!(last_depth < edge.range().start, "{context}");
                }
                let surfaces: Vec<_> = doc
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("rect")
                            && matches!(node.attribute("class"), Some("outer" | "inner"))
                    })
                    .collect();
                assert_eq!(
                    surfaces.len(),
                    if source.contains("Inner") { 4 } else { 2 },
                    "{context}"
                );
                for surface in surfaces {
                    assert!(
                        surface
                            .ancestors()
                            .any(|node| node.attribute("data-state-depth") == Some("true")),
                        "{context}"
                    );
                }
                let mut ids = std::collections::BTreeSet::new();
                for id in doc.descendants().filter_map(|node| node.attribute("id")) {
                    assert!(ids.insert(id), "duplicate {id}: {context}");
                }
            }
        }
    }
}

#[test]
fn cascade_keeps_previous_alpha_and_reset_semantics_without_painting_source_twice() {
    for color in [EffectColorSpace::Srgb, EffectColorSpace::LinearRgb] {
        for second in [EffectInput::Previous, EffectInput::SourceGraphic] {
            let theme = depth_theme(EffectComposition::Cascade, color, second, 6.0, false);
            let svg = render_classic_state_svg_from_text_with_theme(
                "stateDiagram-v2\nA --> B\nstyle A fill-opacity:0.5\n",
                &theme,
            );
            let doc = roxmltree::Document::parse(&svg).unwrap();
            for filter in doc.descendants().filter(|n| {
                n.has_tag_name("filter")
                    && n.attribute("id")
                        .is_some_and(|id| id.ends_with("-theme-effect-body-depth"))
            }) {
                assert!(
                    !filter
                        .children()
                        .any(|node| node.has_tag_name("feGaussianBlur"))
                );
                let offset = filter
                    .children()
                    .find(|n| n.attribute("result") == Some("merman-shadow-1-offset"))
                    .unwrap();
                if second == EffectInput::Previous {
                    assert_eq!(offset.attribute("in"), Some("merman-shadow-0-result"));
                    assert_eq!(
                        final_inputs(filter),
                        ["merman-shadow-1-shadow", "merman-shadow-0-shadow"]
                    );
                } else {
                    assert_eq!(offset.attribute("in"), Some("SourceGraphic"));
                    assert_eq!(final_inputs(filter), ["merman-shadow-1-shadow"]);
                }
            }
        }
    }
}

#[test]
fn shadow_visibility_follows_nodes_hidden_prefixes_and_nested_roots() {
    let theme = depth_theme(
        EffectComposition::Cascade,
        EffectColorSpace::LinearRgb,
        EffectInput::Previous,
        6.0,
        false,
    );
    let source = "stateDiagram-v2\n[*] --> Review\nstate Review {\nstate Inner {\nA --> B\n}\n}\nReview --> Done\n";
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for include_nodes in [false, true] {
            for include_edges in [false, true] {
                let debug = SvgDebugOptions {
                    include_nodes,
                    include_edges,
                    ..SvgDebugOptions::default()
                };
                let (svg, _) = render_depth(
                    source,
                    config(layout, false),
                    Some(&theme),
                    &debug,
                    RenderResourcePolicy::default(),
                )
                .unwrap();
                let doc = roxmltree::Document::parse(&svg).unwrap();
                let shadows: Vec<_> = doc
                    .descendants()
                    .filter(|n| n.attribute("data-state-shadow-for").is_some())
                    .collect();
                assert_eq!(shadows.len(), if include_nodes { 3 } else { 0 });
                for shadow in shadows {
                    let id = shadow.attribute("data-state-shadow-for").unwrap();
                    let body = doc
                        .descendants()
                        .find(|n| n.attribute("id") == Some(id))
                        .unwrap();
                    assert_eq!(scene_ancestry(shadow), scene_ancestry(body));
                }
            }
        }
    }
    // Caller-authored models can retain a note without a side even though source syntax cannot.
    // Its owner and generated note-group vertices must remain hidden in both passes.
    let parsed = common::install_parse_compatibility(&theme, classic_engine())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\nA --> B\nnote right of A : hidden owner\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let (metadata, mut model, context) = parsed.into_parts();
    let merman_model::presentation::RenderSemanticModel::State(state) = &mut model else {
        panic!("State model");
    };
    state
        .states
        .get_mut("A")
        .unwrap()
        .note
        .as_mut()
        .unwrap()
        .position = None;
    let hidden = merman_model::presentation::PresentationDiagram::try_from_frozen(
        metadata,
        model,
        context,
        &merman_model::resources::InputResourcePolicy::for_profile(
            merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
        ),
        &merman_model::OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    let artifact = family::prepare(
        &hidden,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .unwrap(),
    )
    .unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = svg.svg();
    let doc = roxmltree::Document::parse(svg).unwrap();
    let shadows: Vec<_> = doc
        .descendants()
        .filter_map(|n| n.attribute("data-state-shadow-for"))
        .collect();
    assert_eq!(shadows.len(), 1);
    assert!(shadows[0].contains("state-B-"), "{shadows:?}");
}

#[test]
fn absent_cleared_and_unsupported_effects_do_not_add_depth_layers() {
    let source = "stateDiagram-v2\nA --> B\nstyle A filter:url(#source-filter)\n";
    let clear = depth_theme(
        EffectComposition::Independent,
        EffectColorSpace::Srgb,
        EffectInput::SourceGraphic,
        6.0,
        true,
    );
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html in [false, true] {
            for graph in [
                source,
                "stateDiagram-v2\nstate Review {\nstate Inner {\nA --> B\n}\n}\nReview --> Done\nstyle A filter:url(#source-filter)\n",
            ] {
                let (baseline, _) = render_depth(
                    graph,
                    config(layout, html),
                    None,
                    &SvgDebugOptions::default(),
                    RenderResourcePolicy::default(),
                )
                .unwrap();
                let (cleared, _) = render_depth(
                    graph,
                    config(layout, html),
                    Some(&clear),
                    &SvgDebugOptions::default(),
                    RenderResourcePolicy::default(),
                )
                .unwrap();
                assert_eq!(baseline, cleared, "{layout}/{html}/{graph}");
                assert!(!cleared.contains("data-state-depth="));
                let doc = roxmltree::Document::parse(&cleared).unwrap();
                for root in doc
                    .descendants()
                    .filter(|node| node.attribute("class") == Some("root"))
                {
                    assert_eq!(
                        root.children()
                            .filter(roxmltree::Node::is_element)
                            .map(|node| node.attribute("class").unwrap())
                            .collect::<Vec<_>>(),
                        [
                            "clusters",
                            if layout == "elk" {
                                "edges edgePaths"
                            } else {
                                "edgePaths"
                            },
                            "edgeLabels",
                            "nodes"
                        ],
                    );
                }
            }
        }
    }
    let theme = depth_theme(
        EffectComposition::Cascade,
        EffectColorSpace::Srgb,
        EffectInput::Previous,
        6.0,
        false,
    );
    let (with_source_filter, _) = render_depth(
        source,
        config("dagre", false),
        Some(&theme),
        &SvgDebugOptions::default(),
        RenderResourcePolicy::default(),
    )
    .unwrap();
    let (without_source_filter, _) = render_depth(
        "stateDiagram-v2\nA --> B\n",
        config("dagre", false),
        Some(&theme),
        &SvgDebugOptions::default(),
        RenderResourcePolicy::default(),
    )
    .unwrap();
    assert_eq!(with_source_filter, without_source_filter);

    use merman_render::diagram_theme::{ThemeAdmissionPolicy, ThemeCapability};
    let disabled = RenderEnvironment::deterministic().with_theme_admission_policy(
        ThemeAdmissionPolicy::permissive().with_allowed_capabilities(
            ThemeCapability::ALL
                .iter()
                .copied()
                .filter(|capability| *capability != ThemeCapability::SvgFilter),
        ),
    );
    assert!(matches!(
        disabled.begin_session_with_theme(&theme),
        Err(
            merman_render::environment::RenderEnvironmentError::ThemeAdmission(
                merman_render::diagram_theme::ThemeAdmissionError::ThemeCapabilityDenied(
                    ThemeCapability::SvgFilter
                ),
            )
        ),
    ));
    for look in ["neo", "handDrawn"] {
        let (svg, _) = render_depth(
            source,
            serde_json::json!({"look":look,"layout":"dagre","htmlLabels":false,"handDrawnSeed":1}),
            Some(&theme),
            &SvgDebugOptions::default(),
            RenderResourcePolicy::default(),
        )
        .unwrap();
        assert!(!svg.contains("data-state-depth="));
        assert!(!svg.contains("theme-effect-body-depth"));
    }
}

#[test]
fn depth_output_obeys_exact_byte_and_work_budgets() {
    let theme = depth_theme(
        EffectComposition::Independent,
        EffectColorSpace::Srgb,
        EffectInput::SourceGraphic,
        6.0,
        false,
    );
    let source = "stateDiagram-v2\nA --> B: next\nB --> A: needs detail\n";
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let (expected, work) = render_depth(
        source,
        config("dagre", false),
        Some(&theme),
        &SvgDebugOptions::default(),
        policy,
    )
    .unwrap();
    for (limit, exact) in [
        (ResourceLimitId::MaxSvgBytes, expected.len()),
        (ResourceLimitId::MaxLayoutWorkUnits, work),
    ] {
        let (svg, used) = render_depth(
            source,
            config("dagre", false),
            Some(&theme),
            &SvgDebugOptions::default(),
            policy.with_limit(limit, exact).unwrap(),
        )
        .unwrap();
        assert_eq!(svg, expected);
        assert_eq!(used, work);
        let error = render_depth(
            source,
            config("dagre", false),
            Some(&theme),
            &SvgDebugOptions::default(),
            policy.with_limit(limit, exact - 1).unwrap(),
        )
        .unwrap_err();
        let merman_render::Error::ResourceLimitExceeded(error) = error else {
            panic!("{error}")
        };
        assert_eq!(error.limit, limit.as_str());
        assert!(error.actual > error.max);
    }
}
