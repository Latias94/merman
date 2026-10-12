#![cfg(feature = "native-fonts")]

use merman::svg::export::{NativeFontContext, NativeFontOptions};
use merman::svg::{TextMeasurementOperation, TextMeasurementPhase, TextMeasurementSource};
use merman::{
    OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgOutput, SvgRequest,
};

fn font_context() -> NativeFontContext {
    let fonts =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../merman-export/tests/fonts");
    NativeFontOptions::default()
        .with_font_file(fonts.join("Cousine-Regular.ttf"))
        .with_font_file(fonts.join("Cousine-Bold.ttf"))
        .with_font_file(fonts.join("Cousine-Italic.ttf"))
        .load(&OperationControl::new())
        .expect("the checked-in explicit font set loads")
}

fn render(source: &str, context: Option<NativeFontContext>) -> SvgOutput {
    let environment = match context {
        Some(context) => SvgEnvironment::deterministic()
            .with_native_font_context(context)
            .expect("native font environment"),
        None => SvgEnvironment::deterministic(),
    };
    match Renderer::new()
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest {
                environment,
                ..Default::default()
            },
        ))
        .unwrap_or_else(|error| panic!("render failed for {source:?}: {error}"))
    {
        RenderOutput::Svg(Some(output)) => output,
        other => panic!("expected a rendered diagram, got {other:?}"),
    }
}

fn assert_native_without_fallback(output: &SvgOutput, family: &str) {
    let entries = output.evidence().measurement().entries();
    assert!(!entries.is_empty(), "{family} must measure non-title text");
    assert!(
        entries
            .iter()
            .any(|entry| entry.provenance().operation != TextMeasurementOperation::TitleBBoxX),
        "{family} must use native measurement outside diagram titles: {entries:?}"
    );
    for entry in entries {
        assert_eq!(
            entry.provenance().source,
            TextMeasurementSource::Host,
            "{family}: {entry:?}"
        );
        assert_eq!(
            entry.provenance().fallback_reason,
            None,
            "{family}: {entry:?}"
        );
    }
}

fn layout(source: &str, context: Option<NativeFontContext>) -> serde_json::Value {
    let environment = match context {
        Some(context) => SvgEnvironment::deterministic()
            .with_native_font_context(context)
            .expect("native font environment"),
        None => SvgEnvironment::deterministic(),
    };
    match Renderer::new()
        .render(RenderRequest::layout_json(
            source,
            OperationControl::new(),
            SvgRequest {
                environment,
                ..Default::default()
            },
        ))
        .expect("layout inspection succeeds")
    {
        RenderOutput::LayoutJson(Some(output)) => output.layout()["layout"].clone(),
        other => panic!("expected layout JSON, got {other:?}"),
    }
}

#[test]
fn explicit_font_metrics_change_text_driven_geometry_beyond_titles() {
    let cases: &[(&str, &str)] = &[
        #[cfg(feature = "diagram-flowchart")]
        (
            "flowchart",
            "flowchart LR\nA[WWWWWWWWWWWWWWWWWWWWWWWW] -->|WWWWWWWWWWWW| B[Destination]\n",
        ),
        #[cfg(feature = "diagram-class")]
        (
            "class",
            "classDiagram\nclass WWWWWWWWWWWWWWWWWWWWWWWW {\n+String WWWWWWWWWWWWWWWW\n}\n",
        ),
        #[cfg(feature = "diagram-er")]
        (
            "er",
            "erDiagram\nWWWWWWWWWWWWWWWWWWWWWWWW ||--o{ ORDER : WWWWWWWWWWWWWWWW\n",
        ),
        #[cfg(feature = "diagram-sequence")]
        (
            "sequence",
            "sequenceDiagram\nparticipant WWWWWWWWWWWWWWWWWWWWWWWW\nparticipant Compiler\nWWWWWWWWWWWWWWWWWWWWWWWW->>Compiler: WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW\n",
        ),
    ];
    let context = font_context();
    for (family, source) in cases {
        assert_ne!(
            layout(source, None),
            layout(source, Some(context.clone())),
            "{family} geometry must depend on the explicit font, not estimated glyph widths"
        );
    }
}

