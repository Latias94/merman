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
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(BOUNDED_BLOCK_SOURCE, ParseOptions::strict())
        .expect("parse Block resource-bound fixture")
        .expect("detect Block resource-bound fixture");
    let RenderSemanticModel::Block(model) = parsed.model() else {
        panic!("expected a typed Block render model");
    };
    let effective_config = parsed.metadata().effective_config.as_value();
    let layout = crate::block::layout_block_diagram_typed(
        model,
        effective_config,
        &DeterministicTextMeasurer::default(),
    )?;
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let request = SvgRenderOptions::default();
    let debug = SvgDebugOptions::default();
    let execution =
        SvgExecution::unthemed_for_test(&request, &debug, &session, crate::DiagramFamilyId::BLOCK)
            .expect("SVG execution");
    let node_paint_theme = BlockNodePaintThemePlan::baseline(&layout);

    render_block_diagram_svg_model(
        &layout,
        model,
        &node_paint_theme,
        effective_config,
        &execution,
    )
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
