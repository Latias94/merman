//! Generation of the shared editor semantic-token descriptor projections.

use crate::XtaskError;
use merman_analysis::{AnalysisOptions, Analyzer};
use merman_editor_core::{
    DiagramDetectionValidity, DocumentKind, DocumentUri, analyze_document_context_with_shared_text,
    plan_semantic_tokens_for_snapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DESCRIPTOR_PATH: &str = "contracts/editor-language/token-descriptor-v1.json";
const EQUIVALENCE_PATH: &str = "contracts/editor-language/token-equivalence-v1.json";
const EXAMPLE_MANIFEST_PATH: &str = "playground/examples/manifest.json";
const VSCODE_MANIFEST_PATH: &str = "tools/vscode-extension/package.json";
const VSCODE_LANGUAGE_ID: &str = "mermaid";
const VSCODE_SEMANTIC_HIGHLIGHTING_SETTING: &str = "editor.semanticHighlighting.enabled";
const VSCODE_SEMANTIC_HIGHLIGHTING_ENABLED: bool = true;
const EXAMPLE_MANIFEST_SCHEMA_VERSION: u32 = 2;
const DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
const EQUIVALENCE_SCHEMA_VERSION: u32 = 1;
const SUPPORTED_PACKED_ENCODING: &str = "lsp_relative_utf16";
const SUPPORTED_PACKED_WORD_WIDTH_BITS: u32 = 32;
const SUPPORTED_PACKED_FIELD_ORDER: [&str; 5] = [
    "delta_line",
    "delta_start_utf16",
    "length_utf16",
    "token_type_code",
    "token_modifier_bits",
];
const REQUIRED_OVERLAY_IDENTITIES: [(&str, &str); 4] = [
    ("lexeme", "Lexeme"),
    ("semantic_entity", "SemanticEntity"),
    ("semantic_outline", "SemanticOutline"),
    ("semantic_payload", "SemanticPayload"),
];
const VSCODE_STANDARD_TOKEN_TYPES: [&str; 23] = [
    "namespace",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "type",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "decorator",
    "event",
    "function",
    "method",
    "macro",
    "label",
    "comment",
    "string",
    "keyword",
    "number",
    "regexp",
    "operator",
];
const VSCODE_STANDARD_TOKEN_MODIFIERS: [&str; 10] = [
    "declaration",
    "definition",
    "readonly",
    "static",
    "deprecated",
    "abstract",
    "async",
    "modification",
    "documentation",
    "defaultLibrary",
];

const GENERATED_OUTPUTS: &[(&str, ArtifactKind)] = &[
    (
        "crates/merman-editor-core/src/generated/token_descriptor.rs",
        ArtifactKind::Rust,
    ),
    (
        "platforms/web/src/generated/token-descriptor.ts",
        ArtifactKind::TypeScript,
    ),
    (
        "tools/vscode-extension/src/generated/token-descriptor.ts",
        ArtifactKind::VscodeTypeScript,
    ),
    (
        "crates/merman-core/src/generated/editor_rename_policy.rs",
        ArtifactKind::CoreRenamePolicyRust,
    ),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExampleManifest {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    #[serde(rename = "mermaidBaseline")]
    mermaid_baseline: String,
    examples: Vec<ExampleEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExampleEntry {
    #[serde(rename = "diagramType")]
    diagram_type: String,
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    category: String,
    #[allow(dead_code)]
    order: u32,
    #[allow(dead_code)]
    aliases: Vec<String>,
    fixture: String,
    evidence: ExampleEvidence,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExampleEvidence {
    role: String,
    #[allow(dead_code)]
    #[serde(default)]
    kind: Option<String>,
    #[allow(dead_code)]
    claim: String,
}

#[derive(Debug, Clone, Serialize)]
struct TokenEquivalenceCase {
    id: String,
    family: String,
    fixture: Option<String>,
    source: String,
    source_sha256: String,
    detection_validity: String,
    syntax_id: String,
    effective_layout_id: String,
    packed_words: Vec<u32>,
    packed_sha256: String,
}

#[derive(Debug, Serialize)]
struct TokenEquivalencePayload {
    schema_version: u32,
    descriptor_digest: String,
    packed_encoding: String,
    words_per_token: usize,
    family_cases: Vec<TokenEquivalenceCase>,
    recovery_cases: Vec<TokenEquivalenceCase>,
}

#[derive(Debug, Serialize)]
struct TokenEquivalenceArtifact {
    generated_by: &'static str,
    source_manifest: &'static str,
    evidence_digest: String,
    #[serde(flatten)]
    payload: TokenEquivalencePayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TokenDescriptor {
    schema_version: u32,
    rename_policies: Vec<RenamePolicy>,
    token_kinds: Vec<TokenKind>,
    modifiers: Vec<TokenModifier>,
    packed: PackedDescriptor,
    overlay_precedence: Vec<OverlayPrecedence>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RenamePolicy {
    id: String,
    rust_variant: String,
    code: u32,
    description: String,
    #[serde(rename = "default")]
    is_default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TokenKind {
    id: String,
    rust_variant: String,
    code: u32,
    lsp_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vscode: Option<VscodeTokenTypeContribution>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TokenModifier {
    id: String,
    rust_variant: String,
    code: u32,
    lsp_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vscode: Option<VscodeTokenModifierContribution>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VscodeTokenTypeContribution {
    super_type: String,
    description: String,
    scopes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VscodeTokenModifierContribution {
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PackedDescriptor {
    encoding: String,
    word_width_bits: u32,
    words_per_token: usize,
    field_order: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OverlayPrecedence {
    id: String,
    rust_variant: String,
    rank: u8,
}

#[derive(Debug, Clone, Copy)]
enum ArtifactKind {
    Rust,
    TypeScript,
    VscodeTypeScript,
    CoreRenamePolicyRust,
}

impl ArtifactKind {
    fn render(self, descriptor: &TokenDescriptor) -> Result<String, XtaskError> {
        match self {
            Self::Rust => render_rust(descriptor),
            Self::TypeScript => render_typescript(descriptor),
            Self::VscodeTypeScript => render_vscode_typescript(descriptor),
            Self::CoreRenamePolicyRust => render_core_rename_policy_rust(descriptor),
        }
    }
}

fn descriptor_error(message: impl Into<String>) -> XtaskError {
    XtaskError::VerifyFailed(format!(
        "editor token descriptor is invalid: {}",
        message.into()
    ))
}

fn read_descriptor(path: &Path) -> Result<TokenDescriptor, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let descriptor: TokenDescriptor = serde_json::from_str(&text).map_err(|error| {
        descriptor_error(format!("failed to parse {}: {error}", path.display()))
    })?;
    validate_descriptor(&descriptor)?;
    Ok(descriptor)
}

fn validate_descriptor(descriptor: &TokenDescriptor) -> Result<(), XtaskError> {
    if descriptor.schema_version != DESCRIPTOR_SCHEMA_VERSION {
        return Err(descriptor_error(format!(
            "unsupported schema {}; expected {DESCRIPTOR_SCHEMA_VERSION}",
            descriptor.schema_version
        )));
    }
    if descriptor.token_kinds.is_empty() {
        return Err(descriptor_error("at least one token kind is required"));
    }
    if descriptor.rename_policies.is_empty() {
        return Err(descriptor_error("at least one rename policy is required"));
    }
    if descriptor.modifiers.is_empty() {
        return Err(descriptor_error("at least one token modifier is required"));
    }
    if descriptor.modifiers.len() > u32::BITS as usize {
        return Err(descriptor_error(format!(
            "LSP modifier bitsets support at most {} modifiers; found {}",
            u32::BITS,
            descriptor.modifiers.len()
        )));
    }
    if descriptor.overlay_precedence.len() != REQUIRED_OVERLAY_IDENTITIES.len() {
        return Err(descriptor_error(format!(
            "schema {DESCRIPTOR_SCHEMA_VERSION} requires exactly {} overlay origins; found {}",
            REQUIRED_OVERLAY_IDENTITIES.len(),
            descriptor.overlay_precedence.len()
        )));
    }

    for kind in &descriptor.token_kinds {
        validate_identifier(&kind.id, "token-kind")?;
        validate_rust_variant(&kind.rust_variant, "token-kind")?;
        validate_lsp_name(&kind.lsp_name, "token-kind")?;
    }
    for policy in &descriptor.rename_policies {
        validate_identifier(&policy.id, "rename-policy")?;
        validate_rust_variant(&policy.rust_variant, "rename-policy")?;
        validate_single_line_description(&policy.description, &policy.id, "rename policy")?;
    }
    for modifier in &descriptor.modifiers {
        validate_identifier(&modifier.id, "modifier")?;
        validate_rust_variant(&modifier.rust_variant, "modifier")?;
        validate_lsp_name(&modifier.lsp_name, "modifier")?;
    }
    for overlay in &descriptor.overlay_precedence {
        validate_identifier(&overlay.id, "overlay")?;
        validate_rust_variant(&overlay.rust_variant, "overlay")?;
    }

    validate_unique(
        descriptor
            .rename_policies
            .iter()
            .map(|policy| policy.id.as_str()),
        "rename-policy id",
    )?;
    validate_unique(
        descriptor
            .rename_policies
            .iter()
            .map(|policy| policy.rust_variant.as_str()),
        "rename-policy Rust variant",
    )?;
    validate_unique(
        descriptor.token_kinds.iter().map(|kind| kind.id.as_str()),
        "token-kind id",
    )?;
    validate_unique(
        descriptor
            .token_kinds
            .iter()
            .map(|kind| kind.rust_variant.as_str()),
        "token-kind Rust variant",
    )?;
    validate_unique(
        descriptor
            .token_kinds
            .iter()
            .map(|kind| kind.lsp_name.as_str()),
        "token-kind LSP name",
    )?;
    validate_unique(
        descriptor
            .modifiers
            .iter()
            .map(|modifier| modifier.id.as_str()),
        "modifier id",
    )?;
    validate_unique(
        descriptor
            .modifiers
            .iter()
            .map(|modifier| modifier.rust_variant.as_str()),
        "modifier Rust variant",
    )?;
    validate_unique(
        descriptor
            .modifiers
            .iter()
            .map(|modifier| modifier.lsp_name.as_str()),
        "modifier LSP name",
    )?;
    validate_unique(
        descriptor
            .overlay_precedence
            .iter()
            .map(|overlay| overlay.id.as_str()),
        "overlay id",
    )?;
    validate_unique(
        descriptor
            .overlay_precedence
            .iter()
            .map(|overlay| overlay.rust_variant.as_str()),
        "overlay Rust variant",
    )?;
    validate_vscode_contributions(descriptor)?;
    validate_contiguous_ranks(
        descriptor
            .overlay_precedence
            .iter()
            .map(|overlay| overlay.rank),
        descriptor.overlay_precedence.len(),
    )?;
    validate_contiguous_codes(
        descriptor.rename_policies.iter().map(|policy| policy.code),
        descriptor.rename_policies.len(),
        "rename-policy",
    )?;
    validate_contiguous_codes(
        descriptor.token_kinds.iter().map(|kind| kind.code),
        descriptor.token_kinds.len(),
        "token-kind",
    )?;
    validate_contiguous_codes(
        descriptor.modifiers.iter().map(|modifier| modifier.code),
        descriptor.modifiers.len(),
        "modifier",
    )?;

    let defaults = descriptor
        .rename_policies
        .iter()
        .filter(|policy| policy.is_default)
        .count();
    if defaults != 1 {
        return Err(descriptor_error(format!(
            "rename policies require exactly one default; found {defaults}"
        )));
    }

    if descriptor.packed.encoding != SUPPORTED_PACKED_ENCODING
        || descriptor.packed.word_width_bits != SUPPORTED_PACKED_WORD_WIDTH_BITS
        || descriptor.packed.words_per_token != SUPPORTED_PACKED_FIELD_ORDER.len()
        || descriptor
            .packed
            .field_order
            .iter()
            .map(String::as_str)
            .ne(SUPPORTED_PACKED_FIELD_ORDER)
    {
        return Err(descriptor_error(format!(
            "packed tokens must use {SUPPORTED_PACKED_ENCODING}, {SUPPORTED_PACKED_WORD_WIDTH_BITS}-bit words, {} words, and field order {SUPPORTED_PACKED_FIELD_ORDER:?}",
            SUPPORTED_PACKED_FIELD_ORDER.len()
        )));
    }

    let mut overlay_identities = descriptor
        .overlay_precedence
        .iter()
        .map(|overlay| (overlay.id.as_str(), overlay.rust_variant.as_str()))
        .collect::<Vec<_>>();
    overlay_identities.sort_unstable();
    if overlay_identities != REQUIRED_OVERLAY_IDENTITIES {
        return Err(descriptor_error(format!(
            "overlay identities must be {REQUIRED_OVERLAY_IDENTITIES:?}; found {overlay_identities:?}"
        )));
    }
    Ok(())
}

fn validate_identifier(value: &str, context: &str) -> Result<(), XtaskError> {
    let valid = !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' => true,
            b'0'..=b'9' => index > 0,
            b'_' => index > 0 && index + 1 < value.len(),
            _ => false,
        })
        && !value.contains("__");
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} id `{value}` must be lower_snake_case"
        )))
    }
}

fn validate_rust_variant(value: &str, context: &str) -> Result<(), XtaskError> {
    let mut bytes = value.bytes();
    let valid = bytes.next().is_some_and(|byte| byte.is_ascii_uppercase())
        && bytes.all(|byte| byte.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} Rust variant `{value}` must be an ASCII PascalCase identifier"
        )))
    }
}

