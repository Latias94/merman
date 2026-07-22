use merman_editor_core::{
    PlannedTokenKind, PlannedTokenModifier, SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
    SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN, SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
    SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX, semantic_token_descriptor,
};

#[test]
fn generated_descriptor_exposes_the_stable_schema_one_contract() {
    let descriptor = semantic_token_descriptor();

    assert_eq!(descriptor.schema_version, 1);
    let digest = descriptor
        .digest
        .strip_prefix("sha256:")
        .expect("descriptor digest must use SHA-256 provenance");
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(descriptor.digest, SEMANTIC_TOKEN_DESCRIPTOR_DIGEST);
    assert_eq!(descriptor.token_kinds.len(), 22);
    assert_eq!(descriptor.modifiers.len(), 9);
    assert_eq!(descriptor.packed.encoding, "lsp_relative_utf16");
    assert_eq!(descriptor.packed.word_width_bits, 32);
    assert_eq!(
        descriptor.packed.words_per_token,
        SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN
    );
    assert_eq!(
        descriptor.packed.field_order,
        &[
            "delta_line",
            "delta_start_utf16",
            "length_utf16",
            "token_type_code",
            "token_modifier_bits",
        ]
    );
    assert_eq!(
        descriptor.valid_type_code_max,
        SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX
    );
    assert_eq!(
        descriptor.valid_modifier_mask,
        SEMANTIC_TOKEN_VALID_MODIFIER_MASK
    );
}

#[test]
fn generated_codes_lsp_indices_and_modifier_bits_are_contiguous() {
    let descriptor = semantic_token_descriptor();

    for (index, kind) in descriptor.token_kinds.iter().enumerate() {
        assert_eq!(kind.kind.code(), index as u32);
        assert_eq!(kind.lsp_index, index as u32);
        assert_eq!(PlannedTokenKind::from_code(index as u32), Some(kind.kind));
        assert_eq!(kind.kind.id(), kind.id);
        assert_eq!(kind.kind.lsp_name(), kind.lsp_name);
    }
    assert_eq!(PlannedTokenKind::from_code(22), None);

    for (index, modifier) in descriptor.modifiers.iter().enumerate() {
        assert_eq!(modifier.modifier.index(), index as u32);
        assert_eq!(modifier.lsp_index, index as u32);
        assert_eq!(modifier.bit, 1 << index);
        assert_eq!(
            PlannedTokenModifier::from_index(index as u32),
            Some(modifier.modifier)
        );
        assert_eq!(modifier.modifier.id(), modifier.id);
        assert_eq!(modifier.modifier.lsp_name(), modifier.lsp_name);
    }
    assert_eq!(PlannedTokenModifier::from_index(9), None);
    assert_eq!(SEMANTIC_TOKEN_VALID_MODIFIER_MASK, (1 << 9) - 1);
}
