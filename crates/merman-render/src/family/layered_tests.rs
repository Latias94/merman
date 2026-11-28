//! Layout retry and resource accounting through the family boundary.

#[cfg(feature = "layout-dagre")]
use crate::resources::{RenderResourcePolicy, ResourceLimitId};
use crate::{
    Error, LayoutOptions, RenderCapability, RenderCapabilityPolicy, environment::RenderEnvironment,
    family,
};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::{
    OperationControl,
    presentation::{PresentationDiagram, RenderSemanticModel},
    resources::{InputResourcePolicy, ResourceProfile},
};

fn duplicate_edge_model(layout: &str) -> PresentationDiagram {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            &format!(
                "---\nconfig:\n  layout: {layout}\n  htmlLabels: false\n---\nflowchart LR\nA[Read] one@-->|First| B[Check]\nA two@-->|Second| C[Keep]\n"
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Flowchart(mut model) = parsed.model().clone() else {
        panic!("expected a Flowchart model");
    };
    model.edges[1].id = model.edges[0].id.clone();
    PresentationDiagram::try_from_frozen(
        parsed.metadata().clone(),
        RenderSemanticModel::Flowchart(model),
        parsed.render_context().clone(),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

#[test]
fn layered_does_not_retry_without_dagre_admission() {
    let parsed = duplicate_edge_model("layered");
    let policy = RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutDagre);
    let session = RenderEnvironment::deterministic()
        .with_capability_policy(policy)
        .begin_session()
        .unwrap();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(Error::MissingCapability {
            capability: RenderCapability::LayoutDagre,
            ..
        })
    ));
}

#[cfg(feature = "layout-dagre")]
#[test]
fn layered_retry_restarts_dagre_and_keeps_the_cumulative_ceiling() {
    let options = LayoutOptions::default();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let session = |policy| {
        RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .unwrap()
    };
    let layered = duplicate_edge_model("layered");
    let dagre = duplicate_edge_model("dagre");
    let layered_session = session(policy);
    let retry_meter = layered_session.work_meter().clone();
    let artifact = family::prepare(&layered, &options, layered_session).unwrap();
    let retry_work = retry_meter.used();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();

    let dagre_session = session(policy);
    let dagre_meter = dagre_session.work_meter().clone();
    let artifact = family::prepare(&dagre, &options, dagre_session).unwrap();
    assert!(retry_work > dagre_meter.used());
    let direct = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let marker = " data-layout-fallback=\"dagre\"";
    assert!(svg.svg().contains(marker));
    assert_eq!(svg.svg().replacen(marker, "", 1), direct.svg());

    let limited = policy
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, retry_work - 1)
        .unwrap();
    let short_session = session(limited);
    let short_meter = short_session.work_meter().clone();
    assert!(matches!(
        family::prepare(&layered, &options, short_session),
        Err(Error::ResourceLimitExceeded(_))
    ));
    assert_eq!(short_meter.prepared_text_retained_bytes(), 0);
}
