#![cfg(feature = "diagram-class")]

use merman_core::SemanticParser;
use merman_model::{families::class::ClassDiagramModel, semantic::SemanticDiagramModel};

const SOURCE: &str = concat!(
    "---\r\ntitle: Types\r\n---\r\nclassDiagram\r\n",
    "    class A\r\n    cssClass \"A\" accent\r\n",
    "    %% removed comment 🦀\r\n    title Overview\r\n",
    "    classDef accent fill:red\r\n",
);

#[test]
fn ignored_statement_preserves_the_v010_class_model_literal_and_style_evidence() {
    let parser = SemanticParser::new();
    let parse = |source: &str| match parser.parse_diagram_sync(source).unwrap().model {
        SemanticDiagramModel::Class(model) => model,
        _ => panic!("expected a Class model"),
    };
    let clean = parse(&SOURCE.replace("    title Overview\r\n", ""));
    let expected = ClassDiagramModel {
        diagram_type: clean.diagram_type,
        direction: clean.direction,
        acc_title: clean.acc_title,
        acc_descr: clean.acc_descr,
        classes: clean.classes,
        relations: clean.relations,
        notes: clean.notes,
        interfaces: clean.interfaces,
        namespaces: clean.namespaces,
        namespace_facade_aliases: clean.namespace_facade_aliases,
        style_classes: clean.style_classes,
        constants: clean.constants,
        style_precedence_facts: clean.style_precedence_facts,
    };
    let model = parse(SOURCE);
    assert_eq!(model, expected);
    assert!(
        serde_json::to_value(&model)
            .unwrap()
            .get("warningFacts")
            .is_none()
    );
    assert!(
        model
            .style_precedence_facts
            .assignment_before_definition_copy()
    );

    #[cfg(feature = "presentation")]
    {
        use merman_core::{ParseOptions, presentation::MermaidEngine};
        use merman_model::presentation::RenderSemanticModel;

        let rendered = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Class(class) = rendered.model() else {
            panic!("expected a Class render model");
        };
        assert_eq!(class.model(), &expected);
        assert_eq!(
            rendered.class_style_precedence_facts(),
            Some(&expected.style_precedence_facts)
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn ignored_statement_warnings_use_original_spans_and_stay_out_of_the_compat_projection() {
    use merman_core::{ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(SOURCE)
    else {
        panic!("expected a semantic snapshot");
    };
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("expected Class parser facts");
    };
    let [warning] = facts.warning_facts.as_slice() else {
        panic!(
            "expected one ignored statement, got {:?}",
            facts.warning_facts
        );
    };
    let start = SOURCE.find("title Overview").unwrap();
    assert_eq!(warning.rule_id, "merman.authoring.class.ignored_statement");
    assert_eq!(
        warning.span,
        Some(SourceSpan::new(start, start + "title Overview".len()))
    );

    #[cfg(feature = "compat-json")]
    {
        use merman_core::{DiagramParseOutcome, ParseOptions, presentation::MermaidEngine};

        let engine = MermaidEngine::new();
        let snapshot = engine.parse_diagram_snapshot_sync(SOURCE).unwrap().unwrap();
        let DiagramParseOutcome::Parsed {
            model,
            warning_facts,
        } = snapshot.outcome()
        else {
            panic!("expected a successful compatibility snapshot");
        };
        assert_eq!(warning_facts, &facts.warning_facts);
        let clean = SOURCE.replace("    title Overview\r\n", "");
        let expected = engine
            .parse_diagram_sync(&clean, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(model, &expected.model);
        assert!(model.get("warningFacts").is_none());
    }
}