fn validate_lsp_name(value: &str, context: &str) -> Result<(), XtaskError> {
    let mut bytes = value.bytes();
    let valid = bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} LSP name `{value}` must be an ASCII lowerCamelCase identifier"
        )))
    }
}

fn validate_vscode_contributions(descriptor: &TokenDescriptor) -> Result<(), XtaskError> {
    for kind in &descriptor.token_kinds {
        let is_standard = VSCODE_STANDARD_TOKEN_TYPES.contains(&kind.lsp_name.as_str());
        match (&kind.vscode, is_standard) {
            (None, true) => {}
            (Some(_), true) => {
                return Err(descriptor_error(format!(
                    "standard VS Code token type `{}` must not be redeclared",
                    kind.lsp_name
                )));
            }
            (None, false) => {
                return Err(descriptor_error(format!(
                    "custom VS Code token type `{}` requires super_type, description, and TextMate scopes",
                    kind.lsp_name
                )));
            }
            (Some(contribution), false) => {
                if !VSCODE_STANDARD_TOKEN_TYPES.contains(&contribution.super_type.as_str()) {
                    return Err(descriptor_error(format!(
                        "custom VS Code token type `{}` has unknown standard super type `{}`",
                        kind.lsp_name, contribution.super_type
                    )));
                }
                validate_vscode_description(&contribution.description, &kind.lsp_name)?;
                if contribution.scopes.is_empty() {
                    return Err(descriptor_error(format!(
                        "custom VS Code token type `{}` requires at least one TextMate fallback scope",
                        kind.lsp_name
                    )));
                }
                validate_unique(
                    contribution.scopes.iter().map(String::as_str),
                    &format!("TextMate fallback scope for `{}`", kind.lsp_name),
                )?;
                for scope in &contribution.scopes {
                    validate_textmate_scope(scope, &kind.lsp_name)?;
                }
                if !contribution
                    .scopes
                    .iter()
                    .any(|scope| scope.split('.').any(|segment| segment == "mermaid"))
                {
                    return Err(descriptor_error(format!(
                        "custom VS Code token type `{}` requires a Mermaid-owned TextMate scope",
                        kind.lsp_name
                    )));
                }
            }
        }
    }

    for modifier in &descriptor.modifiers {
        let is_standard = VSCODE_STANDARD_TOKEN_MODIFIERS.contains(&modifier.lsp_name.as_str());
        match (&modifier.vscode, is_standard) {
            (None, true) => {}
            (Some(_), true) => {
                return Err(descriptor_error(format!(
                    "standard VS Code token modifier `{}` must not be redeclared",
                    modifier.lsp_name
                )));
            }
            (None, false) => {
                return Err(descriptor_error(format!(
                    "custom VS Code token modifier `{}` requires a description",
                    modifier.lsp_name
                )));
            }
            (Some(contribution), false) => {
                validate_vscode_description(&contribution.description, &modifier.lsp_name)?;
            }
        }
    }
    Ok(())
}

