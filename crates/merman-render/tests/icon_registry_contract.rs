use merman_render::svg::{
    IconPack, IconRegistry, IconRegistryBuildErrorKind, IconRegistryBuilder,
    IconRegistryResourceLimitId, icon_registry_resource_limit_descriptors,
};
use std::collections::BTreeSet;

fn pack(prefix: &str, name: &str, body: &str) -> Vec<u8> {
    format!(r#"{{"prefix":"{prefix}","icons":{{"{name}":{{"body":{body:?}}}}}}}"#).into_bytes()
}

#[test]
fn registry_owns_validated_state_after_borrowed_pack_buffers_are_dropped() {
    let first = pack("alpha", "rocket", "<path d=\"M0 0H16V16H0z\"/>");
    let second = pack("beta", "ship", "<circle cx=\"8\" cy=\"8\" r=\"8\"/>");

    let builder = IconRegistryBuilder::new()
        .add_pack(IconPack::new(&first))
        .expect("first pack is admitted")
        .add_pack(IconPack::new(&second).with_registration_name("fleet"))
        .expect("second pack is admitted");
    drop(first);
    drop(second);

    let registry = builder.build().expect("the transaction publishes once");
    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());
    assert_eq!(registry.clone().len(), registry.len());
}

#[test]
fn convenience_factory_is_transactional_across_multiple_packs() {
    let good = pack("alpha", "rocket", "<path/>");
    let bad = br#"{"prefix":"beta","icons":{"ship":{"body":"<path>"}}}"#;

    let error = IconRegistry::from_packs([
        IconPack::new(&good),
        IconPack::new(bad).with_registration_name("beta"),
    ])
    .expect_err("one invalid pack rejects the complete registry");

    assert_eq!(error.kind(), IconRegistryBuildErrorKind::InvalidXml);
    assert_eq!(error.pack_index(), Some(1));
}

#[test]
fn duplicate_raw_json_keys_are_rejected_before_overwrite() {
    let json = br#"{
        "prefix":"test",
        "icons":{
            "rocket":{"body":"<path/>"},
            "rocket":{"body":"<circle/>"}
        }
    }"#;

    let error = IconRegistry::from_packs([IconPack::new(json)])
        .expect_err("duplicate map keys must not become last-write-wins");

    assert_eq!(error.kind(), IconRegistryBuildErrorKind::DuplicateJsonKey);
    assert_eq!(error.pack_index(), Some(0));
    assert!(!error.to_string().contains("<circle/>"));
}

#[test]
fn invalid_identifiers_and_geometry_fail_closed() {
    let uppercase = br#"{"prefix":"Test","icons":{"rocket":{"body":"<path/>"}}}"#;
    let zero_width = br#"{"prefix":"test","icons":{"rocket":{"body":"<path/>","width":0}}}"#;

    let identifier_error = IconRegistry::from_packs([IconPack::new(uppercase)])
        .expect_err("the admitted Iconify grammar is lowercase ASCII");
    assert_eq!(
        identifier_error.kind(),
        IconRegistryBuildErrorKind::InvalidIdentifier
    );

    let geometry_error = IconRegistry::from_packs([IconPack::new(zero_width)])
        .expect_err("dimensions must be finite and positive");
    assert_eq!(
        geometry_error.kind(),
        IconRegistryBuildErrorKind::InvalidGeometry
    );
}

