use super::*;
use crate::environment::RenderEnvironment;
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
    let mut typography_theme = BlockTypographyThemePlan::resolve(
        None,
        &merman_core::MermaidConfig::from_value(effective_config.clone()),
    )
    .unwrap();
    typography_theme.prepare_class_definitions(&model.class_defs);

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
fn block_prepared_class_css_keeps_shape_and_span_declaration_order() {
    let rendered =
        render_block_direct_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .expect("render prepared Block class CSS");
    let svg = rendered.as_str();
    let document = crate::xml_dom::Document::parse(svg).expect("Block class stylesheet XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Block logical class stylesheet");
    assert!(css.contains(" .branded>*{fill:#696!important;stroke:#333!important;}"));
    assert!(css.contains(" .branded span{fill:#696!important;stroke:#333!important;}"));
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

    let error = write_important_declarations(
        &mut crate::svg::parity::output::XmlTextOutput::new(&mut out),
        declarations,
    )
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
fn block_base_marker_definitions_preserve_historical_bytes() {
    use sha2::{Digest, Sha256};
    let rendered =
        render_block_direct_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .unwrap();
    let svg: &str = &rendered;
    let document = crate::xml_dom::Document::parse(svg).unwrap();
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
fn diagram_id_terminal_precedes_later_block_model_validation() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, 1)
        .expect("valid SVG byte limit");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("begin render session");
    let request = SvgRenderOptions {
        diagram_id: Some("terminal".to_string()),
        ..SvgRenderOptions::default()
    };
    let debug = SvgDebugOptions::default();
    let options =
        SvgExecution::unthemed_for_test(&request, &debug, &session, crate::DiagramFamilyId::BLOCK)
            .expect("SVG execution");
    let layout = BlockDiagramLayout {
        nodes: vec![LayoutNode {
            id: "node".to_string(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            is_cluster: false,
            label_width: None,
            label_height: None,
        }],
        edges: Vec::new(),
        shape_geometries: Vec::new(),
        bounds: None,
    };
    let parsed = merman_core::Engine::new()
        .parse_diagram_for_render_model_sync("block\nnode\n", merman_core::ParseOptions::strict())
        .expect("parse Block render model")
        .expect("Block input is detected");
    let merman_core::RenderSemanticModel::Block(model) = parsed.model() else {
        panic!("expected Block render model");
    };

    let config = &parsed.metadata().effective_config;
    let typography = BlockTypographyThemePlan::resolve(None, config).unwrap();
    let labels =
        BlockNodeLabelPaintPlan::resolve(None, config, model, &layout, options.work_meter())
            .unwrap();
    let edges = crate::block::BlockEdgePaintPlan::resolve(
        None,
        config,
        model,
        &layout,
        options.work_meter(),
    )
    .unwrap();
    let markers = crate::block::BlockMarkerPaintPlan::resolve(
        None,
        config,
        model,
        &layout,
        options.work_meter(),
    )
    .unwrap();
    let background = BlockLabelBackgroundPlan::resolve(
        None,
        config,
        &typography.css_binding().edge_label_background,
        options.work_meter(),
    )
    .unwrap();
    let error = render_block_diagram_svg_model_with_theme(
        &layout,
        model,
        &BlockNodePaintThemePlan::baseline(),
        &labels,
        &edges,
        &markers,
        &background,
        &typography,
        &options,
    )
    .expect_err("diagram-id projection must stop before missing geometry validation");
    let Error::ResourceLimitExceeded(details) = error else {
        panic!("expected SVG byte resource rejection");
    };
    assert_eq!(details.limit, ResourceLimitId::MaxSvgBytes.as_str());
}