fn validate_vscode_description(value: &str, token_name: &str) -> Result<(), XtaskError> {
    validate_single_line_description(value, token_name, "VS Code contribution")
}

fn validate_single_line_description(
    value: &str,
    name: &str,
    context: &str,
) -> Result<(), XtaskError> {
    if value.is_empty()
        || value.trim() != value
        || value.bytes().any(|byte| matches!(byte, b'\r' | b'\n'))
    {
        return Err(descriptor_error(format!(
            "{context} `{name}` requires a non-empty single-line description without surrounding whitespace"
        )));
    }
    Ok(())
}

fn validate_textmate_scope(value: &str, token_name: &str) -> Result<(), XtaskError> {
    let valid = value.split('.').all(|segment| {
        !segment.is_empty()
            && segment.bytes().enumerate().all(|(index, byte)| match byte {
                b'a'..=b'z' => true,
                b'0'..=b'9' | b'_' | b'-' => index > 0,
                _ => false,
            })
    });
    if valid {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "custom VS Code token type `{token_name}` has invalid TextMate scope `{value}`"
        )))
    }
}

fn validate_unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
    context: &str,
) -> Result<(), XtaskError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(descriptor_error(format!(
                "duplicate {context} value `{value}`"
            )));
        }
    }
    Ok(())
}

fn validate_contiguous_codes(
    values: impl IntoIterator<Item = u32>,
    expected_count: usize,
    context: &str,
) -> Result<(), XtaskError> {
    let codes = values.into_iter().collect::<BTreeSet<_>>();
    let expected = (0..expected_count as u32).collect::<BTreeSet<_>>();
    if codes == expected {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "{context} codes must be contiguous 0..{}; found {codes:?}",
            expected_count - 1
        )))
    }
}

fn validate_contiguous_ranks(
    values: impl IntoIterator<Item = u8>,
    expected_count: usize,
) -> Result<(), XtaskError> {
    let ranks = values.into_iter().collect::<BTreeSet<_>>();
    let expected = (0..expected_count)
        .map(|rank| rank as u8)
        .collect::<BTreeSet<_>>();
    if ranks == expected {
        Ok(())
    } else {
        Err(descriptor_error(format!(
            "overlay ranks must be contiguous 0..{}; found {ranks:?}",
            expected_count - 1
        )))
    }
}

fn sorted_token_kinds(descriptor: &TokenDescriptor) -> Vec<&TokenKind> {
    let mut values = descriptor.token_kinds.iter().collect::<Vec<_>>();
    values.sort_by_key(|kind| kind.code);
    values
}

fn sorted_rename_policies(descriptor: &TokenDescriptor) -> Vec<&RenamePolicy> {
    let mut values = descriptor.rename_policies.iter().collect::<Vec<_>>();
    values.sort_by_key(|policy| policy.code);
    values
}

fn sorted_modifiers(descriptor: &TokenDescriptor) -> Vec<&TokenModifier> {
    let mut values = descriptor.modifiers.iter().collect::<Vec<_>>();
    values.sort_by_key(|modifier| modifier.code);
    values
}

fn sorted_overlays(descriptor: &TokenDescriptor) -> Vec<&OverlayPrecedence> {
    let mut values = descriptor.overlay_precedence.iter().collect::<Vec<_>>();
    values.sort_by_key(|overlay| overlay.rank);
    values
}

fn normalized_protocol_descriptor(descriptor: &TokenDescriptor) -> TokenDescriptor {
    let mut normalized = descriptor.clone();
    normalized.rename_policies.sort_by_key(|policy| policy.code);
    normalized.token_kinds.sort_by_key(|kind| kind.code);
    normalized.modifiers.sort_by_key(|modifier| modifier.code);
    normalized
        .overlay_precedence
        .sort_by_key(|overlay| overlay.rank);
    for kind in &mut normalized.token_kinds {
        kind.vscode = None;
    }
    for modifier in &mut normalized.modifiers {
        modifier.vscode = None;
    }
    normalized
}

fn descriptor_digest(descriptor: &TokenDescriptor) -> Result<String, XtaskError> {
    let bytes = serde_json::to_vec(&normalized_protocol_descriptor(descriptor))?;
    Ok(format!("sha256:{}", crate::util::sha256_hex(&bytes)))
}

fn valid_modifier_mask(modifier_count: usize) -> u32 {
    debug_assert!((1..=u32::BITS as usize).contains(&modifier_count));
    if modifier_count == u32::BITS as usize {
        u32::MAX
    } else {
        (1u32 << modifier_count) - 1
    }
}

fn generated_preamble(comment: &str) -> String {
    format!(
        "{comment} This file is @generated by `cargo run -p xtask -- gen-editor-token-descriptor`.\n{comment} Do not edit it directly; edit `{DESCRIPTOR_PATH}` instead.\n\n"
    )
}

