use super::*;
use crate::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};

fn render_info_with_policy(policy: RenderResourcePolicy) -> crate::Result<root_svg::RootedSvg> {
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("render session");
    let request = SvgRenderOptions::default();
    let debug = SvgDebugOptions::default();
    let effective_config = merman_model::presentation::PresentationConfig::default();
    let measurer = crate::text::DeterministicTextMeasurer::default();
    let (layout, appearance) =
        crate::info::prepare_info_diagram(&effective_config, None, &measurer);
    assert!(matches!(
        &appearance,
        crate::info::InfoAppearance::Mermaid(_)
    ));
    let execution =
        SvgExecution::unthemed_for_test(&request, &debug, &session, crate::DiagramFamilyId::INFO)
            .expect("SVG execution");
    let diagram_id = execution.diagram_id_or("merman");
    crate::info::render_info_svg(
        &layout,
        &appearance,
        diagram_id,
        &execution.root_viewport(crate::DiagramFamilyId::INFO, diagram_id),
        execution.work_meter(),
    )
}

#[test]
fn info_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let baseline = render_info_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .expect("render unbounded Info SVG");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1);

    let exact = render_info_with_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact SVG ceiling"),
    )
    .expect("exact Info SVG ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let error = render_info_with_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid short SVG ceiling"),
    )
    .expect_err("one byte below the Info SVG size must fail");
    let crate::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Info MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
}
