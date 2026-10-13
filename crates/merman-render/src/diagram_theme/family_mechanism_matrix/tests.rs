use super::*;

#[derive(Debug, Clone, Copy)]
enum PaintChannel {
    Fill,
    Stroke,
}
use crate::diagram_theme::{
    TextStylePatch, ThemeColorValue, ThemeGeometryPatch, ThemePaintPatch, ThemeStylePatch,
};

#[test]
fn plot_kind_variants_are_confined_to_xychart_series_terminals() {
    for variant in [ThemeVariant::Bar, ThemeVariant::Line] {
        for selector in [
            FamilyThemeSelectorShape::Static {
                variant: Some(variant),
            },
            FamilyThemeSelectorShape::Ordinal {
                variant: Some(variant),
                selector: OrdinalSelector::Exact(2),
            },
            FamilyThemeSelectorShape::Ordinal {
                variant: Some(variant),
                selector: OrdinalSelector::Cycle {
                    period: 3,
                    offset: 2,
                },
            },
        ] {
            for &family in DiagramFamilyId::all() {
                for &target in ThemeTarget::ALL {
                    for facet in [
                        FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
                        FamilyThemeRuleFacet::StrokeWidth,
                        FamilyThemeRuleFacet::Effect,
                    ] {
                        let expected = if family == DiagramFamilyId::XY_CHART
                            && target == ThemeTarget::ChartSeries
                        {
                            FamilyThemeDisposition::TypedAdapter
                        } else {
                            FamilyThemeDisposition::Unsupported
                        };
                        assert_eq!(
                            classify_rule_facet(family, target, selector, facet),
                            expected,
                            "{family:?}/{target:?}/{selector:?}/{facet:?}"
                        );
                    }
                }
            }
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::XY_CHART,
                    ThemeTarget::ChartSeries,
                    selector,
                    FamilyThemeRuleFacet::StrokeDasharray
                ),
                FamilyThemeDisposition::Unsupported,
            );
        }
    }
}

#[test]
fn selector_ordinal_domain_intersection_handles_exact_and_cycle_boundaries() {
    let static_selector = FamilyThemeSelectorShape::Static { variant: None };
    let exact = FamilyThemeSelectorShape::Ordinal {
        variant: Some(ThemeVariant::Warning),
        selector: OrdinalSelector::exact(3).expect("valid exact selector"),
    };
    let cycle = FamilyThemeSelectorShape::Ordinal {
        variant: None,
        selector: OrdinalSelector::cycle(5, 2).expect("valid cycle selector"),
    };

    assert!(!static_selector.ordinal_domain_intersects_occurrence_count(0));
    assert!(static_selector.ordinal_domain_intersects_occurrence_count(1));
    assert!(!exact.ordinal_domain_intersects_occurrence_count(2));
    assert!(exact.ordinal_domain_intersects_occurrence_count(3));
    assert!(!cycle.ordinal_domain_intersects_occurrence_count(2));
    assert!(cycle.ordinal_domain_intersects_occurrence_count(3));
}

#[test]
fn pie_slice_palette_uses_the_direct_adapter() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::PIE, ThemeTarget::PieSlice).disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn pie_and_block_own_static_scalar_fill() {
    for (family, target) in [
        (DiagramFamilyId::PIE, ThemeTarget::PieSlice),
        (DiagramFamilyId::BLOCK, ThemeTarget::Node),
    ] {
        for kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            let expected_default = FamilyThemeDisposition::TypedAdapter;
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Fill(kind),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "{family} {target:?} {kind:?}"
            );
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Fill(kind),
                ),
                expected_default,
                "{family} {target:?} {kind:?}"
            );
        }

        for kind in [
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Fill(kind),
                ),
                FamilyThemeDisposition::Unsupported,
                "{family} {target:?} {kind:?}"
            );
        }
    }
}

#[test]
fn direct_only_family_paint_slices_are_static_and_scalar() {
    for (family, target, channel) in [
        (
            DiagramFamilyId::ARCHITECTURE,
            ThemeTarget::Node,
            PaintChannel::Fill,
        ),
        (
            DiagramFamilyId::ARCHITECTURE,
            ThemeTarget::Node,
            PaintChannel::Stroke,
        ),
        (
            DiagramFamilyId::ARCHITECTURE,
            ThemeTarget::Edge,
            PaintChannel::Stroke,
        ),
        (
            DiagramFamilyId::ARCHITECTURE,
            ThemeTarget::Marker,
            PaintChannel::Fill,
        ),
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::ChartSeries,
            PaintChannel::Fill,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::Cluster,
            PaintChannel::Fill,
        ),
        (
            DiagramFamilyId::C4,
            ThemeTarget::Cluster,
            PaintChannel::Stroke,
        ),
    ] {
        let facet_for = |kind| match channel {
            PaintChannel::Fill => FamilyThemeRuleFacet::Fill(kind),
            PaintChannel::Stroke => FamilyThemeRuleFacet::Stroke(kind),
        };
        for kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    facet_for(kind),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "family={} target={} channel={channel:?} paint_kind={kind:?}",
                family.as_str(),
                target.id(),
            );
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    facet_for(kind),
                ),
                FamilyThemeDisposition::Unsupported,
                "family={} target={} channel={channel:?} paint_kind={kind:?}",
                family.as_str(),
                target.id(),
            );
        }

        for kind in [
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    facet_for(kind),
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
    }
}

#[test]
fn architecture_text_fill_owns_unqualified_and_default_variants() {
    for kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ARCHITECTURE,
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(kind),
            ),
            FamilyThemeDisposition::TypedAdapter,
        );
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ARCHITECTURE,
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Default),
                },
                FamilyThemeRuleFacet::Fill(kind),
            ),
            FamilyThemeDisposition::TypedAdapter,
        );
    }
}

#[test]
fn mindmap_node_palette_uses_the_direct_adapter() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::MINDMAP, ThemeTarget::Node).disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn mindmap_terminal_less_paint_routes_are_unsupported() {
    for (target, channel) in [
        (ThemeTarget::NodeLabel, PaintChannel::Fill),
        (ThemeTarget::Text, PaintChannel::Fill),
        (ThemeTarget::Title, PaintChannel::Fill),
        (ThemeTarget::Marker, PaintChannel::Fill),
        (ThemeTarget::Marker, PaintChannel::Stroke),
        (ThemeTarget::EdgeLabelBackground, PaintChannel::Fill),
        (ThemeTarget::Cluster, PaintChannel::Fill),
        (ThemeTarget::Cluster, PaintChannel::Stroke),
        (ThemeTarget::ClusterLabel, PaintChannel::Fill),
    ] {
        for paint_kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            let facet = match channel {
                PaintChannel::Fill => FamilyThemeRuleFacet::Fill(paint_kind),
                PaintChannel::Stroke => FamilyThemeRuleFacet::Stroke(paint_kind),
            };
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::MINDMAP,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    facet,
                ),
                FamilyThemeDisposition::Unsupported,
                "target={} channel={channel:?} paint_kind={paint_kind:?}",
                target.id()
            );
        }
    }

    for (selector, facet) in [
        (
            FamilyThemeSelectorShape::Static { variant: None },
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
        ),
        (
            FamilyThemeSelectorShape::Static {
                variant: Some(ThemeVariant::Default),
            },
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
        ),
        (
            FamilyThemeSelectorShape::Static {
                variant: Some(ThemeVariant::Default),
            },
            FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Solid),
        ),
    ] {
        assert_eq!(
            classify_rule_facet(DiagramFamilyId::MINDMAP, ThemeTarget::Edge, selector, facet,),
            FamilyThemeDisposition::Unsupported,
            "Mindmap Edge fill has no terminal consumer",
        );
    }
}

#[test]
fn tree_view_terminal_less_paint_routes_are_unsupported() {
    for (target, channel) in [
        (ThemeTarget::Node, PaintChannel::Fill),
        (ThemeTarget::Node, PaintChannel::Stroke),
        (ThemeTarget::Title, PaintChannel::Fill),
        (ThemeTarget::EdgeLabelBackground, PaintChannel::Fill),
        (ThemeTarget::Cluster, PaintChannel::Fill),
        (ThemeTarget::Cluster, PaintChannel::Stroke),
        (ThemeTarget::ClusterLabel, PaintChannel::Fill),
    ] {
        for paint_kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            let facet = match channel {
                PaintChannel::Fill => FamilyThemeRuleFacet::Fill(paint_kind),
                PaintChannel::Stroke => FamilyThemeRuleFacet::Stroke(paint_kind),
            };
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::TREE_VIEW,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    facet,
                ),
                FamilyThemeDisposition::Unsupported,
                "target={} channel={channel:?} paint_kind={paint_kind:?}",
                target.id()
            );
        }
    }
}

#[test]
fn gitgraph_terminal_less_paint_routes_are_unsupported() {
    for (target, channel) in [
        (ThemeTarget::Title, PaintChannel::Fill),
        (ThemeTarget::Marker, PaintChannel::Fill),
        (ThemeTarget::Marker, PaintChannel::Stroke),
        (ThemeTarget::Cluster, PaintChannel::Fill),
        (ThemeTarget::Cluster, PaintChannel::Stroke),
        (ThemeTarget::ClusterLabel, PaintChannel::Fill),
    ] {
        for paint_kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            let facet = match channel {
                PaintChannel::Fill => FamilyThemeRuleFacet::Fill(paint_kind),
                PaintChannel::Stroke => FamilyThemeRuleFacet::Stroke(paint_kind),
            };
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::GIT_GRAPH,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    facet,
                ),
                FamilyThemeDisposition::Unsupported,
                "target={} channel={channel:?} paint_kind={paint_kind:?}",
                target.id()
            );
        }
    }
}

