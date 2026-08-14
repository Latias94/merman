use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilySnapshot {
    schema_version: u32,
    public_id: String,
    root_node: String,
    nodes: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct NodeType {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
    #[serde(default)]
    fields: BTreeMap<String, Value>,
}

#[test]
fn family_node_and_field_snapshots_match_generated_schema() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated: Vec<NodeType> = serde_json::from_slice(
        &fs::read(package.join("src/node-types.json")).expect("read generated node types"),
    )
    .expect("parse generated node types");
    let actual = generated
        .into_iter()
        .filter(|node| node.named)
        .map(|node| (node.kind, node.fields.into_keys().collect::<Vec<String>>()))
        .collect::<BTreeMap<_, _>>();

    let directory = package.join("test/schema/families");
    let mut paths = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.expect("read schema snapshot entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "family schema snapshots must not be empty"
    );

    let mut public_ids = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for path in paths {
        let snapshot: FamilySnapshot = serde_json::from_slice(
            &fs::read(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
        )
        .unwrap_or_else(|error| panic!("invalid snapshot {}: {error}", path.display()));
        assert_eq!(snapshot.schema_version, 1, "{}", path.display());
        assert_eq!(
            path.file_stem().and_then(|stem| stem.to_str()),
            Some(snapshot.public_id.as_str()),
            "{}",
            path.display()
        );
        assert!(public_ids.insert(snapshot.public_id.clone()));
        assert!(roots.insert(snapshot.root_node.clone()));
        assert!(snapshot.nodes.contains_key(&snapshot.root_node));
        assert!(!snapshot.nodes.is_empty());

        for (kind, expected_fields) in snapshot.nodes {
            assert!(
                expected_fields.windows(2).all(|pair| pair[0] < pair[1]),
                "{kind}: fields must be sorted and unique"
            );
            assert_eq!(
                actual.get(&kind),
                Some(&expected_fields),
                "{}: node/field contract drifted",
                path.display()
            );
        }
    }
}
