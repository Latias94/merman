#![cfg(all(
    feature = "layout-shoal",
    any(
        feature = "diagram-architecture",
        feature = "diagram-c4",
        feature = "diagram-class",
        feature = "diagram-er",
        feature = "diagram-requirement",
        feature = "diagram-state",
        feature = "diagram-usecase"
    )
))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::{LayoutAdaptation, LayoutOptions, environment::RenderEnvironment, family};

#[test]
fn shoal_families_fit_width_and_capture_frames_through_the_normal_svg_pipeline() {
    let fixtures = [
        #[cfg(feature = "diagram-architecture")]
        (
            "architecture",
            "architecture-beta\nservice api(server)[API]\nservice db(database)[Database]\napi:R --> L:db",
        ),
        #[cfg(feature = "diagram-c4")]
        (
            "c4",
            "C4Context\nPerson(user, \"Customer\")\nSystem(orders, \"Orders\")\nRel(user, orders, \"Submits\")",
        ),
        #[cfg(feature = "diagram-class")]
        ("class", "classDiagram\nA --> B"),
        #[cfg(feature = "diagram-er")]
        ("er", "erDiagram\nA ||--o{ B : contains"),
        #[cfg(feature = "diagram-requirement")]
        (
            "requirement",
            "requirementDiagram\nelement A {\ntype: hardware\n}\nelement B {\ntype: software\n}\nA - contains -> B",
        ),
        #[cfg(feature = "diagram-state")]
        ("state", "stateDiagram-v2\nA --> B"),
        #[cfg(feature = "diagram-usecase")]
        (
            "usecase",
            "usecase-beta\nactor A(\"Operator\")\nB(\"Review\")\nA --> B",
        ),
    ];
    for (name, source) in fixtures {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                &format!("---\nconfig:\n  layout: shoal\n---\n{source}"),
                ParseOptions::strict(),
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .unwrap()
            .into_presentation();
        let render = |options: &LayoutOptions| {
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert!(
                plan.supported_layout_adaptations()
                    .contains(&LayoutAdaptation::FitWidth)
            );
            let svg = family::prepare(&parsed, options, session)
                .and_then(|artifact| artifact.render_svg(&Default::default(), &Default::default()))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let frame = svg.shoal_layout_frame().unwrap();
            (svg.into_completion(), frame)
        };

        let mut options = LayoutOptions::default().with_shoal_layout_frame(None);
        options.adaptation = LayoutAdaptation::FitWidth;
        options.container_width = 1.0;
        let (first, frame) = render(&options);
        let observation = first.layout_adaptation().unwrap();
        assert_eq!(observation.requested_width(), 1.0, "{name}");
        assert!(observation.overflowed(), "{name}");
        let json = serde_json::to_value(frame.as_ref()).unwrap();
        assert_eq!(json["family"], name);
        assert_eq!(json["nodes"].as_object().unwrap().len(), 2, "{name}");
        let (again, _) = render(&options.with_shoal_layout_frame(Some(frame)));
        assert_eq!(again.layout_adaptation(), Some(observation), "{name}");
        assert_eq!(again.output(), first.output(), "{name}");
    }
}