#[test]
fn mindmap_owns_static_node_paint_and_unqualified_edge_stroke() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            for facet in [
                FamilyThemeRuleFacet::Fill(paint_kind),
                FamilyThemeRuleFacet::Stroke(paint_kind),
            ] {
                assert_eq!(
                    classify_rule_facet(
                        DiagramFamilyId::MINDMAP,
                        ThemeTarget::Node,
                        FamilyThemeSelectorShape::Static { variant },
                        facet,
                    ),
                    FamilyThemeDisposition::TypedAdapter,
                    "variant={variant:?} facet={facet:?}"
                );
            }
        }
    }
    for family in [DiagramFamilyId::MINDMAP, DiagramFamilyId::GIT_GRAPH] {
        for paint_kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            let expected_qualified = if family == DiagramFamilyId::MINDMAP {
                FamilyThemeDisposition::Unsupported
            } else {
                FamilyThemeDisposition::TypedAdapter
            };
            assert_eq!(
                classify_rule_facet(
                    family,
                    ThemeTarget::Edge,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Stroke(paint_kind),
                ),
                FamilyThemeDisposition::TypedAdapter
            );
            assert_eq!(
                classify_rule_facet(
                    family,
                    ThemeTarget::Edge,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Stroke(paint_kind),
                ),
                expected_qualified
            );
        }

        for paint_kind in [
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            assert_eq!(
                classify_rule_facet(
                    family,
                    ThemeTarget::Edge,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Stroke(paint_kind),
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
    }
}

#[test]
fn gitgraph_node_palette_uses_the_direct_adapter() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::GIT_GRAPH, ThemeTarget::Node).disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn sankey_node_palette_is_direct_only() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::SANKEY, ThemeTarget::Node).disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn pie_and_block_own_static_scalar_stroke() {
    for (family, target) in [
        (DiagramFamilyId::PIE, ThemeTarget::PieSlice),
        (DiagramFamilyId::BLOCK, ThemeTarget::Node),
    ] {
        for kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            let expected_default = FamilyThemeDisposition::TypedAdapter;
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Stroke(kind),
                ),
                FamilyThemeDisposition::TypedAdapter
            );
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Stroke(kind),
                ),
                expected_default
            );
        }

        for kind in [
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Stroke(kind),
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
    }
}

#[test]
fn er_entity_scalar_paint_is_direct_for_static_default_selectors_only() {
    for selector in [
        FamilyThemeSelectorShape::Static { variant: None },
        FamilyThemeSelectorShape::Static {
            variant: Some(ThemeVariant::Default),
        },
    ] {
        for facet in [
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent),
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Transparent),
            FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Solid),
        ] {
            assert_eq!(
                classify_rule_facet(DiagramFamilyId::ER, ThemeTarget::Entity, selector, facet,),
                FamilyThemeDisposition::TypedAdapter
            );
        }
    }

    let ordinal = FamilyThemeSelectorShape::Ordinal {
        variant: None,
        selector: OrdinalSelector::exact(1).expect("valid ER entity ordinal"),
    };
    assert_eq!(
        classify_rule_facet(
            DiagramFamilyId::ER,
            ThemeTarget::Entity,
            ordinal,
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
        ),
        FamilyThemeDisposition::Unsupported
    );
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ER,
                ThemeTarget::Relation,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Stroke(paint_kind),
            ),
            FamilyThemeDisposition::TypedAdapter
        );
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ER,
                ThemeTarget::Relation,
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Default),
                },
                FamilyThemeRuleFacet::Stroke(paint_kind),
            ),
            FamilyThemeDisposition::TypedAdapter
        );
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ER,
                ThemeTarget::Relation,
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Default),
                },
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
            FamilyThemeDisposition::TypedAdapter
        );

        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ER,
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
            FamilyThemeDisposition::TypedAdapter
        );
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ER,
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Default),
                },
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
            FamilyThemeDisposition::TypedAdapter
        );

        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::ER,
                ThemeTarget::Table,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
            FamilyThemeDisposition::TypedAdapter
        );
        for variant in [ThemeVariant::Odd, ThemeVariant::Even] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::ER,
                    ThemeTarget::Table,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(variant),
                    },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "variant={variant:?}"
            );
        }
    }
}

#[test]
fn mixed_flowchart_rule_is_split_by_facet() {
    let rule = ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch {
            paint: ThemePaintPatch {
                fill: Specified::Value(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
                ..ThemePaintPatch::default()
            },
            geometry: ThemeGeometryPatch {
                radius: Specified::Value(8.0),
            },
            ..ThemeStylePatch::default()
        },
    );
    let routes = compile_rule_routes(DiagramFamilyId::FLOWCHART, 7, &rule);

    assert_eq!(routes.len(), 2);
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::RuleFacet {
                rule_index: 7,
                target: ThemeTarget::Node,
                selector: FamilyThemeSelectorShape::Static { variant: None },
                facet: FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            }
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        matches!(
            route.mechanism(),
            FamilyThemeMechanism::RuleFacet {
                facet: FamilyThemeRuleFacet::Radius,
                ..
            }
        ) && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
}

#[test]
fn flowchart_and_swimlane_ordinal_node_fills_keep_their_paint_boundary() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for ordinal in [
            OrdinalSelector::Exact(2),
            OrdinalSelector::Cycle {
                period: 2,
                offset: 1,
            },
        ] {
            for variant in [
                None,
                Some(ThemeVariant::Default),
                Some(ThemeVariant::Active),
            ] {
                for paint in FamilyThemePaintKind::ALL {
                    let selector = FamilyThemeSelectorShape::Ordinal {
                        variant,
                        selector: ordinal,
                    };
                    let expected = if variant != Some(ThemeVariant::Active)
                        && matches!(
                            paint,
                            FamilyThemePaintKind::Solid | FamilyThemePaintKind::Transparent
                        ) {
                        FamilyThemeDisposition::TypedAdapter
                    } else {
                        FamilyThemeDisposition::Unsupported
                    };
                    assert_eq!(
                        classify_rule_facet(
                            family,
                            ThemeTarget::Node,
                            selector,
                            FamilyThemeRuleFacet::Fill(paint)
                        ),
                        expected,
                        "{family} {selector:?} {paint:?}"
                    );
                    assert_eq!(
                        classify_rule_facet(
                            family,
                            ThemeTarget::Node,
                            selector,
                            FamilyThemeRuleFacet::Stroke(paint)
                        ),
                        FamilyThemeDisposition::Unsupported,
                        "ordinal stroke admission must remain unchanged"
                    );
                }
            }
        }
    }
}

#[test]
fn flowchart_and_swimlane_own_static_default_node_paint() {
    let default_style = ThemeStylePatch {
        geometry: ThemeGeometryPatch {
            radius: Specified::Value(8.0),
        },
        ..ThemeStylePatch::default()
    }
    .with_fill(CanvasPaint::Transparent)
    .with_stroke(CanvasPaint::solid("#abcdef").expect("valid fixture color"))
    .with_stroke_width(2.5)
    .expect("valid fixture stroke width")
    .with_stroke_dasharray([4.0, 2.0])
    .expect("valid fixture stroke dasharray");
    let default_rule =
        ThemeRule::new(ThemeTarget::Node, default_style).with_variant(ThemeVariant::Default);
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        let routes = compile_rule_routes(family, 0, &default_rule);
        for route in routes {
            let expected = match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    facet: FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_),
                    ..
                } => FamilyThemeDisposition::TypedAdapter,
                FamilyThemeMechanism::RuleFacet {
                    facet:
                        FamilyThemeRuleFacet::StrokeWidth
                        | FamilyThemeRuleFacet::StrokeDasharray
                        | FamilyThemeRuleFacet::Radius,
                    ..
                } => FamilyThemeDisposition::TypedAdapter,
                mechanism => panic!("unexpected route {mechanism:?}"),
            };
            assert_eq!(route.disposition(), expected);
        }
        assert_eq!(
            compile_ordinal_palette_route(family, ThemeTarget::Node).disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
        assert_eq!(
            compile_ordinal_palette_route(family, ThemeTarget::Edge).disposition(),
            FamilyThemeDisposition::Unsupported
        );
    }

    let active_rule = ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
    )
    .with_variant(ThemeVariant::Active);
    assert_eq!(
        compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &active_rule)[0].disposition(),
        FamilyThemeDisposition::Unsupported
    );

    let ordinal_geometry_rule = ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default()
            .with_stroke_width(2.5)
            .expect("valid ordinal width")
            .with_stroke_dasharray([4.0, 2.0])
            .expect("valid ordinal dasharray"),
    )
    .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap());
    assert!(
        compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &ordinal_geometry_rule)
            .iter()
            .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
    );

    for rule in [
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(8.0),
                },
                ..ThemeStylePatch::default()
            },
        )
        .with_variant(ThemeVariant::Active),
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(8.0),
                },
                ..ThemeStylePatch::default()
            },
        )
        .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch {
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(8.0),
                },
                ..ThemeStylePatch::default()
            },
        ),
    ] {
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &rule)[0].disposition(),
            FamilyThemeDisposition::Unsupported
        );
    }
}

#[test]
fn flowchart_and_swimlane_own_static_default_edge_width_and_dasharray() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut edge_geometry_rule = ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default()
                    .with_stroke_width(2.5)
                    .expect("valid edge width")
                    .with_stroke_dasharray([4.0, 2.0])
                    .expect("valid edge dasharray"),
            );
            if let Some(variant) = variant {
                edge_geometry_rule = edge_geometry_rule.with_variant(variant);
            }
            let routes = compile_rule_routes(family, 0, &edge_geometry_rule);
            assert_eq!(routes.len(), 2);
            assert!(routes.iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        facet: FamilyThemeRuleFacet::StrokeWidth,
                        ..
                    }
                ) && route.disposition() == FamilyThemeDisposition::TypedAdapter
            }));
            assert!(routes.iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        facet: FamilyThemeRuleFacet::StrokeDasharray,
                        ..
                    }
                ) && route.disposition() == FamilyThemeDisposition::TypedAdapter
            }));
        }

        for edge_geometry_rule in [
            ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default()
                    .with_stroke_width(2.5)
                    .expect("valid active edge width")
                    .with_stroke_dasharray([4.0, 2.0])
                    .expect("valid active edge dasharray"),
            )
            .with_variant(ThemeVariant::Active),
            ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default()
                    .with_stroke_width(2.5)
                    .expect("valid ordinal edge width")
                    .with_stroke_dasharray([4.0, 2.0])
                    .expect("valid ordinal edge dasharray"),
            )
            .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
        ] {
            assert!(
                compile_rule_routes(family, 0, &edge_geometry_rule)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
            );
        }
    }
}

