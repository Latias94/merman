use merman_analysis::{FenceSemanticRole, FenceTextIndexSource};
use merman_lsp::document_store::DocumentStore;
use tower_lsp::lsp_types::Url;

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n".to_string(),
    );

    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(snapshot.fences[0].body_start, 0);
    assert_eq!(
        snapshot.fences[0].text,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n"
    );
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(
        snapshot.fences[0]
            .text_index
            .has_directive_prefix("classDef")
    );
}

#[test]
fn markdown_documents_create_fences_for_markdown_extensions() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "before\n```mermaid\n%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD\nA-->B\n```\nafter\n"
            .to_string(),
    );

    assert_eq!(snapshot.fences.len(), 1);
    assert!(snapshot.fences[0].text.contains("flowchart TD"));
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "A"));
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(snapshot.fences[0].text_index.has_directive_prefix("init"));
}

#[test]
fn markdown_documents_create_multiple_mermaid_fences() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A-->B\n",
            "```\n",
            "middle\n",
            "```mermaid\n",
            "sequenceDiagram\n",
            "Alice->>Bob: Hi\n",
            "```\n",
            "after\n",
        )
        .to_string(),
    );

    assert_eq!(snapshot.fences.len(), 2);
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert_eq!(snapshot.fences[1].diagram_type.as_deref(), Some("sequence"));
    assert_eq!(
        snapshot.fences[0].text_index.source(),
        FenceTextIndexSource::ParserComplete
    );
    assert_eq!(
        snapshot.fences[1].text_index.source(),
        FenceTextIndexSource::ParserComplete
    );
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "A"));
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "B"));
    assert!(
        snapshot.fences[1]
            .text_index
            .node_ids()
            .any(|id| id == "Alice")
    );
    assert!(
        snapshot.fences[1]
            .text_index
            .node_ids()
            .any(|id| id == "Bob")
    );
}

#[test]
fn newer_versions_replace_the_stored_snapshot() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    let first = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\n".to_string());
    let second = store.upsert(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
    );

    assert_eq!(first.version, 1);
    assert_eq!(second.version, 2);

    let stored = store.get(&uri).unwrap();
    assert_eq!(stored.version, 2);
    assert!(stored.text.contains("sequenceDiagram"));
    assert!(!stored.text.contains("flowchart TD"));
    assert_eq!(stored.fences.len(), 1);
    assert_eq!(stored.fences[0].diagram_type.as_deref(), Some("sequence"));
}

#[test]
fn incomplete_flowchart_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nsubgraph group\nA-->B\nC-->".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "A"));
    assert!(index.node_ids().any(|id| id == "C"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "group")
    );
}

#[test]
fn sequence_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nparticipant Alice\nactor Bob\nAlice->>Bob: Hi\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn sequence_payload_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "sequenceDiagram\n",
            "title: Diagram Title\n",
            "accTitle: Accessible Title\n",
            "accDescr: Accessible Description\n",
            "participant Alice\n",
            "actor Bob\n",
            "Alice->>Bob: Hello\n",
            "Note over Alice,Bob: Review\n",
            "details Alice: {\"owner\": \"platform\"}\n",
            "links Alice: { \"Repo\": \"https://example.com/\" }\n",
            "link Alice: Endpoint @ https://alice.example.com\n",
            "properties Alice: {\"class\": \"internal-service-actor\"}\n",
        )
        .to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Alice" && item.role == FenceSemanticRole::Entity)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Bob" && item.role == FenceSemanticRole::Entity)
    );
    for payload in [
        "Diagram Title",
        "Accessible Title",
        "Accessible Description",
        "Hello",
        "Review",
        r#"{"owner": "platform"}"#,
        r#"{ "Repo": "https://example.com/" }"#,
        "Endpoint @ https://alice.example.com",
        r#"{"class": "internal-service-actor"}"#,
    ] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "sequence payload {payload:?} was not retained as a semantic item"
        );
    }

    for leaked in [
        "Diagram Title",
        "Accessible Title",
        "Accessible Description",
        "Hello",
        "Review",
        r#"{"owner": "platform"}"#,
        "Repo",
        "Endpoint",
        "https://example.com/",
        "https://alice.example.com",
        "internal-service-actor",
    ] {
        assert!(
            !index.node_ids().any(|id| id == leaked),
            "sequence payload leaked {leaked:?} into completion ids"
        );
        assert!(
            !index.outline_items().iter().any(|item| item.name == leaked),
            "sequence payload leaked {leaked:?} into outline items"
        );
    }

    for prefix in [
        "title",
        "accTitle",
        "accDescr",
        "details",
        "links",
        "link",
        "properties",
    ] {
        assert!(index.has_directive_prefix(prefix));
    }
}

