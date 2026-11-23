use super::*;
use merman_core::{MermaidEngine, ParseOptions};

#[test]
fn flowchart_layout_only_preserves_geometry_and_preparation_admission() {
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    let source = "\
flowchart LR
  subgraph Group[Review queue]
    A[Open] -->|submit| B[Review]
  end
  B --> C[Done]
  style B width:220px,height:90px
  classDef default font-size:18px,stroke-width:3px
  classDef cluster fill:#eee,stroke:#000
";
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let options = LayoutOptions::default();
    let render_session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let paint_before_render = crate::svg::FlowchartPreparedNodes::paint_table_entries_built();
    let render_work = Arc::clone(render_session.work_meter());
    let reference = prepare(&parsed, &options, render_session).unwrap();
    let paint_after_render = crate::svg::FlowchartPreparedNodes::paint_table_entries_built();
    assert_eq!(paint_after_render - paint_before_render, 4);
    let layout_session = || {
        crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .unwrap()
    };
    let session = layout_session();
    let layout_work = Arc::clone(session.work_meter());
    let layout = prepare_layout(&parsed, &options, session).unwrap();
    let BuiltinFamilyArtifact::Flowchart(flowchart) = &layout.0.family else {
        panic!("expected Flowchart family artifact");
    };
    assert!(flowchart.prepared_nodes.is_none());
    assert_eq!(
        crate::svg::FlowchartPreparedNodes::paint_table_entries_built(),
        paint_after_render,
    );
    assert_eq!(
        layout.layout_json().unwrap(),
        reference.layout_json().unwrap()
    );
    assert_eq!(
        layout.layout_geometry_json().unwrap(),
        reference.layout_geometry_json().unwrap(),
    );
    assert_eq!(layout_work.used(), render_work.used());
    assert_eq!(
        layout_work.prepared_text_retained_bytes_peak(),
        render_work.prepared_text_retained_bytes_peak(),
    );

    for (id, required) in [
        (ResourceLimitId::MaxLayoutWorkUnits, render_work.used()),
        (
            ResourceLimitId::MaxPreparedTextRetainedBytes,
            render_work.prepared_text_retained_bytes_peak(),
        ),
    ] {
        let limited_session = || {
            crate::environment::RenderEnvironment::deterministic()
                .with_resource_policy(policy.with_limit(id, required - 1).unwrap())
                .begin_session()
                .unwrap()
        };
        let Err(Error::ResourceLimitExceeded(reference)) =
            prepare(&parsed, &options, limited_session())
        else {
            panic!("expected render preparation to exceed {id:?}");
        };
        let Err(Error::ResourceLimitExceeded(layout)) =
            prepare_layout(&parsed, &options, limited_session())
        else {
            panic!("expected layout preparation to exceed {id:?}");
        };
        assert_eq!(layout, reference);
    }
}