#[test]
fn class_owns_static_edge_and_node_paints_and_default_qualified_node_stroke_width() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for (target, facet) in [
            (ThemeTarget::Edge, FamilyThemeRuleFacet::Fill(paint_kind)),
            (ThemeTarget::Edge, FamilyThemeRuleFacet::Stroke(paint_kind)),
            (ThemeTarget::Node, FamilyThemeRuleFacet::Fill(paint_kind)),
            (ThemeTarget::Node, FamilyThemeRuleFacet::Stroke(paint_kind)),
            (
                ThemeTarget::NodeLabel,
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
        ] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::CLASS,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    facet,
                ),
                FamilyThemeDisposition::TypedAdapter,
                "target={target:?} facet={facet:?}"
            );
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::CLASS,
                    target,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    facet,
                ),
                FamilyThemeDisposition::TypedAdapter,
                "qualified target={target:?} facet={facet:?}"
            );
        }
    }

    for variant in [None, Some(ThemeVariant::Default)] {
        let mut rule = ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke_width(2.5)
                .expect("valid Class relation width"),
        );
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }

        let routes = compile_rule_routes(DiagramFamilyId::CLASS, 0, &rule);
        assert_eq!(routes.len(), 1);
        assert_eq!(
            routes[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
    }

    for rule in [
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke_width(2.5)
                .expect("valid active Class relation width"),
        )
        .with_variant(ThemeVariant::Active),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke_width(2.5)
                .expect("valid ordinal Class relation width"),
        )
        .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke_dasharray([4.0, 2.0])
                .expect("valid Class relation dasharray"),
        ),
    ] {
        assert!(
            compile_rule_routes(DiagramFamilyId::CLASS, 0, &rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );
    }
}

#[test]
fn class_terminal_less_paint_routes_are_unsupported() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for (target, facet) in [
            (ThemeTarget::Marker, FamilyThemeRuleFacet::Fill(paint_kind)),
            (
                ThemeTarget::Marker,
                FamilyThemeRuleFacet::Stroke(paint_kind),
            ),
            (
                ThemeTarget::ClusterLabel,
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                assert_eq!(
                    classify_rule_facet(
                        DiagramFamilyId::CLASS,
                        target,
                        FamilyThemeSelectorShape::Static { variant },
                        facet,
                    ),
                    FamilyThemeDisposition::Unsupported,
                    "target={target:?} facet={facet:?} variant={variant:?}"
                );
            }
        }

        for variant in [None, Some(ThemeVariant::Default)] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::CLASS,
                    ThemeTarget::Title,
                    FamilyThemeSelectorShape::Static { variant },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "Class Title.fill directly owns visible namespace labels",
            );
        }

        for variant in [None, Some(ThemeVariant::Odd), Some(ThemeVariant::Even)] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::CLASS,
                    ThemeTarget::Table,
                    FamilyThemeSelectorShape::Static { variant },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                FamilyThemeDisposition::Unsupported,
                "target=Table facet=Fill({paint_kind:?}) variant={variant:?}"
            );
        }
    }
}

#[test]
fn requirement_owns_unqualified_and_default_static_scalar_paint() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for facet in [
            FamilyThemeRuleFacet::Fill(paint_kind),
            FamilyThemeRuleFacet::Stroke(paint_kind),
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                assert_eq!(
                    classify_rule_facet(
                        DiagramFamilyId::REQUIREMENT,
                        ThemeTarget::Requirement,
                        FamilyThemeSelectorShape::Static { variant },
                        facet,
                    ),
                    FamilyThemeDisposition::TypedAdapter,
                    "facet={facet:?} variant={variant:?}"
                );
            }

            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::REQUIREMENT,
                    ThemeTarget::Requirement,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Active),
                    },
                    facet,
                ),
                FamilyThemeDisposition::Unsupported,
                "facet={facet:?}"
            );
        }
    }

    for variant in [
        None,
        Some(ThemeVariant::Default),
        Some(ThemeVariant::Odd),
        Some(ThemeVariant::Even),
    ] {
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::REQUIREMENT,
                ThemeTarget::Table,
                FamilyThemeSelectorShape::Static { variant },
                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            ),
            FamilyThemeDisposition::Unsupported,
            "Requirement table fill must remain unsupported: variant={variant:?}"
        );
    }
}

