#[cfg(feature = "system-timing")]
use merman_core::runtime::RuntimePolicy;
use merman_core::{Engine, ParseOptions};
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{Error, LayoutOptions};

fn timing_debug_options() -> SvgDebugOptions {
    SvgDebugOptions {
        include_timing_diagnostics: true,
        ..SvgDebugOptions::default()
    }
}

fn supported_sources() -> &'static [&'static str] {
    &[
        "flowchart TD\nA --> B\n",
        "classDiagram\nclass A\n",
        "stateDiagram-v2\n[*] --> Active\n",
        #[cfg(feature = "cytoscape-layout")]
        "architecture-beta\n  service api(server)[API]\n",
        #[cfg(feature = "cytoscape-layout")]
        "mindmap\n  Root\n    Child\n",
    ]
}

fn prepare(source: &str, session: RenderSession) -> family::FamilyRenderArtifact {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse source")
        .expect("detect diagram");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare family artifact")
}

#[test]
fn deterministic_sessions_reject_requested_svg_timing_for_supported_entry_shapes() {
    for &source in supported_sources() {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        let error = match prepare(source, session)
            .render_svg(&SvgRenderOptions::default(), &timing_debug_options())
        {
            Ok(_) => panic!("timing diagnostics unexpectedly rendered for {source:?}"),
            Err(error) => error,
        };

        assert!(
            matches!(error, Error::OperationTimingUnavailable(_)),
            "unexpected timing rejection for {source:?}: {error:?}"
        );
    }
}

#[cfg(feature = "system-timing")]
#[test]
fn explicitly_enabled_system_timing_session_can_emit_svg_diagnostics() {
    let policy = RuntimePolicy::deterministic()
        .try_with_system_timing()
        .expect("compiled system timing adapter");

    for &source in supported_sources() {
        let session = RenderEnvironment::deterministic()
            .with_runtime_policy(policy.clone())
            .begin_session()
            .expect("timed render session");

        let timing = session
            .operation_timing()
            .expect("session exposes operation timing authority");
        let _elapsed = timing.start().elapsed();

        let rendered = prepare(source, session)
            .render_svg(&SvgRenderOptions::default(), &timing_debug_options())
            .expect("timing-enabled SVG render");

        assert!(rendered.svg().starts_with("<svg"), "source: {source:?}");
    }
}

#[cfg(feature = "system-timing")]
#[test]
fn unused_system_timing_authority_does_not_change_svg_output() {
    let source = "flowchart TD\nA --> B\n";
    let deterministic_session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let timed_session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            RuntimePolicy::deterministic()
                .try_with_system_timing()
                .expect("compiled system timing adapter"),
        )
        .begin_session()
        .expect("timed render session");

    let deterministic = prepare(source, deterministic_session)
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("deterministic SVG render");
    let timed = prepare(source, timed_session)
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("timing-capable SVG render");

    assert_eq!(deterministic.svg(), timed.svg());
}
