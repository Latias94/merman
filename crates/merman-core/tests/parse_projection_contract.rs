use merman_core as parse_api;
use merman_core::{Error, ParseOptions, diagram_family_capabilities};
use std::path::PathBuf;

#[path = "support/parse_equivalence.rs"]
mod parse_equivalence;
use parse_equivalence::{
    assert_known_type_parse_paths, assert_pair, assert_parse_paths, deterministic_engine,
};

#[cfg(feature = "diagram-flowchart")]
#[test]
#[should_panic(expected = "different parser acceptance")]
fn shared_assertion_rejects_a_single_path_acceptance_change() {
    let engine = deterministic_engine();
    let source = "flowchart TD\nA --> B\n";
    let semantic = engine.parse_diagram_sync(source, ParseOptions::strict());
    let typed =
        engine.parse_diagram_for_render_model_sync("flowchart TD\nA -->", ParseOptions::strict());
    assert_pair(&semantic, &typed, source);
}

#[cfg(feature = "diagram-flowchart")]
#[test]
#[should_panic(expected = "complete semantic projection")]
fn shared_assertion_rejects_different_real_edge_endpoints_with_the_same_metadata() {
    let engine = deterministic_engine();
    let source = "flowchart TD\nA --> B\n";
    let semantic = engine.parse_diagram_sync(source, ParseOptions::strict());
    let typed = engine
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> C\n", ParseOptions::strict());
    assert_pair(&semantic, &typed, source);
}

#[cfg(feature = "diagram-git-graph")]
#[test]
fn incomplete_directive_recovery_preserves_each_parse_path_contract() {
    let engine = deterministic_engine();
    for (source, strict_source, recovered_source) in [
        (
            "gitGraph\n%%{\nmerge x\n",
            "gitGraph\n",
            "gitGraph\nmerge x\n",
        ),
        (
            "gitGraph LR:\n  commit\n  branch develoInfinity%%{\n  checkout main\n  merge develop\n",
            "gitGraph LR:\n  commit\n  branch develoInfinity",
            "gitGraph LR:\n  commit\n  branch develoInfinity\n  checkout main\n  merge develop\n",
        ),
    ] {
        let strict = engine.parse_diagram_sync(source, ParseOptions::strict());
        let lenient = engine.parse_diagram_sync(source, ParseOptions::lenient());
        assert_eq!(
            strict.as_ref().unwrap().as_ref().unwrap().meta.diagram_type,
            "gitGraph"
        );
        assert_eq!(
            lenient
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .meta
                .diagram_type,
            "error"
        );
        assert_pair(
            &strict,
            &engine.parse_diagram_for_render_model_sync(strict_source, ParseOptions::strict()),
            source,
        );
        assert_pair(
            &lenient,
            &engine.parse_diagram_for_render_model_sync(recovered_source, ParseOptions::lenient()),
            source,
        );
        assert_parse_paths(&engine, source);
        assert_known_type_parse_paths(&engine, "gitGraph", source);
        {
            let snapshot = engine.parse_diagram_snapshot_sync(source).unwrap().unwrap();
            assert!(snapshot.recovered_incomplete_directive());
            assert!(matches!(
                snapshot.outcome(),
                merman_core::DiagramParseOutcome::Failed(Error::DiagramParse { diagram_type, .. })
                    if diagram_type == "gitGraph"
            ));
        }
    }
}