#[test]
fn gantt_and_kanban_own_their_exact_static_task_surfaces() {
    let task_radius = ThemeStylePatch {
        geometry: ThemeGeometryPatch {
            radius: Specified::Value(7.0),
        },
        ..ThemeStylePatch::default()
    };
    for family in [DiagramFamilyId::GANTT, DiagramFamilyId::KANBAN] {
        let unqualified = ThemeRule::new(ThemeTarget::Task, task_radius.clone());
        let routes = compile_rule_routes(family, 0, &unqualified);
        assert_eq!(routes.len(), 1);
        assert_eq!(
            routes[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );

        for rule in [
            ThemeRule::new(ThemeTarget::Task, task_radius.clone())
                .with_variant(ThemeVariant::Default),
            ThemeRule::new(ThemeTarget::Task, task_radius.clone())
                .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
        ] {
            assert!(
                compile_rule_routes(family, 0, &rule)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
            );
        }
    }

    let task_fill = ThemeStylePatch::default()
        .with_fill(CanvasPaint::solid("#2563eb").expect("valid task fill"));
    assert_eq!(
        compile_rule_routes(
            DiagramFamilyId::GANTT,
            0,
            &ThemeRule::new(ThemeTarget::Task, task_fill.clone()),
        )[0]
        .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
    assert_eq!(
        compile_rule_routes(
            DiagramFamilyId::KANBAN,
            0,
            &ThemeRule::new(ThemeTarget::Task, task_fill.clone()),
        )[0]
        .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
    for variant in [
        ThemeVariant::Default,
        ThemeVariant::Active,
        ThemeVariant::Success,
        ThemeVariant::Error,
    ] {
        assert_eq!(
            compile_rule_routes(
                DiagramFamilyId::KANBAN,
                0,
                &ThemeRule::new(ThemeTarget::Task, task_fill.clone()).with_variant(variant),
            )[0]
            .disposition(),
            FamilyThemeDisposition::Unsupported
        );
    }
    for variant in [
        ThemeVariant::Default,
        ThemeVariant::Active,
        ThemeVariant::Success,
        ThemeVariant::Error,
    ] {
        assert_eq!(
            compile_rule_routes(
                DiagramFamilyId::GANTT,
                0,
                &ThemeRule::new(ThemeTarget::Task, task_fill.clone()).with_variant(variant),
            )[0]
            .disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
    }
    assert_eq!(
        compile_rule_routes(
            DiagramFamilyId::GANTT,
            0,
            &ThemeRule::new(ThemeTarget::Task, task_fill.clone())
                .with_variant(ThemeVariant::Warning),
        )[0]
        .disposition(),
        FamilyThemeDisposition::Unsupported
    );
    assert_eq!(
        compile_rule_routes(
            DiagramFamilyId::GANTT,
            0,
            &ThemeRule::new(ThemeTarget::Task, task_fill)
                .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
        )[0]
        .disposition(),
        FamilyThemeDisposition::Unsupported
    );

    let task_stroke = ThemeStylePatch::default()
        .with_stroke(CanvasPaint::solid("#334455").expect("valid task stroke"));
    assert_eq!(
        compile_rule_routes(
            DiagramFamilyId::GANTT,
            0,
            &ThemeRule::new(ThemeTarget::Task, task_stroke.clone()),
        )[0]
        .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
    assert_eq!(
        compile_rule_routes(
            DiagramFamilyId::KANBAN,
            0,
            &ThemeRule::new(ThemeTarget::Task, task_stroke.clone()),
        )[0]
        .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
    for variant in [
        ThemeVariant::Default,
        ThemeVariant::Active,
        ThemeVariant::Success,
        ThemeVariant::Error,
    ] {
        assert_eq!(
            compile_rule_routes(
                DiagramFamilyId::GANTT,
                0,
                &ThemeRule::new(ThemeTarget::Task, task_stroke.clone()).with_variant(variant),
            )[0]
            .disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
        assert_eq!(
            compile_rule_routes(
                DiagramFamilyId::KANBAN,
                0,
                &ThemeRule::new(ThemeTarget::Task, task_stroke.clone()).with_variant(variant),
            )[0]
            .disposition(),
            FamilyThemeDisposition::Unsupported
        );
    }
    assert_eq!(
        compile_rule_routes(
            DiagramFamilyId::GANTT,
            0,
            &ThemeRule::new(ThemeTarget::Task, task_stroke).with_variant(ThemeVariant::Warning),
        )[0]
        .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn gitgraph_edge_text_and_label_fill_cutovers_are_static_scalar_and_role_local() {
    for target in [
        ThemeTarget::Text,
        ThemeTarget::NodeLabel,
        ThemeTarget::EdgeLabel,
        ThemeTarget::Edge,
    ] {
        for kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                let facet = FamilyThemeRuleFacet::Fill(kind);
                assert_eq!(
                    classify_rule_facet(
                        DiagramFamilyId::GIT_GRAPH,
                        target,
                        FamilyThemeSelectorShape::Static { variant },
                        facet,
                    ),
                    FamilyThemeDisposition::TypedAdapter
                );
                assert_eq!(
                    classify_rule_facet(
                        DiagramFamilyId::GIT_GRAPH,
                        target,
                        FamilyThemeSelectorShape::Ordinal {
                            variant,
                            selector: OrdinalSelector::exact(1).expect("valid ordinal"),
                        },
                        facet,
                    ),
                    FamilyThemeDisposition::Unsupported
                );
            }
        }
        for kind in [
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::GIT_GRAPH,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Fill(kind),
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::GIT_GRAPH,
                target,
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Active),
                },
                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            ),
            FamilyThemeDisposition::Unsupported
        );
    }
}

#[test]
fn kanban_text_fill_cutover_is_static_scalar_and_family_local() {
    for kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let selector = FamilyThemeSelectorShape::Static { variant };
            let facet = FamilyThemeRuleFacet::Fill(kind);
            assert_eq!(
                classify_rule_facet(DiagramFamilyId::KANBAN, ThemeTarget::Text, selector, facet),
                FamilyThemeDisposition::TypedAdapter
            );
            assert_eq!(
                classify_rule_facet(DiagramFamilyId::CLASS, ThemeTarget::Text, selector, facet),
                FamilyThemeDisposition::TypedAdapter
            );
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::KANBAN,
                    ThemeTarget::Text,
                    FamilyThemeSelectorShape::Ordinal {
                        variant,
                        selector: OrdinalSelector::exact(1).expect("valid ordinal"),
                    },
                    facet,
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
    }
    for kind in [
        FamilyThemePaintKind::Clear,
        FamilyThemePaintKind::LinearGradient,
        FamilyThemePaintKind::RadialGradient,
        FamilyThemePaintKind::Pattern,
    ] {
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::KANBAN,
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(kind),
            ),
            FamilyThemeDisposition::Unsupported
        );
    }
    assert_eq!(
        classify_rule_facet(
            DiagramFamilyId::KANBAN,
            ThemeTarget::Text,
            FamilyThemeSelectorShape::Static {
                variant: Some(ThemeVariant::Active)
            },
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
        ),
        FamilyThemeDisposition::Unsupported
    );
}

#[test]
fn kanban_owns_the_task_ordinal_palette() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::KANBAN, ThemeTarget::Task).disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn terminal_less_title_fill_is_unsupported_for_static_and_dynamic_selectors() {
    for family in [DiagramFamilyId::JOURNEY, DiagramFamilyId::KANBAN] {
        for paint in [
            CanvasPaint::Transparent,
            CanvasPaint::solid("#9333ea").expect("valid title paint"),
        ] {
            for variant in [
                None,
                Some(ThemeVariant::Default),
                Some(ThemeVariant::Active),
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(paint.clone()),
                )
                .for_family(family);
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                for ordinal in [
                    None,
                    Some(OrdinalSelector::exact(1).expect("valid exact selector")),
                    Some(OrdinalSelector::cycle(2, 0).expect("valid cycle selector")),
                ] {
                    let rule = match ordinal {
                        Some(ordinal) => rule.clone().with_ordinal(ordinal),
                        None => rule.clone(),
                    };
                    let routes = compile_rule_routes(family, 0, &rule);
                    assert_eq!(routes.len(), 1);
                    assert_eq!(
                        routes[0].disposition(),
                        FamilyThemeDisposition::Unsupported,
                        "family={family}, variant={variant:?}, ordinal={ordinal:?}, paint={paint:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn journey_owns_the_task_ordinal_palette() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::JOURNEY, ThemeTarget::JourneyTask)
            .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn journey_owns_unqualified_and_default_task_scalar_paints() {
    for paint in [
        CanvasPaint::Transparent,
        CanvasPaint::solid("#123456").expect("valid Journey task paint"),
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut rule = ThemeRule::new(
                ThemeTarget::JourneyTask,
                ThemeStylePatch::default()
                    .with_fill(paint.clone())
                    .with_stroke(paint.clone()),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            assert!(
                compile_rule_routes(DiagramFamilyId::JOURNEY, 0, &rule)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter),
                "variant={variant:?}"
            );
        }
    }
}

#[test]
fn timeline_owns_the_event_ordinal_palette() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::TIMELINE, ThemeTarget::TimelineEvent)
            .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn kanban_owns_family_qualified_ordinal_task_label_foregrounds() {
    let rule = ThemeRule::new(
        ThemeTarget::TaskLabel,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#000000").expect("valid label foreground")),
    )
    .for_family(DiagramFamilyId::KANBAN)
    .with_ordinal(crate::diagram_theme::OrdinalSelector::cycle(12, 0).unwrap());
    let routes = compile_rule_routes(DiagramFamilyId::KANBAN, 0, &rule);

    assert_eq!(routes.len(), 1);
    assert_eq!(
        routes[0].disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn xychart_owns_the_chart_series_ordinal_palette() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::XY_CHART, ThemeTarget::ChartSeries)
            .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn radar_owns_the_chart_series_ordinal_palette() {
    assert_eq!(
        compile_ordinal_palette_route(DiagramFamilyId::RADAR, ThemeTarget::ChartSeries)
            .disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn radar_title_fill_is_typed_for_unqualified_and_default() {
    let solid = FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid);
    let unqualified = FamilyThemeSelectorShape::Static { variant: None };
    let default = FamilyThemeSelectorShape::Static {
        variant: Some(ThemeVariant::Default),
    };

    assert_eq!(
        classify_rule_facet(
            DiagramFamilyId::RADAR,
            ThemeTarget::Title,
            unqualified,
            solid
        ),
        FamilyThemeDisposition::TypedAdapter
    );
    assert_eq!(
        classify_rule_facet(DiagramFamilyId::RADAR, ThemeTarget::Title, default, solid),
        FamilyThemeDisposition::TypedAdapter
    );
}

#[test]
fn timeline_owns_only_unqualified_static_event_geometry_and_opacity() {
    let event_opacity = ThemeStylePatch {
        paint: ThemePaintPatch {
            opacity: Specified::Value(0.42),
            ..ThemePaintPatch::default()
        },
        ..ThemeStylePatch::default()
    };
    let event_radius = ThemeStylePatch {
        geometry: ThemeGeometryPatch {
            radius: Specified::Value(8.0),
        },
        ..ThemeStylePatch::default()
    };
    for style in [event_opacity.clone(), event_radius.clone()] {
        let unqualified = ThemeRule::new(ThemeTarget::TimelineEvent, style);
        let routes = compile_rule_routes(DiagramFamilyId::TIMELINE, 0, &unqualified);
        assert_eq!(routes.len(), 1);
        assert_eq!(
            routes[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
    }

    for rule in [
        ThemeRule::new(ThemeTarget::TimelineEvent, event_opacity.clone())
            .with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::TimelineEvent, event_opacity.clone())
            .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
        ThemeRule::new(ThemeTarget::TimelineEvent, event_radius.clone())
            .with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::TimelineEvent, event_radius)
            .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
    ] {
        assert!(
            compile_rule_routes(DiagramFamilyId::TIMELINE, 0, &rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );
    }
}

#[test]
fn timeline_event_fill_owns_unqualified_and_explicit_default_scalar_routes() {
    let fill = CanvasPaint::solid("#123456").expect("valid Timeline event fill");
    for rule in [
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            ThemeStylePatch::default().with_fill(fill.clone()),
        ),
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            ThemeStylePatch::default().with_fill(fill.clone()),
        )
        .with_variant(ThemeVariant::Default),
    ] {
        let routes = compile_rule_routes(DiagramFamilyId::TIMELINE, 0, &rule);
        assert_eq!(routes.len(), 1);
        assert_eq!(
            routes[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
    }

    for rule in [
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            ThemeStylePatch::default().with_fill(fill.clone()),
        )
        .with_variant(ThemeVariant::Active),
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            ThemeStylePatch::default().with_fill(fill),
        )
        .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
    ] {
        assert!(
            compile_rule_routes(DiagramFamilyId::TIMELINE, 0, &rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );
    }
}

#[test]
fn tree_view_owns_only_unqualified_static_edge_stroke_width() {
    let edge_width = ThemeStylePatch::default()
        .with_stroke_width(6.0)
        .expect("valid Tree View edge width");
    let unqualified = ThemeRule::new(ThemeTarget::Edge, edge_width.clone());
    let routes = compile_rule_routes(DiagramFamilyId::TREE_VIEW, 0, &unqualified);
    assert_eq!(routes.len(), 1);
    assert_eq!(
        routes[0].disposition(),
        FamilyThemeDisposition::TypedAdapter
    );

    for rule in [
        ThemeRule::new(ThemeTarget::Edge, edge_width.clone()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Edge, edge_width)
            .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
    ] {
        assert!(
            compile_rule_routes(DiagramFamilyId::TREE_VIEW, 0, &rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );
    }
}

#[test]
fn tree_view_scalar_paint_owns_unqualified_and_explicit_default_routes() {
    let fill = CanvasPaint::solid("#123456").expect("valid Tree View fill");
    let stroke = CanvasPaint::solid("#654321").expect("valid Tree View stroke");
    for (target, style) in [
        (
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(fill.clone()),
        ),
        (
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(fill.clone()),
        ),
        (
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_fill(fill.clone()),
        ),
        (
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(stroke.clone()),
        ),
        (
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_fill(fill.clone()),
        ),
        (
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_stroke(stroke.clone()),
        ),
    ] {
        let unqualified = compile_rule_routes(
            DiagramFamilyId::TREE_VIEW,
            0,
            &ThemeRule::new(target, style.clone()),
        );
        assert_eq!(unqualified.len(), 1, "target={target:?}");
        assert_eq!(
            unqualified[0].disposition(),
            FamilyThemeDisposition::TypedAdapter,
            "target={target:?}"
        );

        let default = compile_rule_routes(
            DiagramFamilyId::TREE_VIEW,
            0,
            &ThemeRule::new(target, style).with_variant(ThemeVariant::Default),
        );
        assert_eq!(default.len(), 1, "target={target:?}");
        assert_eq!(
            default[0].disposition(),
            FamilyThemeDisposition::TypedAdapter,
            "target={target:?}"
        );
    }
}

#[test]
fn tree_view_marker_qualified_scalar_paints_remain_unsupported() {
    for style in [
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid Tree View fill")),
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#654321").expect("valid Tree View stroke")),
    ] {
        for variant in ThemeVariant::ALL {
            if *variant == ThemeVariant::Default {
                continue;
            }
            let routes = compile_rule_routes(
                DiagramFamilyId::TREE_VIEW,
                0,
                &ThemeRule::new(ThemeTarget::Marker, style.clone()).with_variant(*variant),
            );
            assert_eq!(routes.len(), 1, "variant={variant:?}");
            assert_eq!(
                routes[0].disposition(),
                FamilyThemeDisposition::Unsupported,
                "variant={variant:?}"
            );
        }
    }
}

#[test]
fn flowchart_and_swimlane_marker_paints_have_no_writer_consumer() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for variant in std::iter::once(None).chain(ThemeVariant::ALL.iter().copied().map(Some)) {
            for kind in [
                FamilyThemePaintKind::Clear,
                FamilyThemePaintKind::Transparent,
                FamilyThemePaintKind::Solid,
                FamilyThemePaintKind::LinearGradient,
                FamilyThemePaintKind::RadialGradient,
                FamilyThemePaintKind::Pattern,
            ] {
                for facet in [
                    FamilyThemeRuleFacet::Fill(kind),
                    FamilyThemeRuleFacet::Stroke(kind),
                ] {
                    assert_eq!(
                        classify_rule_facet(
                            family,
                            ThemeTarget::Marker,
                            FamilyThemeSelectorShape::Static { variant },
                            facet,
                        ),
                        FamilyThemeDisposition::Unsupported,
                        "{family}/{variant:?}/{facet:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn flowchart_and_swimlane_node_label_fill_owns_unqualified_and_explicit_default_routes() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for paint_kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                assert_eq!(
                    classify_rule_facet(
                        family,
                        ThemeTarget::NodeLabel,
                        FamilyThemeSelectorShape::Static { variant },
                        FamilyThemeRuleFacet::Fill(paint_kind),
                    ),
                    FamilyThemeDisposition::TypedAdapter,
                    "family={family:?} paint={paint_kind:?} variant={variant:?}"
                );
            }
        }
    }
}

#[test]
fn flowchart_and_swimlane_own_only_static_scalar_edge_paints() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for paint in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            for facet in [
                FamilyThemeRuleFacet::Fill(paint),
                FamilyThemeRuleFacet::Stroke(paint),
            ] {
                for selector in [
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Active),
                    },
                    FamilyThemeSelectorShape::Ordinal {
                        variant: None,
                        selector: OrdinalSelector::exact(1).unwrap(),
                    },
                ] {
                    let typed = matches!(
                        paint,
                        FamilyThemePaintKind::Solid | FamilyThemePaintKind::Transparent
                    ) && matches!(
                        selector,
                        FamilyThemeSelectorShape::Static {
                            variant: None | Some(ThemeVariant::Default)
                        }
                    );
                    assert_eq!(
                        classify_rule_facet(family, ThemeTarget::Edge, selector, facet),
                        if typed {
                            FamilyThemeDisposition::TypedAdapter
                        } else {
                            FamilyThemeDisposition::Unsupported
                        },
                        "family={family:?} selector={selector:?} facet={facet:?}",
                    );
                }
            }
        }
        assert_eq!(
            compile_ordinal_palette_route(family, ThemeTarget::Edge).disposition(),
            FamilyThemeDisposition::Unsupported
        );
    }
}

