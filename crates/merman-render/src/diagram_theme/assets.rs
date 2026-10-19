use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use super::admission::{FontEmbeddingRequirement, FontSource, ThemeAdmissionError};
use super::resources::{ThemeResourceLimitExceeded, ThemeResourcePolicy};

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
pub struct FontCatalogSpec {
    assets: Vec<FontAssetSpec>,
    aliases: Vec<(String, String)>,
    generic_families: Vec<(GenericFontFamily, String)>,
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
        self.aliases.push((alias.into(), target.into()));
        self
    }

    pub fn with_generic_family(
        mut self,
        generic: GenericFontFamily,
        target: impl Into<String>,
    ) -> Self {
        self.generic_families.push((generic, target.into()));
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

    /// Checks custom font input under host-owned ceilings before rejecting unsupported assets.
    ///
    /// Embedded theme fonts are unavailable. Input limits and source validation still run first;
    /// constructing a spec never grants a larger budget than the policy selected by the host.
    pub fn compile(self, resources: &ThemeResourcePolicy) -> Result<FontCatalog, FontCatalogError> {
        compile_font_catalog(self, resources)
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

#[derive(Debug, Clone)]
pub struct FontCatalog(FontCatalogFingerprint);

impl FontCatalog {
    /// Creates the asset-free catalog used when text may resolve through host system fonts.
    ///
    /// This does not enumerate or freeze fonts installed on the host. Its fingerprint identifies
    /// the system-font catalog mode, not the host's installed font set or output portability.
    pub fn system_fonts() -> Self {
        // SHA-256 of the established length-prefixed catalog domain and "default-parity".
        // This identifies one fixed mode, so constructing it does not require hashing.
        Self(FontCatalogFingerprint([
            0xc6, 0xaa, 0x7a, 0xf7, 0x33, 0x22, 0xaa, 0xc3, 0x5c, 0xe4, 0x54, 0x83, 0x69, 0x14,
            0x08, 0x48, 0xc9, 0xa4, 0xd0, 0x93, 0x70, 0x0a, 0xbc, 0xcb, 0x06, 0x3e, 0xa4, 0x7a,
            0x17, 0x97, 0xd0, 0xaa,
        ]))
    }

    pub fn available_sources(&self) -> impl ExactSizeIterator<Item = FontSource> + '_ {
        std::iter::once(FontSource::System)
    }

    pub fn fingerprint(&self) -> FontCatalogFingerprint {
        self.0
    }
}

fn compile_font_catalog(
    spec: FontCatalogSpec,
    resources: &ThemeResourcePolicy,
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

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

/// Font input admission failures. Embedded theme font compilation is unavailable.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FontCatalogError {
    #[error(transparent)]
    ResourceLimit(#[from] ThemeResourceLimitExceeded),
    #[error(transparent)]
    Admission(#[from] ThemeAdmissionError),
    #[error("embedded theme font resources are not supported")]
    EmbeddedFontsUnavailable,
    #[error("font catalog must contain at least one custom asset")]
    EmptyCatalog,
    #[error("font catalog with custom bytes must declare the embedded source")]
    EmbeddedSourceRequired,
    #[error("font asset id `{id}` is duplicated")]
    DuplicateAssetId { id: String },
    #[error("font asset {asset_index} uses an unsupported container")]
    UnsupportedFontContainer { asset_index: usize },
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

    #[test]
    fn custom_font_assets_are_not_supported() {
        for bytes in [b"wOF2", b"OTTO", b"ttcf", &[0, 1, 0, 0]] {
            let spec = FontCatalogSpec::new([FontAssetSpec::new("font", bytes)]);
            assert!(matches!(
                spec.compile(&ThemeResourcePolicy::interactive()),
                Err(FontCatalogError::EmbeddedFontsUnavailable)
            ));
        }
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
            let error = spec.compile(&policy).unwrap_err();
            assert!(
                matches!(error, FontCatalogError::ResourceLimit(ThemeResourceLimitExceeded { limit, .. }) if limit == expected),
                "expected {expected}, got {error}"
            );
        }
    }

    #[test]
    fn system_font_catalog_retains_no_custom_bytes() {
        let catalog = FontCatalog::system_fonts();

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