#[test]
fn invalid_registration_names_are_never_retained_or_echoed() {
    let registration_name = "x".repeat(4 * 1024 * 1024);
    let error = IconRegistryBuilder::new()
        .add_pack(IconPack::new(br#"{}"#).with_registration_name(&registration_name))
        .expect_err("an overlong registration name must fail before it is copied");

    assert_eq!(
        error.kind(),
        IconRegistryBuildErrorKind::ResourceLimitExceeded
    );
    assert_eq!(error.registration_name(), None);
    assert!(!error.to_string().contains(&registration_name));
}

#[test]
fn registration_name_limit_accepts_exact_and_rejects_plus_one() {
    let maximum =
        usize::try_from(IconRegistryResourceLimitId::MaxPrefixBytes.fixed_value()).unwrap();
    let json = br#"{"icons":{"rocket":{"body":"<path/>"}}}"#;
    let exact_name = "a".repeat(maximum);
    let registry =
        IconRegistry::from_packs([IconPack::new(json).with_registration_name(&exact_name)])
            .expect("the exact registration-name ceiling must be admitted");
    assert_eq!(registry.len(), 1);

    let over_name = "a".repeat(maximum + 1);
    let error = IconRegistryBuilder::new()
        .add_pack(IconPack::new(json).with_registration_name(&over_name))
        .expect_err("registration-name ceiling + 1 must fail before retention");
    assert_eq!(
        error.kind(),
        IconRegistryBuildErrorKind::ResourceLimitExceeded
    );
    assert_eq!(
        error.limit_id(),
        Some(IconRegistryResourceLimitId::MaxPrefixBytes.stable_id())
    );
    assert_eq!(error.actual(), u64::try_from(maximum + 1).ok());
    assert_eq!(error.maximum(), u64::try_from(maximum).ok());
    assert_eq!(error.registration_name(), None);
    assert!(!error.to_string().contains(&over_name));
}

#[test]
fn malformed_xml_dtd_and_processing_instructions_are_rejected() {
    for body in ["<path>", "<!DOCTYPE svg><path/>", "<?icon test?><path/>"] {
        let json = pack("test", "rocket", body);
        let error = IconRegistry::from_packs([IconPack::new(&json)])
            .expect_err("external icon fragments must be strict XML");
        assert_eq!(error.kind(), IconRegistryBuildErrorKind::InvalidXml);
        assert!(!error.to_string().contains(body));
    }
}

#[test]
fn alias_cycles_missing_parents_and_icon_alias_collisions_are_errors() {
    let cases: &[(&[u8], IconRegistryBuildErrorKind)] = &[
        (
            br#"{"prefix":"test","icons":{"root":{"body":"<path/>"}},"aliases":{"a":{"parent":"b"},"b":{"parent":"a"}}}"#,
            IconRegistryBuildErrorKind::AliasCycle,
        ),
        (
            br#"{"prefix":"test","icons":{"root":{"body":"<path/>"}},"aliases":{"a":{"parent":"missing"}}}"#,
            IconRegistryBuildErrorKind::MissingAliasParent,
        ),
        (
            br#"{"prefix":"test","icons":{"same":{"body":"<path/>"}},"aliases":{"same":{"parent":"same"}}}"#,
            IconRegistryBuildErrorKind::DuplicateIcon,
        ),
    ];

    for (json, expected) in cases {
        let error = IconRegistry::from_packs([IconPack::new(json)])
            .expect_err("invalid alias graphs reject the transaction");
        assert_eq!(error.kind(), *expected);
    }
}

#[test]
fn canonical_keys_cannot_be_redefined_across_packs() {
    let first = pack("test", "rocket", "<path/>");
    let second = pack("ignored", "rocket", "<circle/>");

    let error = IconRegistry::from_packs([
        IconPack::new(&first),
        IconPack::new(&second).with_registration_name("test"),
    ])
    .expect_err("cross-pack collisions must not depend on registration order");

    assert_eq!(error.kind(), IconRegistryBuildErrorKind::DuplicateIcon);
    assert_eq!(error.registration_name(), Some("test"));
}

#[test]
fn fixed_constructor_limits_are_discoverable_and_not_caller_configurable() {
    let descriptors = icon_registry_resource_limit_descriptors();
    assert_eq!(descriptors.len(), IconRegistryResourceLimitId::ALL.len());
    assert_eq!(
        descriptors
            .iter()
            .map(|descriptor| descriptor.stable_id)
            .collect::<BTreeSet<_>>()
            .len(),
        descriptors.len()
    );
    for descriptor in descriptors {
        assert!(!descriptor.caller_configurable, "{}", descriptor.stable_id);
        assert_eq!(
            descriptor.default_value, descriptor.hard_maximum,
            "{}",
            descriptor.stable_id
        );
        assert!(descriptor.default_value > 0, "{}", descriptor.stable_id);
    }
}
