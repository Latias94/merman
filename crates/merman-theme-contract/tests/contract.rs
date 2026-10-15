use merman_theme_contract::{authoring_version_registry, resolve_authoring_version};

#[test]
fn registry_exposes_the_only_supported_authoring_tuple() {
    let versions = authoring_version_registry();

    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].authoring_schema_version(), 1);
    assert_eq!(versions[0].expansion_version(), 1);
    assert_eq!(versions[0].spec_schema_version(), 1);
}

#[test]
fn authoring_version_resolution_is_tuple_scoped_and_fail_closed() {
    let versions = authoring_version_registry();
    let resolved = resolve_authoring_version(1, 1).expect("version 1 tuple should resolve");
    assert_eq!(resolved.spec_schema_version(), 1);

    let unknown_expansion = resolve_authoring_version(1, 2).unwrap_err();
    assert_eq!(unknown_expansion.authoring_schema_version(), 1);
    assert_eq!(unknown_expansion.expansion_version(), 2);

    let unknown_authoring = resolve_authoring_version(2, 1).unwrap_err();
    assert_eq!(unknown_authoring.authoring_schema_version(), 2);
    assert_eq!(unknown_authoring.expansion_version(), 1);

    for tuple in [(0, 0), (0, 1), (1, 0)] {
        assert!(resolve_authoring_version(tuple.0, tuple.1).is_err());
    }

    let unique_pairs = versions
        .iter()
        .map(|version| {
            (
                version.authoring_schema_version(),
                version.expansion_version(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique_pairs.len(), versions.len());
}
