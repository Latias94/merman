use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::admission::{FontEmbeddingRequirement, FontSource, ThemeAdmissionError};
use super::resources::{ThemeResourceLimitExceeded, ThemeResourcePolicy};
use super::typography::FontStack;

const FONT_CATALOG_FINGERPRINT_DOMAIN: &[u8] = b"merman-font-catalog-v1";
#[cfg(feature = "embedded-fonts")]
mod embedded;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FontContainer {
    TrueType,
    OpenType,
    Collection,
    Woff2,
}

impl FontContainer {
    pub const ALL: &'static [Self] = &[
        Self::TrueType,
        Self::OpenType,
        Self::Collection,
        Self::Woff2,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::TrueType => "truetype",
            Self::OpenType => "opentype",
            Self::Collection => "collection",
            Self::Woff2 => "woff2",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl FontStyle {
    pub const ALL: &'static [Self] = &[Self::Normal, Self::Italic, Self::Oblique];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique => "oblique",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FontEmbeddingPermissions {
    Installable,
    Editable,
    PreviewAndPrint,
    Restricted,
    Unknown,
}

impl FontEmbeddingPermissions {
    #[cfg(feature = "embedded-fonts")]
    const fn id(self) -> &'static str {
        match self {
            Self::Installable => "installable",
            Self::Editable => "editable",
            Self::PreviewAndPrint => "preview-and-print",
            Self::Restricted => "restricted",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum GenericFontFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    SystemUi,
}

impl GenericFontFamily {
    pub const ALL: &'static [Self] = &[
        Self::Serif,
        Self::SansSerif,
        Self::Monospace,
        Self::Cursive,
        Self::Fantasy,
        Self::SystemUi,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Serif => "serif",
            Self::SansSerif => "sans-serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
            Self::SystemUi => "system-ui",
        }
    }

    pub(crate) fn from_css_keyword(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|family| family.id().eq_ignore_ascii_case(value))
    }
}

#[derive(Clone)]
pub struct FontAssetSpec {
    id: String,
    bytes: Arc<[u8]>,
}

impl FontAssetSpec {
    pub fn new(id: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        Self {
            id: id.into(),
            bytes: Arc::from(bytes.as_ref()),
        }
    }

    pub fn from_shared_bytes(id: impl Into<String>, bytes: Arc<[u8]>) -> Self {
        Self {
            id: id.into(),
            bytes,
        }
    }
}

impl fmt::Debug for FontAssetSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontAssetSpec")
            .field("id", &self.id)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
struct FontFamilyAliasSpec {
    alias: String,
    target: String,
}

#[derive(Debug, Clone)]
struct GenericFamilySpec {
    generic: GenericFontFamily,
    target: String,
}

#[derive(Debug, Clone)]
pub struct FontCatalogSpec {
    assets: Vec<FontAssetSpec>,
    aliases: Vec<FontFamilyAliasSpec>,
    generic_families: Vec<GenericFamilySpec>,
    available_sources: BTreeSet<FontSource>,
    embedding: FontEmbeddingRequirement,
}

impl FontCatalogSpec {
    pub fn new(assets: impl IntoIterator<Item = FontAssetSpec>) -> Self {
        Self {
            assets: assets.into_iter().collect(),
            aliases: Vec::new(),
            generic_families: Vec::new(),
            available_sources: [FontSource::Embedded].into_iter().collect(),
            embedding: FontEmbeddingRequirement::NoEmbedding,
        }
    }

    pub fn with_alias(mut self, alias: impl Into<String>, target: impl Into<String>) -> Self {
        self.aliases.push(FontFamilyAliasSpec {
            alias: alias.into(),
            target: target.into(),
        });
        self
    }

    pub fn with_generic_family(
        mut self,
        generic: GenericFontFamily,
        target: impl Into<String>,
    ) -> Self {
        self.generic_families.push(GenericFamilySpec {
            generic,
            target: target.into(),
        });
        self
    }

    pub fn with_available_sources(mut self, sources: impl IntoIterator<Item = FontSource>) -> Self {
        self.available_sources = sources.into_iter().collect();
        self
    }

    pub fn with_embedding_requirement(mut self, requirement: FontEmbeddingRequirement) -> Self {
        self.embedding = requirement;
        self
    }

    /// Validates and canonicalizes this catalog under host-owned resource ceilings.
    ///
    /// Later theme compilers use the same operation; constructing a spec never grants a larger
    /// budget than the policy selected by the host.
    pub fn compile(self, resources: &ThemeResourcePolicy) -> Result<FontCatalog, FontCatalogError> {
        self.compile_with_embedded_fonts_allowed(resources, cfg!(feature = "embedded-fonts"))
    }