#[test]
fn flowchart_label_effect_routes_preserve_family_variant_and_ordinal_boundaries() {
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
            assert_eq!(
                classify_effect_binding(family, target),
                if family == DiagramFamilyId::FLOWCHART {
                    FamilyThemeDisposition::TypedAdapter
                } else {
                    FamilyThemeDisposition::Unsupported
                },
            );
            for variant in [
                None,
                Some(ThemeVariant::Default),
                Some(ThemeVariant::Active),
            ] {
                for ordinal in [false, true] {
                    let selector = if ordinal {
                        FamilyThemeSelectorShape::Ordinal {
                            variant,
                            selector: OrdinalSelector::exact(1).unwrap(),
                        }
                    } else {
                        FamilyThemeSelectorShape::Static { variant }
                    };
                    let typed = family == DiagramFamilyId::FLOWCHART
                        && variant != Some(ThemeVariant::Active)
                        && (!ordinal || target == ThemeTarget::NodeLabel);
                    assert_eq!(
                        classify_rule_facet(family, target, selector, FamilyThemeRuleFacet::Effect),
                        if typed {
                            FamilyThemeDisposition::TypedAdapter
                        } else {
                            FamilyThemeDisposition::Unsupported
                        },
                        "family={family:?} target={target:?} selector={selector:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn flowchart_and_swimlane_own_static_default_label_typography_and_edge_padding() {
    let font_stack = super::super::FontStack::single("Excalifont").expect("valid font stack");
    let style = ThemeStylePatch {
        typography: TextStylePatch {
            font_stack: Specified::Value(font_stack),
            font_size_px: Specified::Value(20.0),
            font_weight: Specified::Value(700),
            ..TextStylePatch::default()
        },
        ..ThemeStylePatch::default()
    };
    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
            let unqualified_rule = ThemeRule::new(target, style.clone());
            assert!(
                compile_rule_routes(family, 0, &unqualified_rule)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
            );

            let default_rule =
                ThemeRule::new(target, style.clone()).with_variant(ThemeVariant::Default);
            assert!(
                compile_rule_routes(family, 0, &default_rule)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
            );

            for rule in [
                ThemeRule::new(target, style.clone()).with_variant(ThemeVariant::Active),
                ThemeRule::new(target, style.clone())
                    .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
            ] {
                assert!(
                    compile_rule_routes(family, 0, &rule).iter().all(|route| {
                        route.disposition() == FamilyThemeDisposition::Unsupported
                    })
                );
            }

            let letter_spacing_rule = ThemeRule::new(
                target,
                ThemeStylePatch {
                    typography: TextStylePatch {
                        letter_spacing_px: Specified::Value(1.5),
                        ..TextStylePatch::default()
                    },
                    ..ThemeStylePatch::default()
                },
            );
            assert!(
                compile_rule_routes(family, 0, &letter_spacing_rule)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
            );
        }

        assert!(
            compile_rule_routes(family, 0, &ThemeRule::new(ThemeTarget::Edge, style.clone()))
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );

        let edge_padding = ThemeRule::new(
            ThemeTarget::EdgeLabel,
            ThemeStylePatch::default().with_padding(super::super::InsetsPx::all(8.0)),
        );
        assert!(
            compile_rule_routes(family, 0, &edge_padding)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );
        let node_padding = ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_padding(super::super::InsetsPx::all(8.0)),
        );
        assert!(
            compile_rule_routes(family, 0, &node_padding)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );
    }
}

#[test]
fn sequence_owns_only_static_unqualified_role_typography() {
    let supported_properties = [
        ThemeTypographyProperty::FontStack,
        ThemeTypographyProperty::FontSize,
        ThemeTypographyProperty::FontWeight,
        ThemeTypographyProperty::FontStyle,
    ];
    let targets = [
        ThemeTarget::ActorLabel,
        ThemeTarget::MessageLabel,
        ThemeTarget::NoteLabel,
        ThemeTarget::LoopLabel,
    ];

    for target in targets {
        for property in supported_properties {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::SEQUENCE,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Typography(property),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "{target:?}/{property:?} should be a direct Sequence role route"
            );
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::SEQUENCE,
                    target,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Typography(property),
                ),
                FamilyThemeDisposition::Unsupported,
                "explicit variants remain outside the first role-typography tranche"
            );
        }
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::SEQUENCE,
                target,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Typography(ThemeTypographyProperty::LineHeight),
            ),
            FamilyThemeDisposition::Unsupported
        );
    }

    let base = TextStyle::default()
        .with_font_size_px(18.0)
        .expect("valid base font size")
        .with_font_weight(600)
        .expect("valid base font weight");
    let base_routes = compile_base_typography_routes(DiagramFamilyId::SEQUENCE, &base);
    assert!(base_routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(base_routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
            && route.disposition() == FamilyThemeDisposition::Unsupported
    }));
}

#[test]
fn unsupported_gradient_and_rule_typography_report_residual_routes() {
    let gradient = super::super::canvas::LinearGradient::new(
        90.0,
        [
            super::super::canvas::GradientStop::new(
                0.0,
                ThemeColorValue::parse("#000000").expect("valid color"),
            )
            .expect("valid stop"),
            super::super::canvas::GradientStop::new(
                1.0,
                ThemeColorValue::parse("#ffffff").expect("valid color"),
            )
            .expect("valid stop"),
        ],
    )
    .expect("valid gradient");
    let typography = TextStylePatch {
        font_weight: Specified::Value(700),
        ..TextStylePatch::default()
    };
    let rule = ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch {
            paint: ThemePaintPatch {
                fill: Specified::Value(CanvasPaint::LinearGradient(gradient)),
                ..ThemePaintPatch::default()
            },
            typography,
            ..ThemeStylePatch::default()
        },
    );

    let routes = compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &rule);

    assert_eq!(routes.len(), 2);
    assert!(
        routes
            .iter()
            .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
    );
}

#[test]
fn state_splits_unsupported_clear_paint_from_supported_geometry() {
    let rule = ThemeRule::new(
        ThemeTarget::State,
        ThemeStylePatch {
            paint: ThemePaintPatch {
                fill: Specified::Clear,
                ..ThemePaintPatch::default()
            },
            geometry: ThemeGeometryPatch {
                radius: Specified::Value(4.0),
            },
            ..ThemeStylePatch::default()
        },
    );

    let routes = compile_rule_routes(DiagramFamilyId::STATE, 0, &rule);
    assert_eq!(routes.len(), 2);
    let dispositions = routes
        .iter()
        .map(|route| {
            let FamilyThemeMechanism::RuleFacet { facet, .. } = route.mechanism() else {
                panic!("expected a State rule facet");
            };
            (facet, route.disposition())
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        dispositions,
        std::collections::BTreeMap::from([
            (
                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Clear),
                FamilyThemeDisposition::Unsupported,
            ),
            (
                FamilyThemeRuleFacet::Radius,
                FamilyThemeDisposition::TypedAdapter
            ),
        ]),
    );
}