#[cfg(feature = "diagram-timeline")]
#[test]
fn timeline_wrap_probes_use_native_computed_length() {
    let output = render(
        "timeline\nsection Preparation\n2024 : A long event label containing several words that must be measured before wrapping\n",
        Some(font_context()),
    );
    assert_native_without_fallback(&output, "timeline");
    assert!(
        output
            .evidence()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                let provenance = entry.provenance();
                provenance.operation == TextMeasurementOperation::ComputedLength
                    && provenance.phase == TextMeasurementPhase::ComputedLength
            }),
        "timeline must use the routed computed-length phase"
    );
}

#[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
#[test]
fn architecture_uses_native_canvas_advances() {
    let output = render(
        "architecture-beta\nservice worker [Worker service with a long label]\n",
        Some(font_context()),
    );
    assert_native_without_fallback(&output, "architecture");
    assert!(
        output
            .evidence()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                let provenance = entry.provenance();
                provenance.operation == TextMeasurementOperation::CanvasMeasureTextWidth
                    && provenance.phase == TextMeasurementPhase::Layout
            }),
        "architecture must use native Canvas-compatible advances"
    );
}

#[cfg(feature = "diagram-info")]
#[test]
fn info_native_viewport_measurement_preserves_default_geometry() {
    let output = render("info", Some(font_context()));
    assert_native_without_fallback(&output, "info");
    let entries = output.evidence().measurement().entries();
    assert_eq!(
        entries.len(),
        2,
        "Info measures only the version text bounds"
    );
    for operation in [
        TextMeasurementOperation::BBoxX,
        TextMeasurementOperation::RawBBoxHeight,
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.provenance().operation == operation)
            .expect("Info viewport records each version text bound");
        assert_eq!(entry.provenance().phase, TextMeasurementPhase::SvgBBox);
        assert_eq!(entry.count(), 1);
    }
    let document = roxmltree::Document::parse(output.svg()).expect("valid info SVG");
    assert_eq!(document.root_element().attribute("viewBox"), None);
    let native_layout = layout("info", Some(font_context()));
    assert_eq!(native_layout, layout("info", None));
    assert_eq!(
        native_layout["InfoDiagram"]["bounds"],
        serde_json::json!({
            "min_x": 0.0,
            "min_y": 0.0,
            "max_x": 400.0,
            "max_y": 100.0,
        })
    );
}

#[test]
fn legend_consumers_use_native_client_rect_widths() {
    let cases: &[(&str, &str)] = &[
        #[cfg(feature = "diagram-pie")]
        (
            "pie",
            "pie\n\"Completed tasks\": 40\n\"Pending tasks\": 60\n",
        ),
        #[cfg(feature = "diagram-journey")]
        (
            "journey",
            "journey\nsection Preparation\nCompile source: 5: Developer with a long name\n",
        ),
    ];
    let context = font_context();
    for (family, source) in cases {
        let output = render(source, Some(context.clone()));
        assert_native_without_fallback(&output, family);
        assert!(
            output
                .evidence()
                .measurement()
                .entries()
                .iter()
                .any(|entry| {
                    entry.provenance().operation
                        == TextMeasurementOperation::BoundingClientRectWidth
                }),
            "{family} legend must use native client-rect widths"
        );
    }
}

#[cfg(feature = "diagram-treemap")]
#[test]
fn treemap_emission_uses_native_computed_length_for_fit_decisions() {
    let output = render(
        "treemap\n\"Root\"\n \"Completed tasks with a long descriptive label\": 40\n \"Pending tasks\": 60\n",
        Some(font_context()),
    );
    assert_native_without_fallback(&output, "treemap");
    assert!(
        output
            .evidence()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                let provenance = entry.provenance();
                provenance.operation == TextMeasurementOperation::ComputedLength
                    && provenance.phase == TextMeasurementPhase::ComputedLength
            }),
        "treemap text-fit decisions must use native advances during emission"
    );
}