    /// Applies the artifact capability policy even when Cargo has unified optional features.
    pub(crate) fn compile_with_embedded_fonts_allowed(
        self,
        resources: &ThemeResourcePolicy,
        allowed: bool,
    ) -> Result<FontCatalog, FontCatalogError> {
        compile_font_catalog(self, resources, allowed)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontCatalogFingerprint([u8; 32]);

impl FontCatalogFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex_bytes(&self.0)
    }
}

impl fmt::Debug for FontCatalogFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FontCatalogFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontAssetFingerprint([u8; 32]);

impl FontAssetFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex_bytes(&self.0)
    }
}

impl fmt::Debug for FontAssetFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FontAssetFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

#[derive(Clone)]
pub struct FontAsset {
    id: String,
    input_container: FontContainer,
    canonical_container: FontContainer,
    canonical_bytes: Arc<[u8]>,
    fingerprint: FontAssetFingerprint,
    admission_receipt: FontAssetAdmissionReceipt,
}

impl FontAsset {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn input_container(&self) -> FontContainer {
        self.input_container
    }

    pub const fn canonical_container(&self) -> FontContainer {
        self.canonical_container
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns shared immutable canonical bytes without copying the retained asset.
    pub fn canonical_data(&self) -> Arc<[u8]> {
        Arc::clone(&self.canonical_bytes)
    }

    pub const fn fingerprint(&self) -> FontAssetFingerprint {
        self.fingerprint
    }
}

impl fmt::Debug for FontAsset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontAsset")
            .field("id", &self.id)
            .field("input_container", &self.input_container)
            .field("canonical_container", &self.canonical_container)
            .field("canonical_bytes", &self.canonical_bytes.len())
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

/// Compile-time facts that cannot be recovered from the retained canonical font bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FontAssetAdmissionReceipt {
    compressed_bytes: Option<usize>,
    expansion_source_bytes: usize,
}

