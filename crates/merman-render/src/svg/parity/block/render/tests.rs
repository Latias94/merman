use super::*;
use crate::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use crate::text::DeterministicTextMeasurer;
use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use std::cell::Cell;
use std::fmt;
use std::ops::Range;

const BOUNDED_BLOCK_SOURCE: &str = r#"block
  A["Alpha"] --> B["Beta"]
  classDef branded fill:#696,stroke:#333
  class A branded
"#;

fn layout_edge(id: &str) -> LayoutEdge {
    LayoutEdge {
        id: id.to_string(),
        from: "from".to_string(),
        to: "to".to_string(),
        from_cluster: None,
        to_cluster: None,
        points: Vec::new(),
        label: None,
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: None,
        end_marker: None,
        stroke_dasharray: None,
    }
}

#[test]
fn block_layout_edge_index_preserves_first_match_and_missing_behavior() {
    let edges = [
        layout_edge("duplicate"),
        layout_edge("other"),
        layout_edge("duplicate"),
    ];
    let index = BlockLayoutEdgeIndex::new(&edges);

    assert!(std::ptr::eq(
        index.get("duplicate").expect("duplicate edge lookup"),
        &edges[0],
    ));
    assert!(std::ptr::eq(
        index.get("other").expect("unique edge lookup"),
        &edges[1],
    ));
    assert!(index.get("missing").is_none());
}

fn render_block_direct_with_policy(
    policy: RenderResourcePolicy,
) -> crate::Result<root_svg::RootedSvg> {
    render_block_direct_with_layout(policy, &BlockNodePaintThemePlan::baseline(), |_| {})
}

fn render_block_direct_with_layout(
    policy: RenderResourcePolicy,
    node_paint_theme: &BlockNodePaintThemePlan,
    change_layout: impl FnOnce(&mut crate::model::BlockDiagramLayout),
) -> crate::Result<root_svg::RootedSvg> {
    render_block_direct_with_edge_theme(policy, node_paint_theme, None, change_layout)
}

fn render_block_direct_with_edge_theme(
    policy: RenderResourcePolicy,
    node_paint_theme: &BlockNodePaintThemePlan,
    theme: Option<&crate::diagram_theme::DiagramTheme>,
    change_layout: impl FnOnce(&mut crate::model::BlockDiagramLayout),
) -> crate::Result<root_svg::RootedSvg> {
    let resolved = theme.map(|theme| theme.resolve(crate::DiagramFamilyId::BLOCK));
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(BOUNDED_BLOCK_SOURCE, ParseOptions::strict())
        .expect("parse Block resource-bound fixture")
        .expect("detect Block resource-bound fixture");
    let RenderSemanticModel::Block(model) = parsed.model() else {
        panic!("expected a typed Block render model");
    };
    let effective_config = parsed.metadata().effective_config.as_value();
    let mut layout = crate::block::layout_block_diagram_typed(
        model,
        effective_config,
        &DeterministicTextMeasurer::default(),
    )?;
    change_layout(&mut layout);
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let request = SvgRenderOptions::default();
    let debug = SvgDebugOptions::default();
    let execution =
        SvgExecution::unthemed_for_test(&request, &debug, &session, crate::DiagramFamilyId::BLOCK)
            .expect("SVG execution");
    let typography_theme = BlockTypographyThemePlan::resolve(
        None,
        &merman_core::MermaidConfig::from_value(effective_config.clone()),
    )
    .unwrap();

    let labels = crate::block::BlockNodeLabelPaintPlan::resolve(
        None,
        &parsed.metadata().effective_config,
        model,
        &layout,
        execution.work_meter(),
    )
    .unwrap();
    let prepared;
    let node_paint_theme = if node_paint_theme.has_terminal_bindings() {
        node_paint_theme
    } else {
        prepared = BlockNodePaintThemePlan::resolve(
            None,
            &parsed.metadata().effective_config,
            model,
            &layout,
            &labels,
            execution.work_meter(),
        )
        .unwrap();
        &prepared
    };

    render_block_diagram_svg_model_with_theme(
        &layout,
        model,
        node_paint_theme,
        &labels,
        &crate::block::BlockEdgePaintPlan::resolve(
            resolved.as_ref(),
            &merman_core::MermaidConfig::from_value(effective_config.clone()),
            model,
            &layout,
            execution.work_meter(),
        )
        .expect("edge paint theme"),
        &crate::block::BlockMarkerPaintPlan::resolve(
            resolved.as_ref(),
            &merman_core::MermaidConfig::from_value(effective_config.clone()),
            model,
            &layout,
            execution.work_meter(),
        )
        .expect("marker paint theme"),
        &BlockLabelBackgroundPlan::resolve(
            None,
            &merman_core::MermaidConfig::from_value(effective_config.clone()),
            &typography_theme.css_binding().edge_label_background,
            execution.work_meter(),
        )
        .expect("background theme"),
        &typography_theme,
        &execution,
    )
}

