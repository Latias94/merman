use merman_ascii::{AsciiError, AsciiRenderOptions, render_model};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};
use merman_core::{Engine, ParseOptions};

fn render_parsed(source: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("diagram should parse")
        .expect("diagram should be detected");
    render_model(parsed.model(), &AsciiRenderOptions::ascii())
        .expect("parsed structured-text diagram should render")
}

fn timeline_task(section: &str, section_index: Option<usize>, task: &str) -> TimelineRenderTask {
    TimelineRenderTask {
        id: 0,
        section: section.to_string(),
        section_index,
        task_type: section.to_string(),
        task: task.to_string(),
        score: 0,
        events: Vec::new(),
    }
}

fn journey_task(section: &str, section_index: Option<usize>, task: &str) -> JourneyRenderTask {
    JourneyRenderTask {
        score: 3,
        score_is_nan: false,
        people: Vec::new(),
        section: section.to_string(),
        section_index,
        task_type: section.to_string(),
        task: task.to_string(),
    }
}

#[test]
fn timeline_duplicate_section_fixture_preserves_occurrence_ownership() {
    let rendered = render_parsed(include_str!(
        "../../../fixtures/timeline/timeline_stress_section_name_repeated.mmd"
    ));

    assert_eq!(
        rendered,
        concat!(
            "title(bytes=22)=\"Repeated section names\"\n",
            "direction: LR\n",
            "section(bytes=8)=\"Repeated\"\n",
            "  - task(bytes=5)=\"2000 \"\n",
            "    * event(bytes=25)=\"First in repeated section\"\n",
            "section(bytes=8)=\"Repeated\"\n",
            "  - task(bytes=5)=\"2001 \"\n",
            "    * event(bytes=29)=\"Second section with same name\"\n",
            "  - task(bytes=5)=\"2002 \"\n",
            "    * event(bytes=10)=\"Third task\"",
        )
    );
}

#[test]
fn journey_duplicate_sections_preserve_parser_occurrence_ownership() {
    let rendered = render_parsed(concat!(
        "journey\n",
        "title Repeated journey\n",
        "section Repeated\n",
        "  First: 5: Alice\n",
        "section Repeated\n",
        "  Second: 3: Bob\n",
    ));

    assert_eq!(
        rendered,
        concat!(
            "title(bytes=16)=\"Repeated journey\"\n",
            "actors=[bytes=5 \"Alice\", bytes=3 \"Bob\"]\n",
            "section(bytes=8)=\"Repeated\"\n",
            "  - task(bytes=7)=\"  First\" score=5 people=[bytes=5 \"Alice\"]\n",
            "section(bytes=8)=\"Repeated\"\n",
            "  - task(bytes=8)=\"  Second\" score=3 people=[bytes=3 \"Bob\"]",
        )
    );
}

#[test]
fn direct_models_disclose_unknown_and_unsectioned_tasks_once() {
    let mut timeline = TimelineDiagramRenderModel::default();
    timeline.sections = vec!["Known".to_string()];
    timeline.tasks = vec![
        timeline_task("Known", Some(0), "Declared"),
        timeline_task("Missing", None, "Unknown"),
        timeline_task("", None, "Loose"),
    ];
    let timeline_rendered = render_model(
        &RenderSemanticModel::Timeline(timeline),
        &AsciiRenderOptions::ascii(),
    )
    .expect("Timeline orphan tasks should remain visible");
    assert_eq!(
        timeline_rendered,
        concat!(
            "direction: LR\n",
            "section(bytes=5)=\"Known\"\n",
            "  - task(bytes=8)=\"Declared\"\n",
            "section(bytes=7)=\"Missing\" status=undeclared\n",
            "  - task(bytes=7)=\"Unknown\"\n",
            "section(bytes=0)=\"\" status=unsectioned\n",
            "  - task(bytes=5)=\"Loose\"",
        )
    );

    let mut journey = JourneyDiagramRenderModel::default();
    journey.sections = vec!["Known".to_string()];
    journey.tasks = vec![
        journey_task("Known", Some(0), "Declared"),
        journey_task("Missing", None, "Unknown"),
        journey_task("", None, "Loose"),
    ];
    let journey_rendered = render_model(
        &RenderSemanticModel::Journey(journey),
        &AsciiRenderOptions::ascii(),
    )
    .expect("Journey orphan tasks should remain visible");
    assert_eq!(
        journey_rendered,
        concat!(
            "section(bytes=5)=\"Known\"\n",
            "  - task(bytes=8)=\"Declared\" score=3\n",
            "section(bytes=7)=\"Missing\" status=undeclared\n",
            "  - task(bytes=7)=\"Unknown\" score=3\n",
            "section(bytes=0)=\"\" status=unsectioned\n",
            "  - task(bytes=5)=\"Loose\" score=3",
        )
    );
}

#[test]
fn duplicate_direct_section_labels_require_an_occurrence_index() {
    let mut timeline = TimelineDiagramRenderModel::default();
    timeline.sections = vec!["Repeated".to_string(), "Repeated".to_string()];
    timeline.tasks = vec![timeline_task("Repeated", None, "Ambiguous")];
    let error = render_model(
        &RenderSemanticModel::Timeline(timeline),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate Timeline labels cannot infer occurrence ownership");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "timeline",
            feature: "ambiguous section label without occurrence index",
        }
    );

    let mut journey = JourneyDiagramRenderModel::default();
    journey.sections = vec!["Repeated".to_string(), "Repeated".to_string()];
    journey.tasks = vec![journey_task("Repeated", None, "Ambiguous")];
    let error = render_model(
        &RenderSemanticModel::Journey(journey),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate Journey labels cannot infer occurrence ownership");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "journey",
            feature: "ambiguous section label without occurrence index",
        }
    );
}

#[test]
fn explicit_section_occurrences_must_be_in_bounds_and_match_the_label() {
    let mut timeline = TimelineDiagramRenderModel::default();
    timeline.sections = vec!["Known".to_string()];
    timeline.tasks = vec![timeline_task("Known", Some(1), "Out of bounds")];
    let error = render_model(
        &RenderSemanticModel::Timeline(timeline),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("out-of-bounds Timeline occurrence ownership must be rejected");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "timeline",
            feature: "section occurrence index outside declared sections",
        }
    );

    let mut journey = JourneyDiagramRenderModel::default();
    journey.sections = vec!["Known".to_string()];
    journey.tasks = vec![journey_task("Other", Some(0), "Mismatch")];
    let error = render_model(
        &RenderSemanticModel::Journey(journey),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("Journey occurrence ownership must match its section label");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "journey",
            feature: "section occurrence label mismatch",
        }
    );
}