#[test]
fn state_routes_reject_mechanisms_without_a_terminal_consumer() {
    use FamilyThemeDisposition::{TypedAdapter, Unsupported};
    use FamilyThemePaintKind::{Clear, LinearGradient, Solid};
    use FamilyThemeRuleFacet::{Effect, Fill, Padding, Stroke, StrokeWidth, Typography};

    for (target, facet, expected) in [
        (ThemeTarget::State, Fill(Solid), TypedAdapter),
        (ThemeTarget::State, Fill(Clear), Unsupported),
        (ThemeTarget::StateLabel, Fill(LinearGradient), Unsupported),
        (ThemeTarget::StateLabel, Stroke(Solid), Unsupported),
        (ThemeTarget::StateLabel, StrokeWidth, Unsupported),
        (ThemeTarget::Transition, Padding, Unsupported),
        (ThemeTarget::Note, Padding, TypedAdapter),
        (ThemeTarget::State, Effect, TypedAdapter),
        (ThemeTarget::Note, Effect, TypedAdapter),
        (ThemeTarget::Composite, Effect, TypedAdapter),
        (ThemeTarget::SpecialState, Effect, TypedAdapter),
        (ThemeTarget::StateLabel, Effect, Unsupported),
        (
            ThemeTarget::State,
            Typography(ThemeTypographyProperty::FontStack),
            Unsupported,
        ),
        (
            ThemeTarget::StateLabel,
            Typography(ThemeTypographyProperty::FontStack),
            TypedAdapter,
        ),
        (
            ThemeTarget::StateLabel,
            Typography(ThemeTypographyProperty::LineHeight),
            Unsupported,
        ),
    ] {
        for_each_matrix_selector(|selector| {
            let expected = match selector {
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Bar | ThemeVariant::Line),
                }
                | FamilyThemeSelectorShape::Ordinal {
                    variant: Some(ThemeVariant::Bar | ThemeVariant::Line),
                    ..
                } => Unsupported,
                _ => expected,
            };
            assert_eq!(
                classify_rule_facet(DiagramFamilyId::STATE, target, selector, facet),
                expected,
                "{target:?} {selector:?} {facet:?}",
            );
        });
    }
}

#[test]
fn state_palette_and_effect_bindings_require_their_actual_target_domains() {
    use FamilyThemeDisposition::{TypedAdapter, Unsupported};

    for (target, expected) in [
        (ThemeTarget::StateLabel, TypedAdapter),
        (ThemeTarget::Transition, TypedAdapter),
        (ThemeTarget::Title, Unsupported),
        (ThemeTarget::CompositeHeader, Unsupported),
        (ThemeTarget::TransitionMarker, Unsupported),
    ] {
        assert_eq!(
            compile_ordinal_palette_route(DiagramFamilyId::STATE, target).disposition(),
            expected,
            "palette {target:?}",
        );
    }
    for (target, expected) in [
        (ThemeTarget::State, TypedAdapter),
        (ThemeTarget::StateLabel, Unsupported),
        (ThemeTarget::Composite, Unsupported),
        (ThemeTarget::Note, Unsupported),
    ] {
        assert_eq!(
            compile_effect_binding_route(DiagramFamilyId::STATE, 0, target).disposition(),
            expected,
            "effect {target:?}",
        );
    }
}

#[test]
fn unsupported_ordinal_rules_report_residual_routes() {
    let rule = ThemeRule::new(
        ThemeTarget::Actor,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
    )
    .with_ordinal(OrdinalSelector::exact(1).expect("valid ordinal"));
    let routes = compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &rule);

    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].disposition(), FamilyThemeDisposition::Unsupported);
}

#[test]
fn sequence_direct_surface_routes_accept_static_scalar_paints_and_default_variants() {
    let gradient = super::super::canvas::LinearGradient::new(
        90.0,
        [
            super::super::canvas::GradientStop::new(
                0.0,
                ThemeColorValue::parse("#000000").expect("valid color"),
            )
            .expect("valid stop"),
            super::super::canvas::GradientStop::new(
                1.0,
                ThemeColorValue::parse("#ffffff").expect("valid color"),
            )
            .expect("valid stop"),
        ],
    )
    .expect("valid gradient");
    for target in [
        ThemeTarget::Actor,
        ThemeTarget::LoopLabelBackground,
        ThemeTarget::Note,
        ThemeTarget::Activation,
    ] {
        let solid = ThemeRule::new(
            target,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#abcdef").expect("valid fill"))
                .with_stroke(CanvasPaint::solid("#123456").expect("valid stroke")),
        );
        let gradient_rule = ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient.clone())),
        );

        let solid_routes = compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &solid);
        assert_eq!(solid_routes.len(), 2);
        assert!(
            solid_routes
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );
        let explicit_default = ThemeRule::new(
            target,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#abcdef").expect("valid fill"))
                .with_stroke(CanvasPaint::solid("#123456").expect("valid stroke")),
        )
        .with_variant(ThemeVariant::Default);
        assert!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &explicit_default)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &gradient_rule)[0].disposition(),
            FamilyThemeDisposition::Unsupported
        );
        let ordinal = ThemeRule::new(
            target,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#abcdef").expect("valid ordinal fill"))
                .with_stroke(CanvasPaint::solid("#123456").expect("valid ordinal stroke")),
        )
        .with_ordinal(OrdinalSelector::exact(1).expect("valid ordinal"));
        assert!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &ordinal)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );
    }
}

#[test]
fn sequence_role_label_paints_own_static_scalar_fill_and_default_variant() {
    let static_unqualified = FamilyThemeSelectorShape::Static { variant: None };
    let explicit_default = FamilyThemeSelectorShape::Static {
        variant: Some(ThemeVariant::Default),
    };
    let ordinal = FamilyThemeSelectorShape::Ordinal {
        variant: None,
        selector: OrdinalSelector::exact(1).expect("valid ordinal"),
    };

    for target in [
        ThemeTarget::ActorLabel,
        ThemeTarget::MessageLabel,
        ThemeTarget::NoteLabel,
        ThemeTarget::LoopLabel,
    ] {
        for kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            let fill = FamilyThemeRuleFacet::Fill(kind);
            assert_eq!(
                classify_rule_facet(DiagramFamilyId::SEQUENCE, target, static_unqualified, fill,),
                FamilyThemeDisposition::TypedAdapter
            );
            assert_eq!(
                classify_rule_facet(DiagramFamilyId::SEQUENCE, target, explicit_default, fill,),
                FamilyThemeDisposition::TypedAdapter
            );
            assert_eq!(
                classify_rule_facet(DiagramFamilyId::SEQUENCE, target, ordinal, fill),
                FamilyThemeDisposition::Unsupported
            );
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::SEQUENCE,
                    target,
                    static_unqualified,
                    FamilyThemeRuleFacet::Stroke(kind),
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
        for kind in [
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::SEQUENCE,
                    target,
                    static_unqualified,
                    FamilyThemeRuleFacet::Fill(kind),
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
    }
}

#[test]
fn sequence_message_owns_static_scalar_fill_and_stroke_with_default_variant() {
    for paint in [
        CanvasPaint::Transparent,
        CanvasPaint::solid("#123456").expect("valid Message stroke"),
    ] {
        let stroke = ThemeRule::new(
            ThemeTarget::Message,
            ThemeStylePatch::default().with_stroke(paint.clone()),
        );
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &stroke)[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );

        let explicit_default = ThemeRule::new(
            ThemeTarget::Message,
            ThemeStylePatch::default().with_stroke(paint),
        )
        .with_variant(ThemeVariant::Default);
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &explicit_default)[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
    }

    for paint in [
        CanvasPaint::Transparent,
        CanvasPaint::solid("#abcdef").expect("valid Message fill"),
    ] {
        let fill = ThemeRule::new(
            ThemeTarget::Message,
            ThemeStylePatch::default().with_fill(paint.clone()),
        );
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &fill)[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );

        let explicit_default = ThemeRule::new(
            ThemeTarget::Message,
            ThemeStylePatch::default().with_fill(paint),
        )
        .with_variant(ThemeVariant::Default);
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &explicit_default)[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );
    }
}

#[test]
fn sequence_number_label_owns_only_static_unqualified_scalar_fill() {
    for paint in [
        CanvasPaint::Transparent,
        CanvasPaint::solid("#123456").expect("valid SequenceNumberLabel fill"),
    ] {
        let fill = ThemeRule::new(
            ThemeTarget::SequenceNumberLabel,
            ThemeStylePatch::default().with_fill(paint.clone()),
        );
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &fill)[0].disposition(),
            FamilyThemeDisposition::TypedAdapter
        );

        let explicit_default = ThemeRule::new(
            ThemeTarget::SequenceNumberLabel,
            ThemeStylePatch::default().with_fill(paint),
        )
        .with_variant(ThemeVariant::Default);
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &explicit_default)[0].disposition(),
            FamilyThemeDisposition::Unsupported
        );
    }

    let stroke = ThemeRule::new(
        ThemeTarget::SequenceNumberLabel,
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#abcdef").expect("valid unsupported stroke")),
    );
    assert_eq!(
        compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &stroke)[0].disposition(),
        FamilyThemeDisposition::Unsupported
    );

    let ordinal = ThemeRule::new(
        ThemeTarget::SequenceNumberLabel,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid ordinal fill")),
    )
    .with_ordinal(OrdinalSelector::exact(1).expect("valid ordinal"));
    assert_eq!(
        compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &ordinal)[0].disposition(),
        FamilyThemeDisposition::Unsupported
    );
}

