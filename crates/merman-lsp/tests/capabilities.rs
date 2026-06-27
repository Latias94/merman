use merman_analysis::FenceTextIndexSource;
use merman_lsp::document_store::DocumentStore;
use tower_lsp::lsp_types::Url;

#[test]
fn product_families_are_parser_backed_and_role_aware() {
    struct CapabilityCase<'a> {
        label: &'a str,
        expected_source: FenceTextIndexSource,
        snippet: &'a str,
        required_ids: &'a [&'a str],
        required_outline: &'a [&'a str],
        required_prefixes: &'a [&'a str],
        forbidden_ids: &'a [&'a str],
    }

    let cases = [
        CapabilityCase {
            label: "flowchart",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "flowchart TD\nA-->B\n",
            required_ids: &["A", "B"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "flowchart-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "flowchart TD\nsubgraph group\nA-->B\nC-->",
            required_ids: &["A", "C"],
            required_outline: &["group"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "sequence",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "sequenceDiagram\nAlice->>Bob: Hi\n",
            required_ids: &["Alice", "Bob"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "sequence-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "sequenceDiagram\nAlice->>Bob: Hi\nBob->>",
            required_ids: &["Alice", "Bob"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "state",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "stateDiagram-v2\n",
                "Idle --> Running\n",
                "state \"Paused State\" as Paused\n",
                "classDef activeStyle fill:#0f0,border:#333\n",
                "class Idle, Running activeStyle\n",
                "style Running fill:#f00\n",
            ),
            required_ids: &["Idle", "Running"],
            required_outline: &["activeStyle"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "class",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "classDiagram\n",
                "class User {\n",
                "  +login()\n",
                "}\n",
                "class User:::service\n",
                "classDef service fill:#eee\n",
                "User <|-- Admin\n",
            ),
            required_ids: &["User", "Admin"],
            required_outline: &["+login()", "service"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "er",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
            required_ids: &["CUSTOMER", "ORDER"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "gantt",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "gantt\n",
                "title Roadmap\n",
                "accTitle: Roadmap chart\n",
                "accDescr: Shows release tasks\n",
                "section Demo\n",
                "Task 1: id1,2014-01-01,1d\n",
                "Task 2: id2,after id1,2d\n",
            ),
            required_ids: &["id1", "id2"],
            required_outline: &["Demo"],
            required_prefixes: &["title", "accTitle", "accDescr", "section"],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "mindmap",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "mindmap\nroot(Root Node)\n child1(Child 1)\n child2\n",
            required_ids: &["root", "child1", "child2"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &["Root", "Node", "Child", "1"],
        },
    ];

    for case in cases {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, case.snippet.to_string());
        let index = &snapshot.fences[0].text_index;

        assert_eq!(
            index.source(),
            case.expected_source,
            "unexpected provenance for {}",
            case.label
        );
        for id in case.required_ids {
            assert!(
                index.node_ids().any(|candidate| candidate == id),
                "missing node id {id:?} for {}",
                case.label
            );
        }
        for name in case.required_outline {
            assert!(
                index.outline_items().iter().any(|item| item.name == *name),
                "missing outline item {name:?} for {}",
                case.label
            );
        }
        for prefix in case.required_prefixes {
            assert!(
                index.has_directive_prefix(prefix),
                "missing directive prefix {prefix:?} for {}",
                case.label
            );
        }
        for forbidden in case.forbidden_ids {
            assert!(
                !index.node_ids().any(|candidate| candidate == forbidden),
                "forbidden node id {forbidden:?} leaked for {}",
                case.label
            );
        }
    }
}