#[cfg(feature = "diagram-git-graph")]
#[test]
fn recovery_can_change_successful_models_without_changing_the_diagram_type() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\ncommit\n";
    let strict = engine
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let lenient = engine
        .parse_diagram_sync(source, ParseOptions::lenient())
        .unwrap()
        .unwrap();
    assert_eq!(strict.meta.diagram_type, "gitGraph");
    assert_eq!(lenient.meta.diagram_type, "gitGraph");
    assert_ne!(strict.model, lenient.model);
    assert_pair(
        &Ok(Some(strict)),
        &engine.parse_diagram_for_render_model_sync("gitGraph\n", ParseOptions::strict()),
        source,
    );
    assert_pair(
        &Ok(Some(lenient)),
        &engine.parse_diagram_for_render_model_sync("gitGraph\ncommit\n", ParseOptions::lenient()),
        source,
    );
    assert_parse_paths(&engine, source);
    assert_known_type_parse_paths(&engine, "gitGraph", source);

    // Complete directives and ordinary input retain the same policy result in both modes.
    for source in ["gitGraph\ncommit\n", "gitGraph\n%%{init: {}}%%\ncommit\n"] {
        assert_pair(
            &engine.parse_diagram_sync(source, ParseOptions::strict()),
            &engine.parse_diagram_for_render_model_sync(source, ParseOptions::lenient()),
            source,
        );
        assert_parse_paths(&engine, source);
        assert!(
            !engine
                .parse_diagram_snapshot_sync(source)
                .unwrap()
                .unwrap()
                .recovered_incomplete_directive()
        );
    }
}

#[cfg(feature = "diagram-git-graph")]
#[test]
#[should_panic(expected = "different parser acceptance")]
fn recovery_does_not_hide_a_strict_typed_path_acceptance_change() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\nmerge x\n";
    let semantic = engine.parse_diagram_with_type_sync("gitGraph", source, ParseOptions::strict());
    assert!(matches!(&semantic, Ok(Some(parsed)) if parsed.meta.diagram_type == "gitGraph"));
    let typed = engine.parse_diagram_for_render_model_with_type_sync(
        "gitGraph",
        "gitGraph\nmerge x\n",
        ParseOptions::strict(),
    );
    assert_pair(&semantic, &typed, source);
}

#[cfg(feature = "diagram-git-graph")]
#[test]
#[should_panic(expected = "complete semantic projection")]
fn recovery_does_not_hide_a_lenient_known_type_model_change() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\ncommit id: \"expected\"\n";
    let semantic = engine.parse_diagram_with_type_sync("gitGraph", source, ParseOptions::lenient());
    assert!(matches!(&semantic, Ok(Some(parsed)) if parsed.meta.diagram_type == "gitGraph"));
    let typed = engine.parse_diagram_for_render_model_with_type_sync(
        "gitGraph",
        "gitGraph\ncommit id: \"changed\"\n",
        ParseOptions::lenient(),
    );
    assert_pair(&semantic, &typed, source);
}

#[cfg(feature = "diagram-git-graph")]
#[test]
#[should_panic(expected = "snapshot recovery projection")]
fn recovery_receipt_does_not_hide_a_snapshot_model_change() {
    let engine = deterministic_engine();
    let source = "gitGraph\n%%{\ncommit id: \"wanted\"\n";
    let strict = engine.parse_diagram_sync(source, ParseOptions::strict());
    let lenient = engine.parse_diagram_sync(source, ParseOptions::lenient());
    let snapshot = engine
        .parse_diagram_snapshot_sync("gitGraph\n%%{\ncommit id: \"actual\"\n")
        .unwrap()
        .unwrap();
    assert!(snapshot.recovered_incomplete_directive());
    parse_equivalence::assert_snapshot(&strict, &lenient, Ok(Some(snapshot)), source);
}

#[test]
fn checked_in_parse_fuzz_seeds_use_the_same_complete_assertions() {
    let engine = deterministic_engine();
    let seeds = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds/mermaid");
    let mut entries = std::fs::read_dir(seeds)
        .expect("parse seed corpus")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    entries.sort();
    assert!(!entries.is_empty());
    for path in entries {
        let bytes = std::fs::read(&path).expect("read parse seed");
        if let Ok(source) = std::str::from_utf8(&bytes) {
            assert_parse_paths(&engine, source);
        }
    }
}

#[test]
fn known_type_invalid_sources_share_model_and_snapshot_acceptance() {
    let engine = deterministic_engine();
    for family in diagram_family_capabilities() {
        for source in ["", "not-a-diagram", "piep", "packet", "\"雪\": -1\n"] {
            assert_known_type_parse_paths(&engine, family.diagram_type, source);
        }
    }
}