#[test]
fn sequence_lifeline_owns_static_scalar_fill_stroke_and_unqualified_width() {
    for paint in [
        CanvasPaint::Transparent,
        CanvasPaint::solid("#123456").expect("valid Lifeline paint"),
    ] {
        let rule = ThemeRule::new(
            ThemeTarget::Lifeline,
            ThemeStylePatch::default()
                .with_fill(paint.clone())
                .with_stroke(paint.clone()),
        );
        assert!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );

        let explicit_default = ThemeRule::new(
            ThemeTarget::Lifeline,
            ThemeStylePatch::default()
                .with_fill(paint.clone())
                .with_stroke(paint),
        )
        .with_variant(ThemeVariant::Default);
        assert!(
            compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &explicit_default)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );
    }

    let stroke_width = ThemeRule::new(
        ThemeTarget::Lifeline,
        ThemeStylePatch::default()
            .with_stroke_width(2.0)
            .expect("valid Lifeline stroke width"),
    );
    assert_eq!(
        compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &stroke_width)[0].disposition(),
        FamilyThemeDisposition::TypedAdapter
    );

    let explicit_default = ThemeRule::new(
        ThemeTarget::Lifeline,
        ThemeStylePatch::default()
            .with_stroke_width(2.0)
            .expect("valid Lifeline stroke width"),
    )
    .with_variant(ThemeVariant::Default);
    assert_eq!(
        compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &explicit_default)[0].disposition(),
        FamilyThemeDisposition::Unsupported
    );

    for paint_kind in [
        FamilyThemePaintKind::Clear,
        FamilyThemePaintKind::LinearGradient,
        FamilyThemePaintKind::RadialGradient,
        FamilyThemePaintKind::Pattern,
    ] {
        for facet in [
            FamilyThemeRuleFacet::Fill(paint_kind),
            FamilyThemeRuleFacet::Stroke(paint_kind),
        ] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::SEQUENCE,
                    ThemeTarget::Lifeline,
                    FamilyThemeSelectorShape::Static { variant: None },
                    facet,
                ),
                FamilyThemeDisposition::Unsupported
            );
        }
    }
}

#[test]
fn flowchart_and_swimlane_clusters_own_unqualified_scalar_paints() {
    for paint in [
        CanvasPaint::Transparent,
        CanvasPaint::solid("#123456").expect("valid Cluster paint"),
    ] {
        let rule = ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(paint.clone())
                .with_stroke(paint.clone()),
        );
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            assert!(
                compile_rule_routes(family, 0, &rule)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
            );
        }

        let explicit_default = ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(paint.clone())
                .with_stroke(paint.clone()),
        )
        .with_variant(ThemeVariant::Default);
        assert!(
            compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &explicit_default)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );
        assert!(
            compile_rule_routes(DiagramFamilyId::SWIMLANE, 0, &explicit_default)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );
    }

    for paint_kind in [
        FamilyThemePaintKind::Clear,
        FamilyThemePaintKind::LinearGradient,
        FamilyThemePaintKind::RadialGradient,
        FamilyThemePaintKind::Pattern,
    ] {
        for facet in [
            FamilyThemeRuleFacet::Fill(paint_kind),
            FamilyThemeRuleFacet::Stroke(paint_kind),
        ] {
            for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
                assert_eq!(
                    classify_rule_facet(
                        family,
                        ThemeTarget::Cluster,
                        FamilyThemeSelectorShape::Static { variant: None },
                        facet,
                    ),
                    FamilyThemeDisposition::Unsupported
                );
            }
        }
    }
}

#[test]
fn treemap_title_only_owns_unqualified_scalar_fill() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::TREEMAP,
                ThemeTarget::Title,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
            FamilyThemeDisposition::TypedAdapter
        );
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::TREEMAP,
                ThemeTarget::Title,
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Default),
                },
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
            FamilyThemeDisposition::Unsupported
        );
    }

    for paint_kind in [
        FamilyThemePaintKind::Clear,
        FamilyThemePaintKind::LinearGradient,
        FamilyThemePaintKind::RadialGradient,
        FamilyThemePaintKind::Pattern,
    ] {
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::TREEMAP,
                ThemeTarget::Title,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(paint_kind),
            ),
            FamilyThemeDisposition::Unsupported
        );
    }

    assert_eq!(
        classify_rule_facet(
            DiagramFamilyId::TREEMAP,
            ThemeTarget::Title,
            FamilyThemeSelectorShape::Ordinal {
                variant: None,
                selector: OrdinalSelector::exact(1).expect("valid Treemap title ordinal"),
            },
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
        ),
        FamilyThemeDisposition::Unsupported
    );
}

#[test]
fn final_direct_family_slices_own_only_their_supported_static_scalar_fill() {
    for (family, target, supports_default) in [
        (DiagramFamilyId::EVENT_MODELING, ThemeTarget::Text, false),
        (DiagramFamilyId::ISHIKAWA, ThemeTarget::Text, false),
        (DiagramFamilyId::VENN, ThemeTarget::Title, false),
        (DiagramFamilyId::VENN, ThemeTarget::Text, true),
        (DiagramFamilyId::ZENUML, ThemeTarget::Title, false),
    ] {
        for paint_kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "family={family} target={target:?} paint={paint_kind:?}"
            );
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static {
                        variant: Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                if supports_default {
                    FamilyThemeDisposition::TypedAdapter
                } else {
                    FamilyThemeDisposition::Unsupported
                },
                "qualified family={family} target={target:?} paint={paint_kind:?}"
            );
        }

        for paint_kind in [
            FamilyThemePaintKind::Clear,
            FamilyThemePaintKind::LinearGradient,
            FamilyThemePaintKind::RadialGradient,
            FamilyThemePaintKind::Pattern,
        ] {
            assert_eq!(
                classify_rule_facet(
                    family,
                    target,
                    FamilyThemeSelectorShape::Static { variant: None },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                FamilyThemeDisposition::Unsupported,
                "family={family} target={target:?} paint={paint_kind:?}"
            );
        }

        assert_eq!(
            classify_rule_facet(
                family,
                target,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None,
                    selector: OrdinalSelector::exact(1).expect("valid ordinal"),
                },
                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            ),
            FamilyThemeDisposition::Unsupported,
            "ordinal family={family} target={target:?}"
        );
    }

    assert_eq!(
        classify_rule_facet(
            DiagramFamilyId::ZENUML,
            ThemeTarget::Text,
            FamilyThemeSelectorShape::Static { variant: None },
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
        ),
        FamilyThemeDisposition::Unsupported
    );
    for property in [
        ThemeTypographyProperty::FontStack,
        ThemeTypographyProperty::FontSize,
    ] {
        assert_eq!(
            classify_base_typography(DiagramFamilyId::ZENUML, property),
            FamilyThemeDisposition::Unsupported
        );
    }
    for family in [DiagramFamilyId::EVENT_MODELING, DiagramFamilyId::ISHIKAWA] {
        assert_eq!(
            classify_rule_facet(
                family,
                ThemeTarget::Title,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            ),
            FamilyThemeDisposition::Unsupported
        );
    }
}

#[test]
fn explicit_clear_remains_an_unsupported_paint_facet() {
    let rule = ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch {
            paint: ThemePaintPatch {
                fill: Specified::Clear,
                ..ThemePaintPatch::default()
            },
            ..ThemeStylePatch::default()
        },
    );

    let routes = compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &rule);

    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].disposition(), FamilyThemeDisposition::Unsupported);
    assert!(matches!(
        routes[0].mechanism(),
        FamilyThemeMechanism::RuleFacet {
            facet: FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Clear),
            ..
        }
    ));
}

#[test]
fn explicit_default_font_size_is_distinct_from_omission() {
    let omitted = compile_base_typography_routes(DiagramFamilyId::SEQUENCE, &TextStyle::default());
    assert!(omitted.iter().all(|route| {
        route.mechanism() != FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
    }));

    let explicit = TextStyle::default()
        .with_font_size_px(16.0)
        .expect("default-valued font size remains a valid explicit request");
    let routes = compile_base_typography_routes(DiagramFamilyId::SEQUENCE, &explicit);
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
}

#[test]
fn sequence_directly_owns_only_base_font_stack_and_size() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace").expect("valid Sequence base font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid font size")
        .with_font_weight(700)
        .expect("valid font weight");
    let routes = compile_base_typography_routes(DiagramFamilyId::SEQUENCE, &typography);

    assert_eq!(routes.len(), 3);
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
            && route.disposition() == FamilyThemeDisposition::Unsupported
    }));
}

#[test]
fn inherited_font_families_directly_own_only_the_base_font_stack() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace").expect("valid inherited font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid unsupported inherited font size");

    for family in [
        DiagramFamilyId::PACKET,
        DiagramFamilyId::MINDMAP,
        DiagramFamilyId::SANKEY,
        DiagramFamilyId::VENN,
        DiagramFamilyId::PIE,
        DiagramFamilyId::QUADRANT_CHART,
    ] {
        let routes = compile_base_typography_routes(family, &typography);
        assert!(
            routes.iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            }),
            "{family} must directly own FontStack"
        );
        assert!(
            routes.iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::Unsupported
            }),
            "{family} must reject FontSize without inventing a terminal"
        );
    }
}

#[test]
fn requirement_directly_owns_base_font_stack_and_size() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace")
                .expect("valid Requirement base font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid Requirement font size");
    let routes = compile_base_typography_routes(DiagramFamilyId::REQUIREMENT, &typography);

    assert_eq!(routes.len(), 2);
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
}

#[test]
fn er_directly_owns_base_font_stack_and_size() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace").expect("valid ER base font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid ER font size");
    let routes = compile_base_typography_routes(DiagramFamilyId::ER, &typography);

    assert_eq!(routes.len(), 2);
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
}

#[test]
fn kanban_directly_owns_base_font_stack_and_size() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace").expect("valid Kanban base font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid Kanban font size")
        .with_font_weight(700)
        .expect("valid Kanban font weight");
    let routes = compile_base_typography_routes(DiagramFamilyId::KANBAN, &typography);

    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
            && route.disposition() == FamilyThemeDisposition::Unsupported
    }));
}

#[test]
fn block_directly_owns_base_font_stack_and_size() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace").expect("valid Block font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid Block font size")
        .with_font_weight(700)
        .expect("valid Block font weight");
    let routes = compile_base_typography_routes(DiagramFamilyId::BLOCK, &typography);

    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
            && route.disposition() == FamilyThemeDisposition::Unsupported
    }));
}

#[test]
fn inherited_text_families_directly_own_only_the_base_font_stack() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace")
                .expect("valid inherited-family font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid inherited-family font size")
        .with_font_weight(700)
        .expect("valid inherited-family font weight");

    for family in [
        DiagramFamilyId::INFO,
        DiagramFamilyId::ERROR,
        DiagramFamilyId::CYNEFIN,
        DiagramFamilyId::WARDLEY,
    ] {
        let routes = compile_base_typography_routes(family, &typography);

        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
        }));
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                && route.disposition() == FamilyThemeDisposition::Unsupported
        }));
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
                && route.disposition() == FamilyThemeDisposition::Unsupported
        }));
    }
}

