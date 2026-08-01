use merman_fixture_render_context::{FixtureRenderContext, Provenance, RenderContextCatalog};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const STALE_STADIUM_CONTEXT: &str =
    "flowchart/upstream_cypress_flowchart_elk_spec_v2_elk_16_render_stadium_shape_008.mmd";

const PREVIOUSLY_MISSING_CONTEXTS: &[&str] = &[
    "flowchart/local_flowchart_elk_hardening_cluster_boundary_styles_004.mmd",
    "flowchart/local_flowchart_elk_hardening_compound_parent_child_edges_001.mmd",
    "flowchart/upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_2_017.mmd",
    "flowchart/upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_4_016.mmd",
    "flowchart/upstream_cypress_flowchart_handdrawn_spec_fdh37_should_render_non_escaped_with_html_labels_037.mmd",
];

const STRICT_SECURITY_FIXTURES: &[&str] = &[
    "class/stress_class_click_strict_sanitization_015.mmd",
    "flowchart/stress_flowchart_click_sanitization_strict_027.mmd",
    "flowchart/stress_flowchart_icons_click_security_strict_057.mmd",
    "sequence/stress_sequence_batch5_strict_links_properties_044.mmd",
    "state/stress_state_securitylevel_strict_clicks_016.mmd",
    "state/stress_state_securitylevel_strict_clicks_with_data_urls_047.mmd",
];

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
}

fn fixture_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).expect("read fixture directory") {
            let path = entry.expect("read fixture entry").path();
            if path.is_dir() {
                if !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('_'))
                {
                    directories.push(path);
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("mmd") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

#[test]
fn committed_catalog_exactly_matches_declared_fixture_host_inputs() {
    let root = fixtures_root();

    // Corpus contract: nested frontmatter `config.securityLevel` is an imported host render
    // option. An init/initialize securityLevel in a committed fixture is likewise retained as
    // explicit host context. Root-level diagram config is not promoted, so secure-filter tests
    // remain strict unless a test explicitly supplies host config outside the fixture corpus.
    let mut derived = BTreeMap::new();
    for path in fixture_paths(&root) {
        let relative = path
            .strip_prefix(&root)
            .expect("fixture below root")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read(&path).expect("read fixture source");
        if let Some(context) = FixtureRenderContext::derive(&relative, &source)
            .unwrap_or_else(|error| panic!("derive {relative}: {error}"))
        {
            derived.insert(
                relative,
                (context.security_level(), context.provenance().clone()),
            );
        }
    }

    let catalog = RenderContextCatalog::load(&root).expect("load committed render contexts");

    let recorded = catalog
        .contexts()
        .map(|context| {
            (
                context.fixture().to_string(),
                (context.security_level(), context.provenance().clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(recorded, derived);

    assert!(!recorded.contains_key(STALE_STADIUM_CONTEXT));
    for fixture in PREVIOUSLY_MISSING_CONTEXTS {
        assert!(
            recorded.contains_key(*fixture),
            "declared host input is missing from the committed catalog: {fixture}"
        );
    }

    let manifest_path = root.join(merman_fixture_render_context::MANIFEST_RELATIVE_PATH);
    assert_eq!(
        fs::read_to_string(manifest_path).expect("read committed manifest"),
        catalog.to_json().expect("render canonical manifest"),
        "the committed catalog must stay canonical and sorted"
    );
}

#[test]
fn committed_contexts_are_only_security_host_inputs() {
    let catalog = RenderContextCatalog::load(fixtures_root()).expect("load committed contexts");
    for context in catalog.contexts() {
        let site_config = context.site_config_value();
        let object = site_config.as_object().expect("site config object");
        assert_eq!(object.len(), 1);
        assert!(matches!(
            object
                .get("securityLevel")
                .and_then(serde_json::Value::as_str),
            Some("loose" | "sandbox")
        ));
        assert!(matches!(
            context.provenance(),
            Provenance::Frontmatter { .. } | Provenance::Directive { .. }
        ));
    }
}

#[test]
fn strict_sanitization_fixtures_are_not_promoted_to_host_contexts() {
    let root = fixtures_root();
    for relative in STRICT_SECURITY_FIXTURES {
        let source = fs::read(root.join(relative)).expect("read strict security fixture");
        assert!(
            FixtureRenderContext::derive(relative, &source)
                .unwrap_or_else(|error| panic!("derive {relative}: {error}"))
                .is_none(),
            "strict secure-filter fixture must retain the default host policy: {relative}"
        );
    }
}