impl FontAssetAdmissionReceipt {
    fn validate(
        self,
        canonical_bytes: &[u8],
        resources: &ThemeResourcePolicy,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        if let Some(compressed_bytes) = self.compressed_bytes {
            resources.check_font_asset_compressed_bytes(compressed_bytes)?;
        }
        resources.check_font_asset_decoded_bytes(canonical_bytes.len())?;
        resources.check_font_decoded_expansion(self.expansion_source_bytes, canonical_bytes.len())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FontCatalogAdmissionReceipt {
    deduplicated_decoded_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFaceMetadata {
    asset_id: String,
    face_index: u32,
    family_name: String,
    postscript_name: Option<String>,
    style: FontStyle,
    weight: u16,
    width: u16,
    permissions: FontEmbeddingPermissions,
    subsetting_allowed: bool,
    outline_embedding_allowed: bool,
    table_count: usize,
}

impl FontFaceMetadata {
    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub const fn face_index(&self) -> u32 {
        self.face_index
    }

    pub fn family_name(&self) -> &str {
        &self.family_name
    }

    pub fn postscript_name(&self) -> Option<&str> {
        self.postscript_name.as_deref()
    }

    pub const fn style(&self) -> FontStyle {
        self.style
    }

    pub const fn weight(&self) -> u16 {
        self.weight
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn permissions(&self) -> FontEmbeddingPermissions {
        self.permissions
    }

    pub const fn subsetting_allowed(&self) -> bool {
        self.subsetting_allowed
    }

    pub const fn outline_embedding_allowed(&self) -> bool {
        self.outline_embedding_allowed
    }

    pub const fn table_count(&self) -> usize {
        self.table_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamilyAlias {
    alias: String,
    target: String,
}

impl FontFamilyAlias {
    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

#[derive(Debug)]
struct FontCatalogInner {
    assets: Vec<FontAsset>,
    faces: Vec<FontFaceMetadata>,
    aliases: Vec<FontFamilyAlias>,
    generic_families: BTreeMap<GenericFontFamily, String>,
    named_family_names: BTreeMap<String, String>,
    resolved_family_names: BTreeMap<String, String>,
    available_sources: BTreeSet<FontSource>,
    embedding: FontEmbeddingRequirement,
    fingerprint: FontCatalogFingerprint,
    admission_receipt: FontCatalogAdmissionReceipt,
}

#[derive(Debug, Clone)]
pub struct FontCatalog(Arc<FontCatalogInner>);

impl FontCatalog {
    /// Creates the asset-free catalog used when text may resolve through host system fonts.
    ///
    /// This does not enumerate or freeze fonts installed on the host. Its fingerprint identifies
    /// the system-font catalog mode, not the host's installed font set or output portability.
    pub fn system_fonts() -> Self {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, FONT_CATALOG_FINGERPRINT_DOMAIN);
        // Preserve the canonical identity of the unchanged asset-free catalog. The former public
        // constructor name described an assurance claim, not different catalog content.
        update_len_prefixed(&mut hasher, b"default-parity");
        Self(Arc::new(FontCatalogInner {
            assets: Vec::new(),
            faces: Vec::new(),
            aliases: Vec::new(),
            generic_families: BTreeMap::new(),
            named_family_names: BTreeMap::new(),
            resolved_family_names: BTreeMap::new(),
            available_sources: [FontSource::System].into_iter().collect(),
            embedding: FontEmbeddingRequirement::NoEmbedding,
            fingerprint: FontCatalogFingerprint(hasher.finalize().into()),
            admission_receipt: FontCatalogAdmissionReceipt::default(),
        }))
    }

    pub fn assets(&self) -> &[FontAsset] {
        &self.0.assets
    }

    pub fn faces(&self) -> &[FontFaceMetadata] {
        &self.0.faces
    }

    pub fn aliases(&self) -> &[FontFamilyAlias] {
        &self.0.aliases
    }

    pub fn generic_family(&self, family: GenericFontFamily) -> Option<&str> {
        self.0.generic_families.get(&family).map(String::as_str)
    }

    pub fn available_sources(&self) -> impl ExactSizeIterator<Item = FontSource> + '_ {
        self.0.available_sources.iter().copied()
    }

    pub fn embedding_requirement(&self) -> FontEmbeddingRequirement {
        self.0.embedding
    }

    pub fn fingerprint(&self) -> FontCatalogFingerprint {
        self.0.fingerprint
    }

    pub(crate) fn requires_prepared_text_layout(&self) -> bool {
        !self.0.assets.is_empty()
    }

    pub(crate) fn validate_retained_resources(
        &self,
        resources: &ThemeResourcePolicy,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        resources.check_font_asset_count(self.0.assets.len())?;
        for asset in &self.0.assets {
            asset
                .admission_receipt
                .validate(&asset.canonical_bytes, resources)?;
        }
        resources.check_font_catalog_decoded_bytes(
            self.0.admission_receipt.deduplicated_decoded_bytes,
        )?;
        resources.check_font_face_count(self.0.faces.len())?;
        resources
            .check_font_table_count(self.0.faces.iter().map(FontFaceMetadata::table_count).sum())?;
        resources.check_font_alias_count(self.0.aliases.len() + self.0.generic_families.len())
    }

    pub(crate) fn canonical_family_name(&self, requested: &str) -> Option<&str> {
        self.0
            .resolved_family_names
            .get(&normalize_family_key(requested))
            .map(String::as_str)
    }

    pub(crate) fn canonical_named_family_name(&self, requested: &str) -> Option<&str> {
        self.0
            .named_family_names
            .get(&normalize_family_key(requested))
            .map(String::as_str)
    }

    pub(crate) fn admit_font_stack(&self, requested: &FontStack) -> Option<FontStack> {
        let mut admitted = Vec::new();
        let mut admitted_keys = BTreeSet::new();
        for family in requested.families() {
            let Some(canonical) = self.canonical_family_name(family) else {
                continue;
            };
            let key = normalize_family_key(canonical);
            if admitted_keys.insert(key) {
                admitted.push(canonical.to_string());
            }
        }
        FontStack::new(admitted).ok()
    }
}

fn compile_font_catalog(
    spec: FontCatalogSpec,
    resources: &ThemeResourcePolicy,
    embedded_fonts_allowed: bool,
) -> Result<FontCatalog, FontCatalogError> {
    resources.check_font_asset_count(spec.assets.len())?;
    let total_input_bytes = spec
        .assets
        .iter()
        .try_fold(0usize, |total, asset| total.checked_add(asset.bytes.len()));
    resources.check_theme_encoded_bytes(
        total_input_bytes.ok_or(FontCatalogError::CatalogByteCountOverflow)?,
    )?;
    let mapping_count = spec
        .aliases
        .len()
        .checked_add(spec.generic_families.len())
        .ok_or(FontCatalogError::CatalogCountOverflow)?;
    resources.check_font_alias_count(mapping_count)?;
    if spec.assets.is_empty() {
        return Err(FontCatalogError::EmptyCatalog);
    }
    if spec.available_sources.is_empty() {
        return Err(FontCatalogError::Admission(
            ThemeAdmissionError::EmptyThemeFontSources,
        ));
    }
    if !spec.available_sources.contains(&FontSource::Embedded) {
        return Err(FontCatalogError::EmbeddedSourceRequired);
    }

    if embedded_fonts_allowed && cfg!(feature = "embedded-fonts") {
        #[cfg(feature = "embedded-fonts")]
        return embedded::compile(spec, resources);
    }

    // Apply limits that can be checked without a font parser before rejecting the capability.
    for asset in &spec.assets {
        if detect_font_container(&asset.bytes) == Some(FontContainer::Woff2) {
            resources.check_font_asset_compressed_bytes(asset.bytes.len())?;
        } else {
            resources.check_font_asset_decoded_bytes(asset.bytes.len())?;
        }
    }
    Err(FontCatalogError::EmbeddedFontsUnavailable)
}

pub(super) fn detect_font_container(bytes: &[u8]) -> Option<FontContainer> {
    let magic = bytes.get(..4)?;
    match magic {
        [0x00, 0x01, 0x00, 0x00] | b"true" | b"typ1" => Some(FontContainer::TrueType),
        b"OTTO" => Some(FontContainer::OpenType),
        b"ttcf" => Some(FontContainer::Collection),
        b"wOF2" => Some(FontContainer::Woff2),
        _ => None,
    }
}

fn normalize_family_key(name: &str) -> String {
    name.trim().to_lowercase()
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontAssetIdError {
    Empty,
    TooLong,
    InvalidCharacter,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FontCatalogError {
    #[error(transparent)]
    ResourceLimit(#[from] ThemeResourceLimitExceeded),
    #[error(transparent)]
    Admission(#[from] ThemeAdmissionError),
    #[error("embedded font assets are unavailable in this artifact")]
    EmbeddedFontsUnavailable,
    #[error("font catalog must contain at least one custom asset")]
    EmptyCatalog,
    #[error("font catalog with custom bytes must declare the embedded source")]
    EmbeddedSourceRequired,
    #[error("font asset {index} has an invalid id: {reason:?}")]
    InvalidAssetId {
        index: usize,
        reason: FontAssetIdError,
    },
    #[error("font asset id `{id}` is duplicated")]
    DuplicateAssetId { id: String },
    #[error("font asset {asset_index} uses an unsupported container")]
    UnsupportedFontContainer { asset_index: usize },
    #[error("font asset {asset_index} is not a valid WOFF2 file")]
    MalformedWoff2 { asset_index: usize },
    #[error(
        "font asset {asset_index} WOFF2 header declares {declared} SFNT bytes but decoder produced {actual}"
    )]
    Woff2SizeMismatch {
        asset_index: usize,
        declared: usize,
        actual: usize,
    },
    #[error("font asset {asset_index} did not produce a valid canonical SFNT")]
    MalformedSfnt { asset_index: usize },
    #[error("font asset `{asset_id}` is not a valid font collection")]
    MalformedCollection { asset_id: String },
    #[error("font face `{asset_id}` index {face_index} is malformed")]
    MalformedFace { asset_id: String, face_index: u32 },
    #[error("font face `{asset_id}` index {face_index} has no Unicode family name")]
    MissingFamilyName { asset_id: String, face_index: u32 },
    #[error("font face `{asset_id}` index {face_index} has an invalid family name")]
    InvalidFamilyName { asset_id: String, face_index: u32 },
    #[error("font face `{asset_id}` index {face_index} has an invalid PostScript name")]
    InvalidPostscriptName { asset_id: String, face_index: u32 },
    #[error(
        "font face `{asset_id}` index {face_index} denies {requirement:?} embedding ({permissions:?})"
    )]
    EmbeddingDenied {
        asset_id: String,
        face_index: u32,
        requirement: FontEmbeddingRequirement,
        permissions: FontEmbeddingPermissions,
    },
    #[error("font faces contain case-insensitive family-name collisions")]
    FamilyNameCollision,
    #[error("font alias {index} is invalid")]
    InvalidAlias { index: usize },
    #[error("font alias target {index} is invalid")]
    InvalidAliasTarget { index: usize },
    #[error("font alias `{alias}` collides with another family or alias")]
    AliasCollision { alias: String },
    #[error("font alias {index} targets an unknown catalog family")]
    UnknownAliasTarget { index: usize },
    #[error("generic font family `{family:?}` is assigned more than once")]
    DuplicateGenericFamily { family: GenericFontFamily },
    #[error("generic font family `{family:?}` has an invalid target")]
    InvalidGenericTarget { family: GenericFontFamily },
    #[error("generic font family `{family:?}` targets an unknown catalog family")]
    UnknownGenericTarget { family: GenericFontFamily },
    #[error("font catalog byte accounting overflowed")]
    CatalogByteCountOverflow,
    #[error("font catalog item accounting overflowed")]
    CatalogCountOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_compile_uses_the_owner_local_theme_resource_policy() {
        let compile: fn(
            FontCatalogSpec,
            &ThemeResourcePolicy,
        ) -> Result<FontCatalog, FontCatalogError> = FontCatalogSpec::compile;
        let _ = compile;
    }

    #[test]
    fn empty_custom_catalog_does_not_select_system_fonts() {
        assert!(matches!(
            FontCatalogSpec::new([]).compile(&ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::EmptyCatalog)
        ));
    }

    #[cfg(not(feature = "embedded-fonts"))]
    #[test]
    fn custom_font_assets_require_the_embedded_fonts_capability() {
        for bytes in [b"wOF2", b"OTTO", b"ttcf", &[0, 1, 0, 0]] {
            let spec = FontCatalogSpec::new([FontAssetSpec::new("font", bytes)]);
            assert!(matches!(
                spec.compile(&ThemeResourcePolicy::interactive()),
                Err(FontCatalogError::EmbeddedFontsUnavailable)
            ));
        }
    }

    #[test]
    fn artifact_policy_can_reject_embedded_fonts_independently_of_cargo_features() {
        let spec = FontCatalogSpec::new([FontAssetSpec::new("font", b"wOF2")]);
        assert!(matches!(
            spec.compile_with_embedded_fonts_allowed(&ThemeResourcePolicy::interactive(), false),
            Err(FontCatalogError::EmbeddedFontsUnavailable)
        ));
        assert!(matches!(
            FontCatalogSpec::new([])
                .compile_with_embedded_fonts_allowed(&ThemeResourcePolicy::interactive(), false),
            Err(FontCatalogError::EmptyCatalog)
        ));
    }

    #[test]
    fn resource_limits_precede_missing_embedded_fonts_capability() {
        use super::super::resources::ThemeResourceLimitId;

        for (limit, bytes, alias, expected) in [
            (
                ThemeResourceLimitId::MaxFontAssets,
                b"wOF2",
                false,
                "max_font_assets",
            ),
            (
                ThemeResourceLimitId::MaxThemeEncodedBytes,
                b"wOF2",
                false,
                "max_theme_encoded_bytes",
            ),
            (
                ThemeResourceLimitId::MaxFontAliases,
                b"wOF2",
                true,
                "max_font_aliases",
            ),
            (
                ThemeResourceLimitId::MaxFontAssetCompressedBytes,
                b"wOF2",
                false,
                "max_font_asset_compressed_bytes",
            ),
            (
                ThemeResourceLimitId::MaxFontAssetDecodedBytes,
                b"OTTO",
                false,
                "max_font_asset_decoded_bytes",
            ),
        ] {
            let mut spec = FontCatalogSpec::new([FontAssetSpec::new("font", bytes)]);
            if alias {
                spec = spec.with_alias("Alias", "Font");
            }
            let policy = ThemeResourcePolicy::interactive()
                .with_limit(limit, 0)
                .unwrap();
            let error = spec
                .compile_with_embedded_fonts_allowed(&policy, false)
                .unwrap_err();
            assert!(
                matches!(error, FontCatalogError::ResourceLimit(ThemeResourceLimitExceeded { limit, .. }) if limit == expected),
                "expected {expected}, got {error}"
            );
        }
    }

    #[test]
    fn system_font_catalog_retains_no_custom_bytes() {
        let catalog = FontCatalog::system_fonts();

        assert!(!catalog.requires_prepared_text_layout());
        assert!(catalog.assets().is_empty());
        assert!(catalog.faces().is_empty());
        assert_eq!(
            catalog.fingerprint().to_hex(),
            "c6aa7af73322aac35ce4548369140848c9a4d093700abccb063ea47a1797d0aa"
        );
        assert_eq!(
            catalog.available_sources().collect::<Vec<_>>(),
            vec![FontSource::System]
        );
    }
}