#[test]
fn incomplete_sequence_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\nBob->>".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn state_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "stateDiagram-v2\n",
            "[*] --> Idle\n",
            "Idle --> Running\n",
            "Idle: Waiting state\n",
            "Idle --> Running: starts\n",
            "state \"Paused State\" as Paused\n",
            "note right of Running : Running details\n",
            "note \"Floating note\" as note1\n",
            "classDef activeStyle fill:#0f0,border:#333\n",
            "class Idle, Running activeStyle\n",
            "style Running fill:#f00\n",
            "accTitle: Lifecycle chart\n",
            "accDescr: Shows state transitions\n",
            "click Running \"https://example.com/run\" \"Run details\"\n",
        )
        .to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
    assert!(!index.node_ids().any(|id| id == "activeStyle"));
    assert!(!index.node_ids().any(|id| id == "Waiting state"));
    assert!(!index.node_ids().any(|id| id == "starts"));
    assert!(!index.node_ids().any(|id| id == "Paused State"));
    assert!(!index.node_ids().any(|id| id == "Running details"));
    assert!(!index.node_ids().any(|id| id == "Floating note"));
    assert!(!index.node_ids().any(|id| id == "note1"));
    assert!(!index.node_ids().any(|id| id == "fill:#0f0,border:#333"));
    assert!(!index.node_ids().any(|id| id == "fill:#f00"));
    assert!(!index.node_ids().any(|id| id == "Lifecycle chart"));
    assert!(!index.node_ids().any(|id| id == "Shows state transitions"));
    assert!(!index.node_ids().any(|id| id == "https://example.com/run"));
    assert!(!index.node_ids().any(|id| id == "Run details"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "activeStyle"
                && item.detail.as_deref() == Some("state class definition"))
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Waiting state")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "starts")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Paused State")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Running details")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Lifecycle chart")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com/run")
    );
}

#[test]
fn incomplete_state_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "stateDiagram-v2\nIdle --> Running\nRunning -->".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
}

#[test]
fn class_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "classDiagram\nclass User\nUser <|-- Admin\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(index.node_ids().any(|id| id == "Admin"));
}

#[test]
fn incomplete_class_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "classDiagram\nclass User\nUser <|--".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "User"));
}

#[test]
fn class_member_outline_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "classDiagram\n",
            "class User {\n",
            "  +login()\n",
            "  -password: String\n",
            "}\n",
            "class Visible[\"Visible label\"]\n",
            "<<interface>> User\n",
            "User: email\n",
            "Class1 \"1\" *-- \"many\" Class02 : contains\n",
            "User <|-- Admin : manages\n",
            "note for User \"Primary user\"\n",
            "note \"Floating note\"\n",
            "click User href \"https://example.com\" \"Open user\" _blank\n",
            "click User call open(userId) \"Open user\"\n",
            "accTitle: Class chart\n",
            "accDescr: Shows class relationships\n",
            "classDef service fill:#eee\n",
            "class User:::service\n",
            "cssClass \"User,Admin\" service\n",
            "style User fill:#fff\n",
        )
        .to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.has_directive_prefix("classDef"));
    assert!(index.has_directive_prefix("class"));
    assert!(index.has_directive_prefix("cssClass"));
    assert!(index.has_directive_prefix("style"));
    assert!(index.has_directive_prefix("click"));
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(!index.node_ids().any(|id| id == "+login()"));
    assert!(!index.node_ids().any(|id| id == "-password: String"));
    assert!(!index.node_ids().any(|id| id == "email"));
    assert!(!index.node_ids().any(|id| id == "interface"));
    assert!(!index.node_ids().any(|id| id == "https://example.com"));
    assert!(!index.node_ids().any(|id| id == "Open user"));
    assert!(!index.node_ids().any(|id| id == "_blank"));
    assert!(!index.node_ids().any(|id| id == "service"));
    assert!(!index.node_ids().any(|id| id == "fill:#eee"));
    assert!(!index.node_ids().any(|id| id == "fill:#fff"));
    assert!(!index.node_ids().any(|id| id == "open"));
    assert!(!index.node_ids().any(|id| id == "userId"));
    assert!(!index.node_ids().any(|id| id == "Visible label"));
    assert!(!index.node_ids().any(|id| id == "1"));
    assert!(!index.node_ids().any(|id| id == "many"));
    assert!(!index.node_ids().any(|id| id == "manages"));
    assert!(!index.node_ids().any(|id| id == "Primary user"));
    assert!(!index.node_ids().any(|id| id == "Floating note"));
    assert!(!index.node_ids().any(|id| id == "Class chart"));
    assert!(!index.node_ids().any(|id| id == "Shows class relationships"));

    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "+login()" && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "-password: String"
                && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "email" && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index.outline_items().iter().any(
            |item| item.name == "service" && item.detail.as_deref() == Some("class definition")
        )
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "interface")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Open user")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "_blank")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "fill:#eee")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "open"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "userId")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "manages")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Visible label")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "1"));
    assert!(!index.outline_items().iter().any(|item| item.name == "many"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Primary user")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Class chart")
    );
}