#[test]
fn every_compiled_text_driven_family_uses_native_measurement_without_titles() {
    let cases: &[(&str, &str)] = &[
        #[cfg(feature = "diagram-agentflow")]
        (
            "agentflow",
            "agentflow-beta TB\nflow support[\"Support\"]\n input[\"Question\"]@{ shape: input }\n task[\"Resolve\"]@{ shape: task }\n input --> task\nend\n",
        ),
        #[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
        (
            "architecture",
            "architecture-beta\nservice worker [Worker]\n",
        ),
        #[cfg(feature = "diagram-block")]
        (
            "block",
            "block-beta\ncolumns 2\nA[\"First block\"] B[\"Second block\"]\n",
        ),
        #[cfg(feature = "diagram-c4")]
        (
            "c4",
            "C4Context\nPerson(customer, \"Customer\", \"A customer\")\nSystem(bank, \"Banking System\", \"Does banking\")\nRel(customer, bank, \"Uses\")\n",
        ),
        #[cfg(feature = "diagram-class")]
        (
            "class",
            "classDiagram\nclass Animal {\n+String name\n+eat()\n}\nclass Dog\nAnimal <|-- Dog\n",
        ),
        #[cfg(feature = "diagram-cynefin")]
        (
            "cynefin",
            "cynefin-beta\nclear\n \"Runbook\"\ncomplex\n \"Retrospective\"\nclear --> complex : \"Probe\"\n",
        ),
        #[cfg(feature = "diagram-er")]
        (
            "er",
            "erDiagram\nCUSTOMER ||--o{ ORDER : places\nCUSTOMER {\nstring name\n}\n",
        ),
        #[cfg(feature = "diagram-event-modeling")]
        (
            "eventmodeling",
            "eventmodeling\ntf 01 ui Shop.Cart\ntf 02 cmd Ordering.AddItem ->> 01 { sku: \"SKU-1\" }\n",
        ),
        #[cfg(feature = "diagram-flowchart")]
        (
            "flowchart",
            "flowchart LR\nA[Start process] -->|Continue work| B[Finish process]\n",
        ),
        #[cfg(feature = "diagram-gantt")]
        (
            "gantt",
            "gantt\ndateFormat YYYY-MM-DD\nsection Preparation\nCompile source :a1, 2024-01-01, 1d\n",
        ),
        #[cfg(feature = "diagram-git-graph")]
        (
            "gitgraph",
            "gitGraph:\ncommit id: \"Initial change\"\nbranch feature\ncheckout feature\ncommit id: \"Feature change\"\n",
        ),
        #[cfg(feature = "diagram-ishikawa")]
        (
            "ishikawa",
            "ishikawa-beta\nBlurry Photo\n Process\n  Out of focus\n User\n  Wrong position\n",
        ),
        #[cfg(feature = "diagram-journey")]
        (
            "journey",
            "journey\nsection Preparation\nCompile source: 5: Developer\n",
        ),
        #[cfg(feature = "diagram-kanban")]
        ("kanban", "kanban\n todo[Todo]\n  task[Compile source]\n"),
        #[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
        ("mindmap", "mindmap\n Root\n  First child\n  Second child\n"),
        #[cfg(feature = "diagram-pie")]
        (
            "pie",
            "pie\n\"Completed tasks\": 40\n\"Pending tasks\": 60\n",
        ),
        #[cfg(feature = "diagram-railroad")]
        (
            "railroad",
            "railroad-beta\nrule = sequence(terminal(\"hello\"), nonterminal(\"other\")) ;\nother = terminal(\"world\") ;\n",
        ),
        #[cfg(feature = "diagram-requirement")]
        (
            "requirement",
            "requirementDiagram\nrequirement req1 {\nid: 1\ntext: User login\nrisk: high\nverifymethod: analysis\n}\n",
        ),
        #[cfg(feature = "diagram-sankey")]
        ("sankey", "sankey-beta\nSource,Destination,10\n"),
        #[cfg(feature = "diagram-sequence")]
        (
            "sequence",
            "sequenceDiagram\nparticipant Developer\nparticipant Compiler\nDeveloper->>Compiler: Compile source\nCompiler-->>Developer: Build finished\n",
        ),
        #[cfg(feature = "diagram-state")]
        (
            "state",
            "stateDiagram-v2\n[*] --> Idle\nIdle --> Active: start process\nActive --> Idle: stop process\n",
        ),
        #[cfg(feature = "diagram-swimlane")]
        (
            "swimlane",
            "swimlane-beta LR\nA[Start process] --> B[Finish process]\n",
        ),
        #[cfg(feature = "diagram-timeline")]
        (
            "timeline",
            "timeline\nsection Preparation\n2024 : First release\n2025 : Second release\n",
        ),
        #[cfg(feature = "diagram-tree-view")]
        (
            "treeview",
            "treeView-beta\nsrc/\n index.js\n App.tsx\npackage.json\n",
        ),
        #[cfg(feature = "diagram-treemap")]
        (
            "treemap",
            "treemap\n\"Root\"\n \"Completed tasks\": 40\n \"Pending tasks\": 60\n",
        ),
        #[cfg(feature = "diagram-usecase")]
        (
            "usecase",
            "usecase-beta\nactor Customer(\"Customer\")\nsystemBoundary \"Order system\"\n Checkout(\"Place order\")\nend\nCustomer --> Checkout\n",
        ),
        #[cfg(feature = "diagram-wardley")]
        (
            "wardley",
            "wardley-beta\nsize [1100, 800]\nanchor Business [0.95, 0.63]\ncomponent Cup of Tea [0.79, 0.61]\nBusiness -> Cup of Tea\nannotations [1, 0]\nannotation 1,[0.57, 0.16] \"Alternative approaches in research\"\n",
        ),
        #[cfg(feature = "diagram-xychart")]
        (
            "xychart",
            "xychart-beta\nx-axis [Jan, Feb, Mar]\ny-axis \"Revenue\" 0 --> 100\nbar [20, 40, 80]\n",
        ),
        #[cfg(feature = "diagram-zenuml")]
        ("zenuml", "zenuml\nAlice->Bob: Hello\nBob-->Alice: Reply\n"),
    ];
    let context = font_context();
    for (family, source) in cases {
        let output = render(source, Some(context.clone()));
        assert_native_without_fallback(&output, family);
    }
}