#[test]
fn xychart_font_stack_is_typed_while_font_size_remains_unsupported() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("XY Chart Phantom Font")
                .expect("valid XY Chart font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid XY Chart font size");
    let routes = compile_base_typography_routes(DiagramFamilyId::XY_CHART, &typography);

    assert_eq!(routes.len(), 2);
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::Unsupported
    }));
}

#[test]
fn treemap_font_stack_is_typed_while_font_size_remains_unsupported() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("Treemap font stack")
                .expect("valid Treemap font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid Treemap font size");
    let routes = compile_base_typography_routes(DiagramFamilyId::TREEMAP, &typography);

    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::Unsupported
    }));
}

#[test]
fn wardley_generic_text_paint_routes_are_unsupported_without_terminals() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for target in [ThemeTarget::Text, ThemeTarget::Title] {
            for variant in [None, Some(ThemeVariant::Default)] {
                assert_eq!(
                    classify_rule_facet(
                        DiagramFamilyId::WARDLEY,
                        target,
                        FamilyThemeSelectorShape::Static { variant },
                        FamilyThemeRuleFacet::Fill(paint_kind),
                    ),
                    FamilyThemeDisposition::Unsupported,
                    "target={target:?} facet=Fill({paint_kind:?}) variant={variant:?}"
                );
            }
        }
    }
}

#[test]
fn generic_title_paint_routes_are_unsupported_without_a_title_color_consumer() {
    for family in [
        DiagramFamilyId::ARCHITECTURE,
        DiagramFamilyId::C4,
        DiagramFamilyId::CYNEFIN,
        DiagramFamilyId::ER,
        DiagramFamilyId::KANBAN,
        DiagramFamilyId::SANKEY,
        DiagramFamilyId::TIMELINE,
    ] {
        for paint_kind in [
            FamilyThemePaintKind::Transparent,
            FamilyThemePaintKind::Solid,
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                assert_eq!(
                    classify_rule_facet(
                        family,
                        ThemeTarget::Title,
                        FamilyThemeSelectorShape::Static { variant },
                        FamilyThemeRuleFacet::Fill(paint_kind),
                    ),
                    FamilyThemeDisposition::Unsupported,
                    "family={family} facet=Fill({paint_kind:?}) variant={variant:?}"
                );
            }
        }

        assert_eq!(
            classify_rule_facet(
                family,
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static { variant: None },
                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            ),
            FamilyThemeDisposition::TypedAdapter,
            "{family} must retain its real generic text consumer"
        );
    }
}

#[test]
fn railroad_title_paint_is_owned_by_the_typed_rule_name_route() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::RAILROAD,
                    ThemeTarget::Title,
                    FamilyThemeSelectorShape::Static { variant },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "Railroad rule-name titleColor route drifted for {paint_kind:?}, variant={variant:?}"
            );
        }
    }
}

#[test]
fn railroad_text_paint_is_owned_by_the_typed_text_surface() {
    for paint_kind in [
        FamilyThemePaintKind::Transparent,
        FamilyThemePaintKind::Solid,
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            assert_eq!(
                classify_rule_facet(
                    DiagramFamilyId::RAILROAD,
                    ThemeTarget::Text,
                    FamilyThemeSelectorShape::Static { variant },
                    FamilyThemeRuleFacet::Fill(paint_kind),
                ),
                FamilyThemeDisposition::TypedAdapter,
                "Railroad text fill route drifted for {paint_kind:?}, variant={variant:?}"
            );
        }
    }
    for variant in [
        Some(ThemeVariant::Active),
        Some(ThemeVariant::Success),
        Some(ThemeVariant::Error),
    ] {
        assert_eq!(
            classify_rule_facet(
                DiagramFamilyId::RAILROAD,
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static { variant },
                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            ),
            FamilyThemeDisposition::Unsupported,
            "Railroad text fill must remain closed for {variant:?}"
        );
    }
}

#[test]
fn info_and_error_text_paint_routes_match_their_actual_terminals() {
    let disposition = |family, target| {
        let rule = ThemeRule::new(
            target,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").expect("valid Info/Error terminal fill")),
        );
        compile_rule_routes(family, 0, &rule)[0].disposition()
    };

    assert_eq!(
        disposition(DiagramFamilyId::INFO, ThemeTarget::Text),
        FamilyThemeDisposition::TypedAdapter
    );
    let explicit_default = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid Info terminal fill")),
    )
    .with_variant(ThemeVariant::Default);
    assert_eq!(
        compile_rule_routes(DiagramFamilyId::INFO, 0, &explicit_default)[0].disposition(),
        FamilyThemeDisposition::TypedAdapter
    );
    assert_eq!(
        disposition(DiagramFamilyId::INFO, ThemeTarget::Title),
        FamilyThemeDisposition::Unsupported
    );
    for target in [ThemeTarget::Text, ThemeTarget::Title] {
        assert_eq!(
            disposition(DiagramFamilyId::ERROR, target),
            FamilyThemeDisposition::Unsupported
        );
    }
}

#[test]
fn packet_text_roles_own_default_static_scalar_fill() {
    for target in [
        ThemeTarget::Text,
        ThemeTarget::PacketByteLabel,
        ThemeTarget::PacketFieldLabel,
        ThemeTarget::Title,
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut rule = ThemeRule::new(
                target,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#123456").expect("valid Packet role fill")),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            assert_eq!(
                compile_rule_routes(DiagramFamilyId::PACKET, 0, &rule)[0].disposition(),
                FamilyThemeDisposition::TypedAdapter,
                "{target:?} {variant:?}"
            );
        }

        let active = ThemeRule::new(
            target,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").expect("valid Packet active fill")),
        )
        .with_variant(ThemeVariant::Active);
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::PACKET, 0, &active)[0].disposition(),
            FamilyThemeDisposition::Unsupported,
            "{target:?} Active"
        );
    }
}

#[test]
fn railroad_directly_owns_base_typography_and_scalar_text_paint() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("monospace").expect("valid Railroad font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid Railroad font size")
        .with_font_weight(700)
        .expect("valid Railroad font weight");
    let routes = compile_base_typography_routes(DiagramFamilyId::RAILROAD, &typography);

    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism() == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
            && route.disposition() == FamilyThemeDisposition::TypedAdapter
    }));
    assert!(routes.iter().any(|route| {
        route.mechanism()
            == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
            && route.disposition() == FamilyThemeDisposition::Unsupported
    }));
}

#[test]
fn flowchart_and_swimlane_base_typography_is_owned_by_the_shared_layout_plan() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("Excalifont").expect("valid fixture font stack"),
        )
        .with_font_size_px(18.0)
        .expect("valid font size")
        .with_font_weight(700)
        .expect("valid font weight");

    for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
        let routes = compile_base_typography_routes(family, &typography);

        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
        }));
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
        }));
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
                && route.disposition() == FamilyThemeDisposition::Unsupported
        }));
    }
}

#[test]
fn class_eventmodeling_gitgraph_ishikawa_and_radar_mark_font_stack_and_font_size_as_typed() {
    let typography = TextStyle::default()
        .with_font_stack(
            super::super::FontStack::single("DirectTypographySans")
                .expect("valid direct typography font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid direct typography font size")
        .with_font_weight(700)
        .expect("valid unsupported direct typography font weight");

    for family in [
        DiagramFamilyId::CLASS,
        DiagramFamilyId::EVENT_MODELING,
        DiagramFamilyId::GIT_GRAPH,
        DiagramFamilyId::ISHIKAWA,
        DiagramFamilyId::RADAR,
    ] {
        let routes = compile_base_typography_routes(family, &typography);
        for property in [
            ThemeTypographyProperty::FontStack,
            ThemeTypographyProperty::FontSize,
        ] {
            assert!(
                routes.iter().any(|route| {
                    route.mechanism() == FamilyThemeMechanism::BaseTypography(property)
                        && route.disposition() == FamilyThemeDisposition::TypedAdapter
                }),
                "{family} must directly own {property:?}"
            );
        }
        assert!(
            routes.iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
                    && route.disposition() == FamilyThemeDisposition::Unsupported
            }),
            "{family} must reject unsupported font weight"
        );
    }
}

#[test]
fn architecture_direct_typography_accepts_oversized_font_stacks() {
    let families = (0..32)
        .map(|index| format!("font-{index}-{}", "x".repeat(180)))
        .collect::<Vec<_>>();
    let typography = TextStyle::default()
        .with_font_stack(super::super::FontStack::new(families).expect("valid font stack"))
        .with_font_size_px(18.0)
        .expect("valid font size");
    let routes = compile_base_typography_routes(DiagramFamilyId::ARCHITECTURE, &typography);

    assert_eq!(routes.len(), 2);
    for property in [
        ThemeTypographyProperty::FontStack,
        ThemeTypographyProperty::FontSize,
    ] {
        assert!(routes.iter().any(|route| {
            route.mechanism() == FamilyThemeMechanism::BaseTypography(property)
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
        }));
    }
}

#[test]
fn direct_typography_families_accept_oversized_font_stacks() {
    let families = (0..32)
        .map(|index| format!("font-{index}-{}", "x".repeat(180)))
        .collect::<Vec<_>>();
    let typography = TextStyle::default()
        .with_font_stack(super::super::FontStack::new(families).expect("valid font stack"))
        .with_font_size_px(18.0)
        .expect("valid font size");
    for family in [DiagramFamilyId::SEQUENCE, DiagramFamilyId::GIT_GRAPH] {
        let routes = compile_base_typography_routes(family, &typography);

        assert_eq!(routes.len(), 2, "{family}");
        for property in [
            ThemeTypographyProperty::FontStack,
            ThemeTypographyProperty::FontSize,
        ] {
            assert!(
                routes.iter().any(|route| {
                    route.mechanism() == FamilyThemeMechanism::BaseTypography(property)
                        && route.disposition() == FamilyThemeDisposition::TypedAdapter
                }),
                "{family} must directly own {property:?}"
            );
        }
    }
}
