use merman_core::{EditorRenamePolicy, Engine, ParseOptions, RenderSemanticModel};

#[test]
fn usecase_public_parse_routes_preserve_preprocessed_source_identity() {
    let source = "---\ntitle: 订单\n---\nusecase-beta\nactor Customer(\"客户\")\nsystemBoundary \"Order system\"\n  Checkout(\"Place order\")\nend\nCustomer --> Checkout\n";
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_sync(source, ParseOptions::strict())
        .expect("semantic parse")
        .expect("Usecase detection");
    let typed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("typed parse")
        .expect("Usecase detection");
    let RenderSemanticModel::Usecase(model) = typed.model() else {
        panic!("Usecase must have its own typed semantic model");
    };
    assert_eq!(typed.metadata().title.as_deref(), Some("订单"));
    assert_eq!(model.nodes[0].label, "客户");
    assert_eq!(model.boundaries[0].members, ["Checkout"]);
    assert_eq!(model.relationships[0].source, "Customer");
    assert_eq!(model.relationships[0].target, "Checkout");
    assert_eq!(
        typed
            .model()
            .compatibility_json(typed.metadata())
            .expect("JSON projection"),
        parsed.model
    );

    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("usecase", source)
        .expect("editor parse")
        .expect("parser-backed facts");
    assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);
    for id in ["Customer", "Checkout"] {
        let occurrences = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == id)
            .collect::<Vec<_>>();
        assert_eq!(
            occurrences.len(),
            2,
            "declaration and relationship endpoint for {id}"
        );
        for occurrence in occurrences {
            assert_eq!(
                &source[occurrence.selection.start..occurrence.selection.end],
                id
            );
            assert_eq!(
                occurrence.rename_policy,
                EditorRenamePolicy::UsecaseIdentifier
            );
        }
    }
}

#[test]
fn usecase_rename_accepts_only_a_complete_identifier_token() {
    let policy = EditorRenamePolicy::UsecaseIdentifier;
    for accepted in [
        "Customer",
        "_user",
        "123",
        "123abc",
        "actor2",
        "systemBoundary2",
    ] {
        assert!(policy.accepts(accepted), "{accepted}");
    }
    for rejected in [
        "",
        "two words",
        "customer-id",
        "客户",
        "actor",
        "direction",
        "end",
        "systemBoundary",
    ] {
        assert!(!policy.accepts(rejected), "{rejected}");
    }
}