#[test]
fn fixed_geometry_families_do_not_invent_body_measurement() {
    let cases: &[(&str, &str)] = &[
        #[cfg(feature = "diagram-packet")]
        ("packet", "packet-beta\n0-7: \"DATA\"\n8-15: \"TYPE\"\n"),
        #[cfg(feature = "diagram-quadrant-chart")]
        (
            "quadrantchart",
            "quadrantChart\nx-axis Low --> High\ny-axis Low --> High\nquadrant-1 Advance\nquadrant-2 Evaluate\nquadrant-3 Reconsider\nquadrant-4 Improve\nTask A: [0.3, 0.6]\n",
        ),
        #[cfg(feature = "diagram-radar")]
        (
            "radar",
            "radar-beta\naxis Quality,Delivery,Support\ncurve score{1,2,3}\n",
        ),
        #[cfg(feature = "diagram-venn")]
        (
            "venn",
            "venn-beta\nset A[\"Core\"]:20\nset B[\"Editor\"]:14\nunion A,B[\"Shared\"]:4\n",
        ),
    ];
    let context = font_context();
    for (family, source) in cases {
        let output = render(source, Some(context.clone()));
        assert!(
            output.evidence().measurement().entries().is_empty(),
            "{family} uses source-defined fixed geometry"
        );
    }
}