fn render_rust(descriptor: &TokenDescriptor) -> Result<String, XtaskError> {
    let kinds = sorted_token_kinds(descriptor);
    let modifiers = sorted_modifiers(descriptor);
    let overlays = sorted_overlays(descriptor);
    let packed = &descriptor.packed;
    let digest = descriptor_digest(descriptor)?;
    let valid_modifier_mask = valid_modifier_mask(modifiers.len());
    let mut output = generated_preamble("//");

    writeln!(
        output,
        "pub const SEMANTIC_TOKEN_DESCRIPTOR_DIGEST: &str =\n    {digest:?};"
    )
    .unwrap();
    writeln!(
        output,
        "pub const SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN: usize = {};",
        packed.words_per_token
    )
    .unwrap();
    writeln!(
        output,
        "pub const SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX: u32 = {};",
        kinds.len() - 1
    )
    .unwrap();
    writeln!(
        output,
        "pub const SEMANTIC_TOKEN_VALID_MODIFIER_MASK: u32 = {valid_modifier_mask};\n"
    )
    .unwrap();

    output.push_str(
        "#[repr(u32)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum PlannedTokenKind {\n",
    );
    for kind in &kinds {
        writeln!(output, "    {} = {},", kind.rust_variant, kind.code).unwrap();
    }
    output.push_str("}\n\nimpl PlannedTokenKind {\n");
    writeln!(output, "    pub const ALL: [Self; {}] = [", kinds.len()).unwrap();
    for kind in &kinds {
        writeln!(output, "        Self::{},", kind.rust_variant).unwrap();
    }
    output.push_str(
        "    ];\n\n    pub const fn code(self) -> u32 {\n        self as u32\n    }\n\n    pub const fn id(self) -> &'static str {\n        SEMANTIC_TOKEN_KIND_DESCRIPTORS[self as usize].id\n    }\n\n    pub const fn lsp_name(self) -> &'static str {\n        SEMANTIC_TOKEN_KIND_DESCRIPTORS[self as usize].lsp_name\n    }\n\n    pub const fn from_code(code: u32) -> Option<Self> {\n        if code <= SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX {\n            Some(Self::ALL[code as usize])\n        } else {\n            None\n        }\n    }\n}\n\n",
    );

    output.push_str(
        "#[repr(u32)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum PlannedTokenModifier {\n",
    );
    for modifier in &modifiers {
        writeln!(output, "    {} = {},", modifier.rust_variant, modifier.code).unwrap();
    }
    output.push_str("}\n\nimpl PlannedTokenModifier {\n");
    writeln!(output, "    pub const ALL: [Self; {}] = [", modifiers.len()).unwrap();
    for modifier in &modifiers {
        writeln!(output, "        Self::{},", modifier.rust_variant).unwrap();
    }
    output.push_str(
        "    ];\n\n    pub const fn index(self) -> u32 {\n        self as u32\n    }\n\n    pub const fn bit(self) -> u32 {\n        1 << self.index()\n    }\n\n    pub const fn id(self) -> &'static str {\n        SEMANTIC_TOKEN_MODIFIER_DESCRIPTORS[self as usize].id\n    }\n\n    pub const fn lsp_name(self) -> &'static str {\n        SEMANTIC_TOKEN_MODIFIER_DESCRIPTORS[self as usize].lsp_name\n    }\n\n    pub const fn from_index(index: u32) -> Option<Self> {\n        if index < Self::ALL.len() as u32 {\n            Some(Self::ALL[index as usize])\n        } else {\n            None\n        }\n    }\n}\n\n",
    );

    output.push_str(
        "#[repr(u8)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum TokenOverlayKind {\n",
    );
    for overlay in &overlays {
        writeln!(output, "    {} = {},", overlay.rust_variant, overlay.rank).unwrap();
    }
    output.push_str(
        "}\n\nimpl TokenOverlayKind {\n    pub const fn precedence(self) -> u8 {\n        self as u8\n    }\n}\n\n",
    );

    output.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SemanticTokenKindDescriptor {\n    pub kind: PlannedTokenKind,\n    pub id: &'static str,\n    pub lsp_name: &'static str,\n    pub lsp_index: u32,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SemanticTokenModifierDescriptor {\n    pub modifier: PlannedTokenModifier,\n    pub id: &'static str,\n    pub lsp_name: &'static str,\n    pub lsp_index: u32,\n    pub bit: u32,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SemanticTokenPackedDescriptor {\n    pub encoding: &'static str,\n    pub word_width_bits: u32,\n    pub words_per_token: usize,\n    pub field_order: &'static [&'static str; SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN],\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SemanticTokenDescriptor {\n    pub schema_version: u32,\n    pub digest: &'static str,\n    pub token_kinds: &'static [SemanticTokenKindDescriptor],\n    pub modifiers: &'static [SemanticTokenModifierDescriptor],\n    pub packed: SemanticTokenPackedDescriptor,\n    pub valid_type_code_max: u32,\n    pub valid_modifier_mask: u32,\n}\n\n",
    );

    writeln!(
        output,
        "const SEMANTIC_TOKEN_KIND_DESCRIPTORS: [SemanticTokenKindDescriptor; {}] = [",
        kinds.len()
    )
    .unwrap();
    for kind in &kinds {
        writeln!(
            output,
            "    SemanticTokenKindDescriptor {{\n        kind: PlannedTokenKind::{},\n        id: {:?},\n        lsp_name: {:?},\n        lsp_index: {},\n    }},",
            kind.rust_variant, kind.id, kind.lsp_name, kind.code
        )
        .unwrap();
    }
    output.push_str("];\n\n");
    writeln!(
        output,
        "const SEMANTIC_TOKEN_MODIFIER_DESCRIPTORS: [SemanticTokenModifierDescriptor; {}] = [",
        modifiers.len()
    )
    .unwrap();
    for modifier in &modifiers {
        writeln!(
            output,
            "    SemanticTokenModifierDescriptor {{\n        modifier: PlannedTokenModifier::{},\n        id: {:?},\n        lsp_name: {:?},\n        lsp_index: {},\n        bit: {},\n    }},",
            modifier.rust_variant,
            modifier.id,
            modifier.lsp_name,
            modifier.code,
            1u32 << modifier.code
        )
        .unwrap();
    }
    output.push_str("];\n\n");
    output.push_str(
        "const SEMANTIC_TOKEN_PACKED_FIELD_ORDER: [&str; SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN] = [\n",
    );
    for field in &packed.field_order {
        writeln!(output, "    {field:?},").unwrap();
    }
    output.push_str("];\n\n");
    writeln!(
        output,
        "pub const SEMANTIC_TOKEN_DESCRIPTOR: SemanticTokenDescriptor = SemanticTokenDescriptor {{\n    schema_version: {},\n    digest: SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,\n    token_kinds: &SEMANTIC_TOKEN_KIND_DESCRIPTORS,\n    modifiers: &SEMANTIC_TOKEN_MODIFIER_DESCRIPTORS,\n    packed: SemanticTokenPackedDescriptor {{\n        encoding: {:?},\n        word_width_bits: {},\n        words_per_token: SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,\n        field_order: &SEMANTIC_TOKEN_PACKED_FIELD_ORDER,\n    }},\n    valid_type_code_max: SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX,\n    valid_modifier_mask: SEMANTIC_TOKEN_VALID_MODIFIER_MASK,\n}};\n\npub const fn semantic_token_descriptor() -> &'static SemanticTokenDescriptor {{\n    &SEMANTIC_TOKEN_DESCRIPTOR\n}}",
        descriptor.schema_version,
        packed.encoding,
        packed.word_width_bits
    )
    .unwrap();
    Ok(output)
}

fn render_core_rename_policy_rust(descriptor: &TokenDescriptor) -> Result<String, XtaskError> {
    let policies = sorted_rename_policies(descriptor);
    let mut output = generated_preamble("//");

    output.push_str("/// Grammar-owned validation policy for renaming an entity occurrence.\n");
    output.push_str("#[derive(\n");
    for derive in [
        "Debug",
        "Clone",
        "Copy",
        "Default",
        "PartialEq",
        "Eq",
        "PartialOrd",
        "Ord",
        "Hash",
        "serde::Serialize",
        "serde::Deserialize",
    ] {
        writeln!(output, "    {derive},").unwrap();
    }
    output.push_str(")]\n");
    output.push_str("pub enum EditorRenamePolicy {\n");
    for policy in &policies {
        writeln!(output, "    /// {}", policy.description).unwrap();
        if policy.is_default {
            output.push_str("    #[default]\n");
        }
        writeln!(output, "    #[serde(rename = {:?})]", policy.id).unwrap();
        writeln!(output, "    {},", policy.rust_variant).unwrap();
    }
    output.push_str("}\n\nimpl EditorRenamePolicy {\n");
    writeln!(output, "    pub const ALL: [Self; {}] = [", policies.len()).unwrap();
    for policy in &policies {
        writeln!(output, "        Self::{},", policy.rust_variant).unwrap();
    }
    output.push_str("    ];\n\n");
    writeln!(
        output,
        "    pub const IDS: [&'static str; {}] = [",
        policies.len()
    )
    .unwrap();
    for policy in &policies {
        writeln!(output, "        {:?},", policy.id).unwrap();
    }
    output.push_str(
        "    ];\n\n    pub const fn as_str(self) -> &'static str {\n        match self {\n",
    );
    for policy in &policies {
        writeln!(
            output,
            "            Self::{} => {:?},",
            policy.rust_variant, policy.id
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n}\n");
    Ok(output)
}

fn render_typescript(descriptor: &TokenDescriptor) -> Result<String, XtaskError> {
    let kinds = sorted_token_kinds(descriptor);
    let modifiers = sorted_modifiers(descriptor);
    let overlays = sorted_overlays(descriptor);
    let packed = &descriptor.packed;
    let digest = descriptor_digest(descriptor)?;
    let valid_modifier_mask = valid_modifier_mask(modifiers.len());
    let mut output = generated_preamble("//");

    writeln!(
        output,
        "export const SEMANTIC_TOKEN_DESCRIPTOR_DIGEST = {digest:?} as const;"
    )
    .unwrap();
    writeln!(
        output,
        "export const SEMANTIC_TOKEN_RECORD_WIDTH = {} as const;",
        packed.words_per_token
    )
    .unwrap();
    writeln!(
        output,
        "export const SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX = {} as const;",
        kinds.len() - 1
    )
    .unwrap();
    writeln!(
        output,
        "export const SEMANTIC_TOKEN_VALID_MODIFIER_MASK = {valid_modifier_mask} as const;\n"
    )
    .unwrap();

    output.push_str("export const EDITOR_RENAME_POLICIES = [\n");
    for policy in sorted_rename_policies(descriptor) {
        writeln!(output, "  {:?},", policy.id).unwrap();
    }
    output.push_str(
        "] as const;\n\nexport type EditorRenamePolicy =\n  (typeof EDITOR_RENAME_POLICIES)[number];\n\n",
    );

    output.push_str("export const SEMANTIC_TOKEN_TYPE_LSP_NAMES = [\n");
    for kind in &kinds {
        writeln!(output, "  {:?},", kind.lsp_name).unwrap();
    }
    output.push_str("] as const;\n\nexport const SEMANTIC_TOKEN_MODIFIER_LSP_NAMES = [\n");
    for modifier in &modifiers {
        writeln!(output, "  {:?},", modifier.lsp_name).unwrap();
    }
    output.push_str("] as const;\n\n");

    output.push_str("export const SEMANTIC_TOKEN_DESCRIPTOR = {\n");
    writeln!(
        output,
        "  schemaVersion: {},\n  digest: SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,",
        descriptor.schema_version
    )
    .unwrap();
    output.push_str("  renamePolicies: EDITOR_RENAME_POLICIES,\n");
    output.push_str("  tokenTypes: [\n");
    for kind in &kinds {
        writeln!(
            output,
            "    {{ id: {:?}, code: {}, lspName: {:?}, lspIndex: {} }},",
            kind.id, kind.code, kind.lsp_name, kind.code
        )
        .unwrap();
    }
    output.push_str("  ],\n  modifiers: [\n");
    for modifier in &modifiers {
        writeln!(
            output,
            "    {{ id: {:?}, index: {}, bit: {}, lspName: {:?}, lspIndex: {} }},",
            modifier.id,
            modifier.code,
            1u32 << modifier.code,
            modifier.lsp_name,
            modifier.code
        )
        .unwrap();
    }
    output.push_str("  ],\n  packed: {\n");
    writeln!(
        output,
        "    encoding: {:?},\n    wordWidthBits: {},\n    recordWidth: SEMANTIC_TOKEN_RECORD_WIDTH,",
        packed.encoding,
        packed.word_width_bits
    )
    .unwrap();
    output.push_str("    fieldOrder: [\n");
    for field in &packed.field_order {
        writeln!(output, "      {field:?},").unwrap();
    }
    output.push_str("    ],\n  },\n  overlayPrecedence: [\n");
    for overlay in &overlays {
        writeln!(
            output,
            "    {{ id: {:?}, rank: {} }},",
            overlay.id, overlay.rank
        )
        .unwrap();
    }
    output.push_str(
        "  ],\n  validTypeCodeMax: SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX,\n  validModifierMask: SEMANTIC_TOKEN_VALID_MODIFIER_MASK,\n  tokenTypeLspNames: SEMANTIC_TOKEN_TYPE_LSP_NAMES,\n  modifierLspNames: SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,\n} as const;\n\nexport type SemanticTokenTypeCode =\n  (typeof SEMANTIC_TOKEN_DESCRIPTOR.tokenTypes)[number][\"code\"];\nexport type SemanticTokenModifierIndex =\n  (typeof SEMANTIC_TOKEN_DESCRIPTOR.modifiers)[number][\"index\"];\n",
    );
    Ok(output)
}

fn render_vscode_typescript(descriptor: &TokenDescriptor) -> Result<String, XtaskError> {
    let mut output = render_typescript(descriptor)?;
    output.push_str("\nexport const VSCODE_CUSTOM_TOKEN_TYPES = [\n");
    for kind in sorted_token_kinds(descriptor) {
        let Some(contribution) = &kind.vscode else {
            continue;
        };
        writeln!(
            output,
            "  {{\n    id: {:?},\n    superType: {:?},\n    description: {:?},\n    scopes: [",
            kind.lsp_name, contribution.super_type, contribution.description
        )
        .unwrap();
        for scope in &contribution.scopes {
            writeln!(output, "      {scope:?},").unwrap();
        }
        output.push_str("    ],\n  },\n");
    }
    output.push_str("] as const;\n\nexport const VSCODE_CUSTOM_TOKEN_MODIFIERS = [\n");
    for modifier in sorted_modifiers(descriptor) {
        let Some(contribution) = &modifier.vscode else {
            continue;
        };
        writeln!(
            output,
            "  {{ id: {:?}, description: {:?} }},",
            modifier.lsp_name, contribution.description
        )
        .unwrap();
    }
    writeln!(
        output,
        "] as const;\n\nexport const VSCODE_MERMAID_SEMANTIC_HIGHLIGHTING_ENABLED = {VSCODE_SEMANTIC_HIGHLIGHTING_ENABLED} as const;"
    )
    .unwrap();
    Ok(output)
}

fn sha256_label(bytes: &[u8]) -> String {
    format!("sha256:{}", crate::util::sha256_hex(bytes))
}

fn read_example_manifest(root: &Path) -> Result<ExampleManifest, XtaskError> {
    let path = root.join(EXAMPLE_MANIFEST_PATH);
    let text = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let manifest: ExampleManifest = serde_json::from_str(&text).map_err(|error| {
        descriptor_error(format!(
            "failed to parse family baseline manifest {}: {error}",
            path.display()
        ))
    })?;
    if manifest.schema_version != EXAMPLE_MANIFEST_SCHEMA_VERSION {
        return Err(descriptor_error(format!(
            "family baseline manifest schema must be {EXAMPLE_MANIFEST_SCHEMA_VERSION}, found {}",
            manifest.schema_version
        )));
    }
    if manifest.mermaid_baseline != merman_core::baseline::PINNED_MERMAID_BASELINE_TAG {
        return Err(descriptor_error(format!(
            "family baseline manifest must target {}, found {}",
            merman_core::baseline::PINNED_MERMAID_BASELINE_TAG,
            manifest.mermaid_baseline
        )));
    }
    Ok(manifest)
}

fn token_equivalence_case(
    analyzer: &Analyzer,
    id: String,
    family: String,
    fixture: Option<String>,
    source: String,
) -> Result<TokenEquivalenceCase, XtaskError> {
    let analyzed = analyze_document_context_with_shared_text(
        analyzer,
        DocumentUri::new(format!("file:///editor-language/{id}.mmd")),
        1,
        Arc::from(source.as_str()),
        DocumentKind::Diagram,
    )
    .map_err(|rejection| {
        descriptor_error(format!(
            "token equivalence case `{id}` exceeded {}",
            rejection.resource_limit()
        ))
    })?;
    let detection = analyzed.detection().ok_or_else(|| {
        descriptor_error(format!(
            "token equivalence case `{id}` has no diagram detection"
        ))
    })?;
    if detection.diagram_type != family {
        return Err(descriptor_error(format!(
            "token equivalence case `{id}` expected family `{family}` but detected `{}`",
            detection.diagram_type
        )));
    }
    let plan = plan_semantic_tokens_for_snapshot(analyzed.snapshot()).map_err(|error| {
        descriptor_error(format!(
            "token equivalence case `{id}` failed semantic-token planning: {error}"
        ))
    })?;
    if plan.packed().is_empty() {
        return Err(descriptor_error(format!(
            "token equivalence case `{id}` produced no packed semantic tokens"
        )));
    }
    let packed_words = plan.packed().to_vec();
    let packed_json = serde_json::to_vec(&packed_words)?;

    Ok(TokenEquivalenceCase {
        id,
        family,
        fixture,
        source_sha256: sha256_label(source.as_bytes()),
        source,
        detection_validity: match detection.validity {
            DiagramDetectionValidity::Valid => "valid",
            DiagramDetectionValidity::RecoverableInvalid => "recoverable-invalid",
        }
        .to_string(),
        syntax_id: detection.syntax_id.clone(),
        effective_layout_id: detection.effective_layout_id.clone(),
        packed_words,
        packed_sha256: sha256_label(&packed_json),
    })
}

fn token_equivalence_artifact(
    root: &Path,
    descriptor: &TokenDescriptor,
) -> Result<String, XtaskError> {
    let manifest = read_example_manifest(root)?;
    let mut baselines = BTreeMap::new();
    for example in manifest
        .examples
        .into_iter()
        .filter(|example| example.evidence.role == "family-baseline")
    {
        let family = example.diagram_type.clone();
        if baselines.insert(family.clone(), example).is_some() {
            return Err(descriptor_error(format!(
                "family `{family}` has more than one family-baseline example"
            )));
        }
    }

    let supported = merman_core::diagram_family_capabilities()
        .iter()
        .filter_map(|capability| capability.metadata_id)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let baseline_families = baselines.keys().cloned().collect::<BTreeSet<_>>();
    if supported != baseline_families {
        return Err(descriptor_error(format!(
            "family baseline evidence must exactly cover the supported catalog; missing={:?}, unexpected={:?}",
            supported.difference(&baseline_families).collect::<Vec<_>>(),
            baseline_families.difference(&supported).collect::<Vec<_>>()
        )));
    }

    let runtime_policy = merman_core::runtime::RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)
        .expect("valid UTC offset")
        .with_fixed_today(Some(
            merman_core::time::CivilDate::new(2026, 6, 10)
                .expect("token-equivalence evidence date is valid"),
        ));
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_runtime_policy(runtime_policy));
    let mut family_cases = Vec::with_capacity(baselines.len());
    for (family, example) in baselines {
        let fixture_path = root.join(&example.fixture);
        let source = fs::read_to_string(&fixture_path).map_err(|source| XtaskError::ReadFile {
            path: fixture_path.display().to_string(),
            source,
        })?;
        let case = token_equivalence_case(
            &analyzer,
            family.clone(),
            family,
            Some(example.fixture),
            source,
        )?;
        if case.detection_validity != "valid" {
            return Err(descriptor_error(format!(
                "family baseline `{}` must be valid, found `{}`",
                case.id, case.detection_validity
            )));
        }
        family_cases.push(case);
    }

    // Keep the incomplete edge at EOF. Flowchart line endings are whitespace, so a following
    // node statement can legally become the edge target under Mermaid-compatible semantics.
    let recovery_source = "flowchart TD\n  Before -->\n".to_string();
    let recovery = token_equivalence_case(
        &analyzer,
        "flowchart-incomplete-edge".to_string(),
        "flowchart".to_string(),
        None,
        recovery_source,
    )?;
    if recovery.detection_validity != "recoverable-invalid" {
        return Err(descriptor_error(format!(
            "flowchart recovery evidence must be recoverable-invalid, found `{}`",
            recovery.detection_validity
        )));
    }

    let payload = TokenEquivalencePayload {
        schema_version: EQUIVALENCE_SCHEMA_VERSION,
        descriptor_digest: descriptor_digest(descriptor)?,
        packed_encoding: descriptor.packed.encoding.clone(),
        words_per_token: descriptor.packed.words_per_token,
        family_cases,
        recovery_cases: vec![recovery],
    };
    let evidence_digest = sha256_label(&serde_json::to_vec(&payload)?);
    let artifact = TokenEquivalenceArtifact {
        generated_by: "cargo run -p xtask -- gen-editor-token-descriptor",
        source_manifest: EXAMPLE_MANIFEST_PATH,
        evidence_digest,
        payload,
    };
    let mut output = serde_json::to_string_pretty(&artifact)?;
    output.push('\n');
    Ok(output)
}

fn generated_artifacts(descriptor: &TokenDescriptor) -> Result<Vec<(PathBuf, String)>, XtaskError> {
    GENERATED_OUTPUTS
        .iter()
        .map(|(path, kind)| Ok((PathBuf::from(path), kind.render(descriptor)?)))
        .collect()
}

fn vscode_manifest_projection(
    root: &Path,
    descriptor: &TokenDescriptor,
) -> Result<String, XtaskError> {
    let path = root.join(VSCODE_MANIFEST_PATH);
    let text = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let mut manifest: Value = serde_json::from_str(&text).map_err(|error| {
        descriptor_error(format!(
            "failed to parse VS Code manifest {}: {error}",
            path.display()
        ))
    })?;

    let manifest_object = manifest
        .as_object_mut()
        .ok_or_else(|| descriptor_error("VS Code manifest root must be an object"))?;
    let contributes = manifest_object
        .get_mut("contributes")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| descriptor_error("VS Code manifest contributes must be an object"))?;

    let custom_types = sorted_token_kinds(descriptor)
        .into_iter()
        .filter_map(|kind| {
            kind.vscode.as_ref().map(|contribution| {
                json!({
                    "id": kind.lsp_name,
                    "superType": contribution.super_type,
                    "description": contribution.description,
                })
            })
        })
        .collect::<Vec<_>>();
    let custom_modifiers = sorted_modifiers(descriptor)
        .into_iter()
        .filter_map(|modifier| {
            modifier.vscode.as_ref().map(|contribution| {
                json!({
                    "id": modifier.lsp_name,
                    "description": contribution.description,
                })
            })
        })
        .collect::<Vec<_>>();
    let mut scopes = Map::new();
    for kind in sorted_token_kinds(descriptor) {
        if let Some(contribution) = &kind.vscode {
            scopes.insert(kind.lsp_name.clone(), json!(contribution.scopes));
        }
    }

    contributes.insert("semanticTokenTypes".to_string(), Value::Array(custom_types));
    contributes.insert(
        "semanticTokenModifiers".to_string(),
        Value::Array(custom_modifiers),
    );
    contributes.insert(
        "semanticTokenScopes".to_string(),
        json!([{
            "language": VSCODE_LANGUAGE_ID,
            "scopes": scopes,
        }]),
    );

    let configuration_defaults = contributes
        .entry("configurationDefaults".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            descriptor_error("VS Code manifest configurationDefaults must be an object")
        })?;
    let mermaid_defaults = configuration_defaults
        .entry(format!("[{VSCODE_LANGUAGE_ID}]"))
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            descriptor_error("VS Code manifest [mermaid] configuration defaults must be an object")
        })?;
    mermaid_defaults.insert(
        VSCODE_SEMANTIC_HIGHLIGHTING_SETTING.to_string(),
        json!(VSCODE_SEMANTIC_HIGHLIGHTING_ENABLED),
    );

    let mut output = serde_json::to_string_pretty(&manifest)?;
    output.push('\n');
    Ok(output)
}