#[test]
fn er_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn incomplete_er_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "erDiagram\nCUSTOMER ||--o{ ORDER :".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn er_attribute_payload_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "erDiagram\n",
            "BOOK {\n",
            "  string title PK, FK \"primary title\"\n",
            "}\n",
        )
        .to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "BOOK"));
    assert!(!index.node_ids().any(|id| id == "title"));
    assert!(!index.node_ids().any(|id| id == "string"));
    assert!(!index.node_ids().any(|id| id == "PK"));
    assert!(!index.node_ids().any(|id| id == "FK"));
    assert!(!index.node_ids().any(|id| id == "primary title"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "title" && item.detail.as_deref() == Some("er attribute"))
    );
}

#[test]
fn gantt_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "gantt\n",
            "title Roadmap\n",
            "accTitle: Roadmap chart\n",
            "accDescr: Shows release tasks\n",
            "accDescr {\n",
            "  Shows release tasks\n",
            "  across releases\n",
            "}\n",
            "dateFormat YYYY-MM-DD\n",
            "section Demo\n",
            "Task 1: id1,2014-01-01,1d\n",
            "Task 2: id2,after id1,2d\n",
            "click id2 call open(userId) href \"https://example.com/\"\n",
        )
        .to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("gantt"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "id1"));
    assert!(index.node_ids().any(|id| id == "id2"));
    assert!(!index.node_ids().any(|id| id == "Demo"));
    assert!(!index.node_ids().any(|id| id == "Roadmap"));
    assert!(!index.node_ids().any(|id| id == "Roadmap chart"));
    assert!(!index.node_ids().any(|id| id == "Shows release tasks"));
    assert!(
        !index
            .node_ids()
            .any(|id| id == "Shows release tasks\n  across releases")
    );
    assert!(!index.node_ids().any(|id| id == "YYYY-MM-DD"));
    assert!(!index.node_ids().any(|id| id == "open"));
    assert!(!index.node_ids().any(|id| id == "userId"));
    assert!(!index.node_ids().any(|id| id == "https://example.com/"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "Demo" && item.detail.as_deref() == Some("gantt section"))
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Roadmap")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Roadmap chart")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Shows release tasks")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Shows release tasks\n  across releases")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "YYYY-MM-DD")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "open"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "userId")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com/")
    );
    assert!(index.has_directive_prefix("title"));
    assert!(index.has_directive_prefix("accTitle"));
    assert!(index.has_directive_prefix("accDescr"));
    assert!(index.has_directive_prefix("dateFormat"));
    assert!(index.has_directive_prefix("section"));
    assert!(index.has_directive_prefix("click"));
}

#[test]
fn incomplete_gantt_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "gantt\ndateFormat YYYY-MM-DD\nTask 1: id1,2014-01-01,1d\nTask 2".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "id1"));
    assert!(!index.node_ids().any(|id| id == "Task"));
}

#[test]
fn mindmap_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "mindmap\n",
            "root(Root Node)\n",
            " child1(Child 1)\n",
            " :::hot\n",
            " ::icon(bomb)\n",
            " child2\n",
        )
        .to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "root"));
    assert!(index.node_ids().any(|id| id == "child1"));
    assert!(index.node_ids().any(|id| id == "child2"));
    assert!(!index.node_ids().any(|id| id == "hot"));
    assert!(!index.node_ids().any(|id| id == "bomb"));
    assert!(!index.outline_items().iter().any(|item| item.name == "hot"));
    assert!(!index.outline_items().iter().any(|item| item.name == "bomb"));
}

#[test]
fn incomplete_mindmap_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "mindmap\nroot\n child[unterminated".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "root"));
    assert!(!index.node_ids().any(|id| id == "child"));
}
