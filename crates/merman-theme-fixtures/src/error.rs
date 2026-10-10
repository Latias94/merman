use crate::model::{
    ExpectedOutputTarget, ExpectedPortabilityGrade, MermaidStyleFamily, ReferenceDiagramFamily,
    ReferenceThemeFacet, ReferenceThemeMechanism,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("theme fixture manifest is invalid JSON: {0}")]
    InvalidJson(serde_json::Error),
    #[error("unsupported theme fixture schema version {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("C6 acceptance catalog is invalid JSON: {0}")]
    InvalidC6AcceptanceJson(serde_json::Error),
    #[error("unsupported C6 acceptance schema version {0}")]
    UnsupportedC6AcceptanceSchemaVersion(u32),
    #[error("C6 acceptance manifest is invalid: {reason}")]
    InvalidC6AcceptanceManifest { reason: String },
    #[error(
        "C6 acceptance manifest lineage mismatch; declared predecessor {declared}, actual {actual}"
    )]
    C6AcceptanceManifestLineageMismatch { declared: String, actual: String },
    #[error("C6 proof recipe `{key}` is duplicated")]
    DuplicateC6ProofRecipe { key: String },
    #[error("C6 proof recipe set mismatch; missing {missing:?}, unexpected {unexpected:?}")]
    C6ProofRecipeSetMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
    #[error("C6 proof recipe `{key}` is invalid: {reason}")]
    InvalidC6ProofRecipe { key: String, reason: String },
    #[error("C6 critical mechanism set mismatch; missing {missing:?}, unexpected {unexpected:?}")]
    C6CriticalMechanismSetMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
    #[error("C6 critical mechanisms are not covered by the acceptance cells: {mechanisms:?}")]
    UncoveredC6CriticalMechanisms { mechanisms: Vec<String> },
    #[error("C6 acceptance cell `{key}` is duplicated")]
    DuplicateC6AcceptanceCell { key: String },
    #[error("C6 acceptance cell set mismatch; missing {missing:?}, unexpected {unexpected:?}")]
    C6AcceptanceCellSetMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
    #[error("C6 acceptance cell `{key}` is invalid: {reason}")]
    InvalidC6AcceptanceCell { key: String, reason: String },
    #[error("theme fixture root `{path}` is invalid: {source}")]
    InvalidRoot {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("theme fixture root `{0}` is not a directory")]
    RootNotDirectory(PathBuf),
    #[error("could not read theme fixture file `{path}`: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("theme fixture file `{path}` is not UTF-8: {source}")]
    InvalidUtf8File {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
    #[error("theme fixture path `{0}` resolves outside the fixture root")]
    EscapingPath(String),
    #[error("theme fixture path `{0}` is not a file")]
    NotAFile(PathBuf),
    #[error("invalid {kind} path `{path}`")]
    InvalidPath { path: String, kind: &'static str },
    #[error("invalid catalog id `{0}`")]
    InvalidId(String),
    #[error("catalog id `{id}` is duplicated between {first_kind} and {second_kind} records")]
    DuplicateId {
        id: String,
        first_kind: &'static str,
        second_kind: &'static str,
    },
    #[error("source `{0}` is incomplete")]
    IncompleteSource(String),
    #[error("source `{source_id}` evidence snapshot is invalid JSON: {error}")]
    InvalidSourceSnapshot {
        source_id: String,
        error: serde_json::Error,
    },
    #[error("source `{0}` evidence snapshot does not match its manifest record")]
    SourceSnapshotMismatch(String),
    #[error(
        "source `{source_id}` checkout revision mismatch; expected {expected}, actual {actual}"
    )]
    SourceCheckoutRevisionMismatch {
        source_id: String,
        expected: String,
        actual: String,
    },
    #[error("required source `{0}` is missing")]
    MissingRequiredSource(String),
    #[error("modern_mermaid evidence snapshot is invalid JSON: {0}")]
    InvalidModernMermaidSnapshot(serde_json::Error),
    #[error("modern_mermaid evidence snapshot does not match the catalog")]
    ModernMermaidSnapshotMismatch,
    #[error("modern_mermaid theme `{theme}` exceeds the pinned source line {source_line}")]
    ModernMermaidThemeSourceLineMismatch { theme: String, source_line: u32 },
    #[error("Excalidraw font snapshot is invalid JSON: {0}")]
    InvalidExcalidrawFontSnapshot(serde_json::Error),
    #[error("Excalidraw font snapshot does not match the pinned asset catalog")]
    ExcalidrawFontSnapshotMismatch,
    #[error("Mermaid style precedence snapshot is invalid JSON: {0}")]
    InvalidMermaidStylePrecedenceSnapshot(serde_json::Error),
    #[error("Mermaid style precedence snapshot does not match the pinned closed matrix")]
    MermaidStylePrecedenceSnapshotMismatch,
    #[error(
        "Mermaid style evidence `{reference}` exceeds the pinned source file's {line_count} lines"
    )]
    MermaidStyleEvidenceRangeOutOfBounds {
        reference: String,
        line_count: usize,
    },
    #[error("record `{record}` has invalid pinned revision `{revision}`")]
    InvalidRevision { record: String, revision: String },
    #[error("record `{record}` references unknown `{field}` value `{target}`")]
    UnknownReference {
        record: String,
        field: &'static str,
        target: String,
    },
    #[error("unknown theme fixture lookup `{0}`")]
    UnknownLookup(String),
    #[error("asset `{0}` has incomplete font metadata")]
    InvalidAssetMetadata(String),
    #[error("asset `{0}` has an invalid CSS unicode range")]
    InvalidUnicodeRange(String),
    #[error("asset `{0}` does not preserve its pinned upstream bytes")]
    AssetSourceHashMismatch(String),
    #[error("fixture `{fixture}` contains uncovered codepoint U+{codepoint:04X}")]
    UncoveredFixtureCodepoint { fixture: String, codepoint: u32 },
    #[error("fixture `{fixture}` font family `{family}` does not resolve to a declared asset")]
    UnresolvedFixtureFontFamily { fixture: String, family: String },
    #[error(
        "fixture `{fixture}` places controlled font family `{family}` after generic fallback `{generic}`"
    )]
    FixtureFontAfterGenericFallback {
        fixture: String,
        family: String,
        generic: String,
    },
    #[error("fixture `{fixture}` declares font asset `{asset}` that is not selected by its stack")]
    UnusedFixtureFontAsset { fixture: String, asset: String },
    #[error("fixture `{fixture}` source could not be parsed for visible text: {message}")]
    InvalidFixtureSource { fixture: String, message: String },
    #[error(
        "fixture `{fixture}` source compatibility mechanism set mismatch; expected {expected:?}, actual {actual:?}"
    )]
    SourceCompatibilityMechanismMismatch {
        fixture: String,
        expected: BTreeSet<ReferenceThemeMechanism>,
        actual: BTreeSet<ReferenceThemeMechanism>,
    },
    #[error(
        "fixture `{fixture}` source style evidence set mismatch; expected {expected:?}, actual {actual:?}"
    )]
    SourceStyleEvidenceMismatch {
        fixture: String,
        expected: BTreeSet<String>,
        actual: BTreeSet<String>,
    },
    #[error(
        "typed fixture `{fixture}` contains source-owned visual evidence: mechanisms {mechanisms:?}, style evidence {style_evidence_ids:?}"
    )]
    TypedFixtureContainsSourceEvidence {
        fixture: String,
        mechanisms: BTreeSet<ReferenceThemeMechanism>,
        style_evidence_ids: BTreeSet<String>,
    },
    #[error("fixture `{fixture}` themeCSS contains unsupported or invalid content: {message}")]
    UnmodeledThemeCss { fixture: String, message: String },
    #[error("style precedence entry `{0}` has no source-compatibility fixture consumer")]
    UnconsumedStyleEvidence(String),
    #[error("fixture `{0}` declares typography but has no parsed visible text")]
    MissingFixtureTextSamples(String),
    #[error("record `{record}` has invalid SHA-256 `{value}`")]
    InvalidSha256 { record: String, value: String },
    #[error("record `{record}` hash mismatch for `{path}`: expected {expected}, actual {actual}")]
    HashMismatch {
        record: String,
        path: String,
        expected: String,
        actual: String,
    },
    #[error("record `{record}` has an empty evidence file `{path}`")]
    EmptyEvidenceFile { record: String, path: String },
    #[error("fixture `{fixture}` has an invalid expectation: {source}")]
    InvalidFixtureExpectation {
        fixture: String,
        source: serde_json::Error,
    },
    #[error("fixture `{fixture}` has an invalid typed theme input: {source}")]
    InvalidThemeInput {
        fixture: String,
        source: serde_json::Error,
    },
    #[error("fixture `{0}` must declare both theme input path and SHA-256")]
    IncompleteThemeInputReference(String),
    #[error("fixture `{0}` does not match its closed typed theme input")]
    ThemeInputMismatch(String),
    #[error("fixture `{0}` has an invalid semantic rule or empty style effect")]
    InvalidSemanticRule(String),
    #[error(
        "fixture `{fixture}` theme input asset set mismatch; expected {expected:?}, actual {actual:?}"
    )]
    ThemeInputAssetSetMismatch {
        fixture: String,
        expected: BTreeSet<String>,
        actual: BTreeSet<String>,
    },
    #[error("fixture `{0}` does not match its closed expectation")]
    FixtureExpectationMismatch(String),
    #[error("fixture `{fixture}` references unknown style evidence `{id}`")]
    UnknownStyleEvidence { fixture: String, id: String },
    #[error(
        "fixture `{fixture}` has source family {source_family:?}, but style evidence `{id}` belongs to {evidence_family:?}"
    )]
    StyleEvidenceFamilyMismatch {
        fixture: String,
        id: String,
        source_family: ReferenceDiagramFamily,
        evidence_family: MermaidStyleFamily,
    },
    #[error("fixture `{0}` must declare at least one evidence source")]
    IncompleteFixture(String),
    #[error("duplicate translation for reference mechanism {0:?}")]
    DuplicateTranslation(ReferenceThemeMechanism),
    #[error("invalid translation for reference mechanism {0:?}")]
    InvalidTranslation(ReferenceThemeMechanism),
    #[error("reference translation set mismatch; expected {expected:?}, actual {actual:?}")]
    TranslationSetMismatch {
        expected: BTreeSet<ReferenceThemeMechanism>,
        actual: BTreeSet<ReferenceThemeMechanism>,
    },
    #[error(
        "modern_mermaid reference theme set mismatch; expected {expected:?}, actual {actual:?}"
    )]
    ReferenceThemeSetMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("theme `{0}` is incomplete")]
    IncompleteTheme(String),
    #[error("theme `{theme}` lacks fixture evidence for source mechanism {mechanism:?}")]
    UncoveredSourceMechanism {
        theme: String,
        mechanism: ReferenceThemeMechanism,
    },
    #[error("theme `{theme}` lacks fixture evidence for source facet {facet:?}")]
    UncoveredSourceFacet {
        theme: String,
        facet: ReferenceThemeFacet,
    },
    #[error("theme `{theme}` target {target:?} lacks typed evidence for mechanism {mechanism:?}")]
    UncoveredTargetMechanism {
        theme: String,
        target: ExpectedOutputTarget,
        mechanism: ReferenceThemeMechanism,
    },
    #[error("theme `{theme}` has an incomplete {target:?} expectation")]
    IncompleteTarget {
        theme: String,
        target: ExpectedOutputTarget,
    },
    #[error("theme `{theme}` has invalid {target:?} residual `{residual}`")]
    InvalidTargetResidual {
        theme: String,
        target: ExpectedOutputTarget,
        residual: String,
    },
    #[error(
        "theme `{theme}` {target:?} residual set mismatch; expected {expected:?}, actual {actual:?}"
    )]
    TargetResidualSetMismatch {
        theme: String,
        target: ExpectedOutputTarget,
        expected: BTreeSet<ReferenceThemeMechanism>,
        actual: BTreeSet<ReferenceThemeMechanism>,
    },
    #[error("portable theme `{theme}` target {target:?} cannot declare residuals")]
    PortableTargetHasResidual {
        theme: String,
        target: ExpectedOutputTarget,
    },
    #[error("unverified theme `{theme}` target {target:?} requires a residual")]
    UnverifiedTargetMissingResidual {
        theme: String,
        target: ExpectedOutputTarget,
    },
    #[error("Aurora must declare browser-backdrop-blur for every output target")]
    MissingAuroraBackdropResidual,
    #[error("reference theme `{theme}` unexpectedly has {grade:?} portability for {target:?}")]
    UnexpectedReferenceThemePortability {
        theme: String,
        target: ExpectedOutputTarget,
        grade: ExpectedPortabilityGrade,
    },
}