fn all_generated_artifacts(
    root: &Path,
    descriptor: &TokenDescriptor,
) -> Result<Vec<(PathBuf, String)>, XtaskError> {
    let mut artifacts = generated_artifacts(descriptor)?;
    artifacts.push((
        PathBuf::from(EQUIVALENCE_PATH),
        token_equivalence_artifact(root, descriptor)?,
    ));
    artifacts.push((
        PathBuf::from(VSCODE_MANIFEST_PATH),
        vscode_manifest_projection(root, descriptor)?,
    ));
    Ok(artifacts)
}

fn write_generated_artifact(path: &Path, contents: &str) -> Result<(), XtaskError> {
    let parent = path.parent().ok_or_else(|| {
        descriptor_error(format!("generated path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;
    fs::write(path, contents).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn drifted_artifacts(
    root: &Path,
    descriptor: &TokenDescriptor,
) -> Result<Vec<PathBuf>, XtaskError> {
    let mut drift = Vec::new();
    for (relative_path, expected) in generated_artifacts(descriptor)? {
        let path = root.join(&relative_path);
        let actual = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        if actual.replace("\r\n", "\n") != expected {
            drift.push(relative_path);
        }
    }
    Ok(drift)
}

fn token_equivalence_drift(root: &Path, descriptor: &TokenDescriptor) -> Result<bool, XtaskError> {
    let expected = token_equivalence_artifact(root, descriptor)?;
    let path = root.join(EQUIVALENCE_PATH);
    let actual = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    Ok(actual.replace("\r\n", "\n") != expected)
}

fn vscode_manifest_drift(root: &Path, descriptor: &TokenDescriptor) -> Result<bool, XtaskError> {
    let expected = vscode_manifest_projection(root, descriptor)?;
    let path = root.join(VSCODE_MANIFEST_PATH);
    let actual = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    Ok(actual.replace("\r\n", "\n") != expected)
}

pub(crate) fn gen_editor_token_descriptor(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    let artifacts = all_generated_artifacts(&root, &descriptor)?;
    for (relative_path, contents) in artifacts {
        write_generated_artifact(&root.join(relative_path), &contents)?;
    }
    Ok(())
}

pub(crate) fn verify_editor_token_descriptor_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    let mut drift = drifted_artifacts(&root, &descriptor)?;
    if token_equivalence_drift(&root, &descriptor)? {
        drift.push(PathBuf::from(EQUIVALENCE_PATH));
    }
    if vscode_manifest_drift(&root, &descriptor)? {
        drift.push(PathBuf::from(VSCODE_MANIFEST_PATH));
    }
    if drift.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "editor language generated contract drifted: {}; regenerate with `cargo run -p xtask -- gen-editor-token-descriptor`",
            drift
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }
}

pub(crate) fn verify_editor_token_descriptor(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    match verify_editor_token_descriptor_artifacts()? {
        Some(message) => Err(XtaskError::VerifyFailed(message)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed_descriptor() -> TokenDescriptor {
        read_descriptor(&crate::cmd::workspace_root().join(DESCRIPTOR_PATH))
            .expect("committed editor token descriptor")
    }

    #[test]
    fn descriptor_has_stable_codes_packing_precedence_and_digest() {
        let descriptor = committed_descriptor();
        assert_eq!(
            sorted_token_kinds(&descriptor)
                .into_iter()
                .map(|kind| kind.code)
                .collect::<Vec<_>>(),
            (0..descriptor.token_kinds.len() as u32).collect::<Vec<_>>()
        );
        assert_eq!(
            sorted_modifiers(&descriptor)
                .into_iter()
                .map(|modifier| modifier.code)
                .collect::<Vec<_>>(),
            (0..descriptor.modifiers.len() as u32).collect::<Vec<_>>()
        );
        assert_eq!(descriptor.packed.encoding, SUPPORTED_PACKED_ENCODING);
        assert_eq!(
            descriptor.packed.words_per_token,
            SUPPORTED_PACKED_FIELD_ORDER.len()
        );
        assert_eq!(
            descriptor.packed.field_order,
            SUPPORTED_PACKED_FIELD_ORDER.map(str::to_string)
        );
        assert_eq!(
            descriptor_digest(&descriptor).unwrap(),
            merman_editor_core::SEMANTIC_TOKEN_DESCRIPTOR_DIGEST
        );
    }

    #[test]
    fn digest_is_independent_of_json_array_order() {
        let descriptor = committed_descriptor();
        let mut reordered = descriptor.clone();
        reordered.rename_policies.reverse();
        reordered.token_kinds.reverse();
        reordered.modifiers.reverse();
        reordered.overlay_precedence.reverse();
        assert_eq!(
            descriptor_digest(&descriptor).unwrap(),
            descriptor_digest(&reordered).unwrap()
        );
    }

    #[test]
    fn protocol_digest_excludes_vscode_theme_projection_metadata() {
        let descriptor = committed_descriptor();
        let mut theme_only_change = descriptor.clone();
        let contribution = theme_only_change
            .token_kinds
            .iter_mut()
            .find(|kind| kind.lsp_name == "mermanIdentifier")
            .unwrap()
            .vscode
            .as_mut()
            .unwrap();
        contribution.description = "A different editor-facing description.".to_string();
        contribution.scopes = vec!["variable.other.changed.mermaid".to_string()];

        assert_eq!(
            descriptor_digest(&descriptor).unwrap(),
            descriptor_digest(&theme_only_change).unwrap()
        );

        let mut protocol_change = descriptor.clone();
        protocol_change.token_kinds[0].lsp_name = "mermanChangedKeyword".to_string();
        assert_ne!(
            descriptor_digest(&descriptor).unwrap(),
            descriptor_digest(&protocol_change).unwrap()
        );
    }

    #[test]
    fn validator_rejects_duplicate_gapped_and_invalid_contracts() {
        let mut duplicate_policy = committed_descriptor();
        duplicate_policy.rename_policies[1].id = duplicate_policy.rename_policies[0].id.clone();
        assert!(validate_descriptor(&duplicate_policy).is_err());

        let mut gapped_policy = committed_descriptor();
        gapped_policy.rename_policies.last_mut().unwrap().code += 1;
        assert!(validate_descriptor(&gapped_policy).is_err());

        let mut multiple_defaults = committed_descriptor();
        multiple_defaults.rename_policies[0].is_default = true;
        assert!(validate_descriptor(&multiple_defaults).is_err());

        let mut invalid_policy_description = committed_descriptor();
        invalid_policy_description.rename_policies[0].description = "invalid\nvalue".to_string();
        assert!(validate_descriptor(&invalid_policy_description).is_err());

        let mut duplicate = committed_descriptor();
        duplicate.token_kinds[1].lsp_name = duplicate.token_kinds[0].lsp_name.clone();
        assert!(validate_descriptor(&duplicate).is_err());

        let mut gap = committed_descriptor();
        gap.token_kinds.last_mut().unwrap().code += 1;
        assert!(validate_descriptor(&gap).is_err());

        let mut invalid_modifier = committed_descriptor();
        invalid_modifier.modifiers[0].lsp_name = "not_valid".to_string();
        assert!(validate_descriptor(&invalid_modifier).is_err());

        let mut invalid_packing = committed_descriptor();
        invalid_packing.packed.field_order.swap(0, 1);
        assert!(validate_descriptor(&invalid_packing).is_err());

        let mut invalid_precedence = committed_descriptor();
        invalid_precedence.overlay_precedence[0].rank = 4;
        assert!(validate_descriptor(&invalid_precedence).is_err());

        let mut missing_custom_type = committed_descriptor();
        missing_custom_type
            .token_kinds
            .iter_mut()
            .find(|kind| kind.lsp_name == "mermanIdentifier")
            .unwrap()
            .vscode = None;
        assert!(validate_descriptor(&missing_custom_type).is_err());

        let mut redeclared_standard_type = committed_descriptor();
        let keyword = redeclared_standard_type
            .token_kinds
            .iter_mut()
            .find(|kind| kind.lsp_name == "keyword")
            .unwrap();
        keyword.vscode = Some(VscodeTokenTypeContribution {
            super_type: "keyword".to_string(),
            description: "Must not redeclare a standard token.".to_string(),
            scopes: vec!["keyword.control.mermaid".to_string()],
        });
        assert!(validate_descriptor(&redeclared_standard_type).is_err());

        let mut invalid_scope = committed_descriptor();
        invalid_scope
            .token_kinds
            .iter_mut()
            .find(|kind| kind.lsp_name == "mermanIdentifier")
            .unwrap()
            .vscode
            .as_mut()
            .unwrap()
            .scopes = vec!["Variable Other".to_string()];
        assert!(validate_descriptor(&invalid_scope).is_err());
    }

    #[test]
    fn validator_allows_contiguous_legend_expansion_within_lsp_bitset_limits() {
        let mut descriptor = committed_descriptor();
        descriptor.token_kinds.push(TokenKind {
            id: "test_expansion".to_string(),
            rust_variant: "TestExpansion".to_string(),
            code: descriptor.token_kinds.len() as u32,
            lsp_name: "mermanTestExpansion".to_string(),
            vscode: Some(VscodeTokenTypeContribution {
                super_type: "variable".to_string(),
                description: "A generated test token.".to_string(),
                scopes: vec!["variable.other.test.mermaid".to_string()],
            }),
        });
        descriptor.modifiers.push(TokenModifier {
            id: "test_expansion".to_string(),
            rust_variant: "TestExpansion".to_string(),
            code: descriptor.modifiers.len() as u32,
            lsp_name: "mermanTestExpansion".to_string(),
            vscode: Some(VscodeTokenModifierContribution {
                description: "A generated test modifier.".to_string(),
            }),
        });

        validate_descriptor(&descriptor).unwrap();
        assert!(render_rust(&descriptor).unwrap().contains("TestExpansion"));
        assert!(
            render_typescript(&descriptor)
                .unwrap()
                .contains("mermanTestExpansion")
        );
        let vscode = render_vscode_typescript(&descriptor).unwrap();
        assert!(vscode.contains("VSCODE_CUSTOM_TOKEN_TYPES"));
        assert!(vscode.contains("variable.other.test.mermaid"));
        assert!(vscode.contains("VSCODE_CUSTOM_TOKEN_MODIFIERS"));
    }

    #[test]
    fn overlay_precedence_is_owned_by_the_descriptor() {
        let mut descriptor = committed_descriptor();
        let lexeme = descriptor
            .overlay_precedence
            .iter_mut()
            .find(|overlay| overlay.id == "lexeme")
            .unwrap();
        lexeme.rank = 3;
        let entity = descriptor
            .overlay_precedence
            .iter_mut()
            .find(|overlay| overlay.id == "semantic_entity")
            .unwrap();
        entity.rank = 0;

        validate_descriptor(&descriptor).unwrap();
        let rust = render_rust(&descriptor).unwrap();
        let typescript = render_typescript(&descriptor).unwrap();
        assert!(rust.contains("Lexeme = 3"));
        assert!(rust.contains("SemanticEntity = 0"));
        assert!(typescript.contains("{ id: \"lexeme\", rank: 3 }"));
        assert!(typescript.contains("{ id: \"semantic_entity\", rank: 0 }"));
    }

    #[test]
    fn validator_rejects_more_modifiers_than_the_lsp_u32_bitset_can_hold() {
        let mut descriptor = committed_descriptor();
        while descriptor.modifiers.len() <= u32::BITS as usize {
            let index = descriptor.modifiers.len();
            descriptor.modifiers.push(TokenModifier {
                id: format!("test_modifier_{index}"),
                rust_variant: format!("TestModifier{index}"),
                code: index as u32,
                lsp_name: format!("testModifier{index}"),
                vscode: Some(VscodeTokenModifierContribution {
                    description: format!("Generated test modifier {index}."),
                }),
            });
        }

        assert!(validate_descriptor(&descriptor).is_err());
    }

    #[test]
    fn parser_rejects_unknown_descriptor_fields() {
        let mut value = serde_json::to_value(committed_descriptor()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("legacy".to_string(), serde_json::Value::Bool(true));
        assert!(serde_json::from_value::<TokenDescriptor>(value).is_err());
    }

    #[test]
    fn drift_check_reports_only_changed_projection() {
        let descriptor = committed_descriptor();
        let temporary = tempfile::tempdir().unwrap();
        for (relative_path, contents) in generated_artifacts(&descriptor).unwrap() {
            write_generated_artifact(&temporary.path().join(relative_path), &contents).unwrap();
        }
        let changed = PathBuf::from(GENERATED_OUTPUTS[1].0);
        fs::write(temporary.path().join(&changed), "stale projection\n").unwrap();
        assert_eq!(
            drifted_artifacts(temporary.path(), &descriptor).unwrap(),
            vec![changed]
        );
    }

    #[test]
    fn vscode_manifest_projects_custom_tokens_scopes_and_enablement() {
        let descriptor = committed_descriptor();
        let temporary = tempfile::tempdir().unwrap();
        let manifest_path = temporary.path().join(VSCODE_MANIFEST_PATH);
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            "{\n  \"name\": \"test\",\n  \"contributes\": {\n    \"commands\": []\n  }\n}\n",
        )
        .unwrap();

        let projected: Value = serde_json::from_str(
            &vscode_manifest_projection(temporary.path(), &descriptor).unwrap(),
        )
        .unwrap();
        let contributes = &projected["contributes"];
        let types = contributes["semanticTokenTypes"].as_array().unwrap();
        let modifiers = contributes["semanticTokenModifiers"].as_array().unwrap();
        assert_eq!(types.len(), 10);
        assert_eq!(modifiers.len(), 4);
        assert!(types.iter().all(|entry| {
            !VSCODE_STANDARD_TOKEN_TYPES.contains(&entry["id"].as_str().unwrap())
                && entry["superType"].is_string()
                && entry["description"].is_string()
        }));
        assert!(modifiers.iter().all(|entry| {
            !VSCODE_STANDARD_TOKEN_MODIFIERS.contains(&entry["id"].as_str().unwrap())
                && entry["description"].is_string()
        }));
        assert_eq!(contributes["semanticTokenScopes"][0]["language"], "mermaid");
        assert_eq!(
            contributes["semanticTokenScopes"][0]["scopes"]
                .as_object()
                .unwrap()
                .len(),
            10
        );
        assert_eq!(
            contributes["configurationDefaults"]["[mermaid]"]["editor.semanticHighlighting.enabled"],
            true
        );
        assert!(contributes["commands"].is_array());
    }

    #[test]
    fn committed_generated_artifacts_have_no_drift() {
        assert_eq!(verify_editor_token_descriptor_artifacts().unwrap(), None);
    }
}