#[test]
fn block_unthemed_renderer_preserves_node_integrity_errors() {
    for case in [
        "duplicate",
        "missing-source",
        "missing-geometry",
        "both-missing",
    ] {
        let error = render_block_direct_with_layout(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            &BlockNodePaintThemePlan::baseline(),
            |layout| match case {
                "duplicate" => layout.nodes.push(layout.nodes[0].clone()),
                "missing-source" => layout.nodes[0].id = "missing-source".to_string(),
                "missing-geometry" => layout.shape_geometries.clear(),
                "both-missing" => {
                    layout.nodes[0].id = "missing-source".to_string();
                    layout.shape_geometries.clear();
                }
                _ => unreachable!(),
            },
        )
        .expect_err("malformed unthemed Block layout must fail");
        let crate::Error::InvalidModel { message } = error else {
            panic!("expected Block InvalidModel for {case}, got {error}");
        };
        if matches!(case, "missing-geometry" | "both-missing") {
            assert!(message.starts_with("missing Block shape geometry for node `"));
        } else {
            assert_eq!(
                message,
                "Block node shell paint terminal receipt was incomplete"
            );
        }
    }
}

#[test]
fn block_duplicate_node_keeps_svg_limit_error_before_terminal_error() {
    let baseline =
        render_block_direct_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .expect("render unbounded Block SVG");
    let error = render_block_direct_with_layout(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, baseline.len())
            .expect("valid Block SVG ceiling"),
        &BlockNodePaintThemePlan::baseline(),
        |layout| layout.nodes.push(layout.nodes[0].clone()),
    )
    .expect_err("duplicate node output must still reach the SVG ceiling first");
    let crate::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Block MaxSvgBytes rejection before terminal validation, got {error}");
    };
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
}

#[test]
fn block_unthemed_renderer_rejects_reusing_a_completed_paint_plan() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(BOUNDED_BLOCK_SOURCE, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let RenderSemanticModel::Block(model) = parsed.model() else {
        panic!("Block model")
    };
    let layout = crate::block::layout_block_diagram_typed(
        model,
        parsed.metadata().effective_config.as_value(),
        &DeterministicTextMeasurer::default(),
    )
    .unwrap();
    let work = crate::resources::OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    );
    let labels = crate::block::BlockNodeLabelPaintPlan::resolve(
        None,
        &parsed.metadata().effective_config,
        model,
        &layout,
        &work,
    )
    .unwrap();
    let plan = BlockNodePaintThemePlan::resolve(
        None,
        &parsed.metadata().effective_config,
        model,
        &layout,
        &labels,
        &work,
    )
    .unwrap();
    render_block_direct_with_layout(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        &plan,
        |_| {},
    )
    .expect("first Block render completes");
    let error = render_block_direct_with_layout(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        &plan,
        |_| {},
    )
    .expect_err("the same Block plan cannot complete twice");
    let crate::Error::InvalidModel { message } = error else {
        panic!("expected Block terminal receipt rejection, got {error}");
    };
    assert_eq!(
        message,
        "Block node shell paint terminal receipt was incomplete"
    );
}

#[derive(Default)]
struct RejectEscapedAmpersand {
    rejected: bool,
    writes_after_rejection: usize,
    retained: String,
}

impl RejectEscapedAmpersand {
    fn record_write(&mut self, value: &str) -> fmt::Result {
        if self.rejected {
            self.writes_after_rejection += 1;
            return Err(fmt::Error);
        }
        if value == "&amp;" {
            self.rejected = true;
            return Err(fmt::Error);
        }
        self.retained.push_str(value);
        Ok(())
    }
}

impl fmt::Write for RejectEscapedAmpersand {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.record_write(value)
    }
}

impl SvgOutput for RejectEscapedAmpersand {
    fn push_str(&mut self, value: &str) {
        let _ = self.record_write(value);
    }

    fn push(&mut self, value: char) {
        let mut encoded = [0u8; 4];
        let _ = self.record_write(value.encode_utf8(&mut encoded));
    }

    fn len(&self) -> usize {
        self.retained.len()
    }

    fn as_str(&self) -> &str {
        self.retained.as_str()
    }

    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
        self.retained.replace_range(range, replacement);
        Ok(())
    }

    fn checkpoint(&mut self) -> crate::Result<()> {
        if self.rejected {
            Err(crate::Error::InvalidModel {
                message: "test SVG sink rejected the escaped ampersand".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

#[test]
fn block_important_declarations_stop_at_first_svg_sink_failure() {
    let polls = Cell::new(0);
    let mut out = RejectEscapedAmpersand::default();
    let declarations = [("fill", "alpha&beta<gamma"), ("stroke", "red")]
        .into_iter()
        .inspect(|_| polls.set(polls.get() + 1));

    let error = write_important_declarations(&mut out, declarations)
        .expect_err("the rejecting sink must stop Block class declaration emission");

    assert!(matches!(error, crate::Error::InvalidModel { .. }));
    assert_eq!(polls.get(), 1, "remaining declarations must not be parsed");
    assert_eq!(
        out.writes_after_rejection, 0,
        "escaped output must stop at the first failed sink write"
    );
}

#[test]
fn block_svg_sink_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let baseline =
        render_block_direct_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .expect("render unbounded Block SVG directly");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1);

    let exact = render_block_direct_with_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact Block SVG ceiling"),
    )
    .expect("exact Block SVG sink ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let error = render_block_direct_with_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid below-exact Block SVG ceiling"),
    )
    .expect_err("one byte below the direct Block SVG size must fail inside the renderer");
    let crate::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Block MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn block_typed_edge_writer_rejects_missing_duplicate_and_mismatched_layout() {
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#123456").unwrap()),
                    )
                    .for_family(crate::DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .unwrap();
    for case in ["missing", "duplicate", "wrong-endpoint"] {
        let error = render_block_direct_with_edge_theme(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            &BlockNodePaintThemePlan::baseline(),
            Some(&theme),
            |layout| match case {
                "missing" => layout.edges.clear(),
                "duplicate" => layout.edges.push(layout.edges[0].clone()),
                "wrong-endpoint" => layout.edges[0].to = "wrong".to_string(),
                _ => unreachable!(),
            },
        )
        .expect_err("typed edge receipt must reject malformed layout");
        assert!(
            error
                .to_string()
                .contains("Block edge paint terminal receipt"),
            "{case}: {error}"
        );
    }
}

#[test]
fn block_base_marker_definitions_preserve_historical_bytes() {
    use sha2::{Digest, Sha256};
    let rendered =
        render_block_direct_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .unwrap();
    let svg: &str = &rendered;
    let document = roxmltree::Document::parse(svg).unwrap();
    let markers = document
        .descendants()
        .filter(|node| node.has_tag_name("marker"))
        .collect::<Vec<_>>();
    assert_eq!(markers.len(), 12);
    let range = markers.first().unwrap().range().start..markers.last().unwrap().range().end;
    assert_eq!(
        data_encoding::HEXLOWER.encode(&Sha256::digest(svg[range].as_bytes())),
        "f1a0eb49e563cc64e92401557a6fe52b4a9ba12b56b2a3ca2089a2f5f63dfa62",
        "the original twelve definitions retain their exact bytes and order"
    );
}

#[test]
fn block_marker_writer_rejects_missing_duplicate_and_mismatched_layout() {
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )
                    .for_family(crate::DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .unwrap();
    for case in ["missing", "duplicate", "wrong-endpoint"] {
        let error = render_block_direct_with_edge_theme(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            &BlockNodePaintThemePlan::baseline(),
            Some(&theme),
            |layout| match case {
                "missing" => layout.edges.clear(),
                "duplicate" => layout.edges.push(layout.edges[0].clone()),
                "wrong-endpoint" => layout.edges[0].to = "wrong".to_string(),
                _ => unreachable!(),
            },
        )
        .expect_err("marker receipt must reject malformed layout");
        assert!(
            error
                .to_string()
                .contains("Block marker paint terminal receipt"),
            "{case}: {error}"
        );
    }
}
