//! Host font-source policy and CSS font stacks.

use std::{fmt, sync::Arc};

use super::is_css_wide_keyword;

const MAX_FONT_STACK_ENTRIES: usize = 32;
const MAX_FONT_FAMILY_BYTES: usize = 256;

/// Source from which a text or export backend may resolve a font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FontSource {
    Embedded,
    System,
}

impl FontSource {
    pub const ALL: &'static [Self] = &[Self::Embedded, Self::System];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::System => "system",
        }
    }
}

impl std::fmt::Display for FontSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Ordered host-owned font-source policy.
///
/// The order is priority and the contained values are the allowed set. Restriction always keeps
/// the order of the receiver, so an operation cannot replace host priority with request priority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSourcePolicy {
    priority: &'static [FontSource],
}

impl FontSourcePolicy {
    pub fn new(
        priority: impl IntoIterator<Item = FontSource>,
    ) -> Result<Self, FontSourcePolicyError> {
        let mut priority = priority.into_iter();
        let first = priority.next().ok_or(FontSourcePolicyError::Empty)?;
        let Some(second) = priority.next() else {
            return Ok(match first {
                FontSource::Embedded => Self::embedded_only(),
                FontSource::System => Self::system_only(),
            });
        };
        if first == second {
            return Err(FontSourcePolicyError::Duplicate(second));
        }
        if let Some(duplicate) = priority.next() {
            return Err(FontSourcePolicyError::Duplicate(duplicate));
        }
        Ok(match first {
            FontSource::Embedded => Self::embedded_then_system(),
            FontSource::System => Self::system_then_embedded(),
        })
    }

    pub fn embedded_only() -> Self {
        Self {
            priority: &[FontSource::Embedded],
        }
    }

    pub fn system_only() -> Self {
        Self {
            priority: &[FontSource::System],
        }
    }

    pub fn embedded_then_system() -> Self {
        Self {
            priority: &[FontSource::Embedded, FontSource::System],
        }
    }

    pub fn system_then_embedded() -> Self {
        Self {
            priority: &[FontSource::System, FontSource::Embedded],
        }
    }

    pub fn priority(&self) -> impl ExactSizeIterator<Item = FontSource> + '_ {
        self.priority.iter().copied()
    }

    pub fn contains(&self, source: FontSource) -> bool {
        self.priority.contains(&source)
    }

    pub fn first(&self) -> FontSource {
        self.priority[0]
    }

    pub fn restrict_with(&self, restriction: &Self) -> Result<Self, FontSourcePolicyError> {
        self.restrict_to(restriction.priority.iter().copied())
    }

    pub fn restrict_to(
        &self,
        allowed: impl IntoIterator<Item = FontSource>,
    ) -> Result<Self, FontSourcePolicyError> {
        let mut embedded = false;
        let mut system = false;
        for source in allowed {
            match source {
                FontSource::Embedded => embedded = true,
                FontSource::System => system = true,
            }
        }
        match (
            embedded && self.contains(FontSource::Embedded),
            system && self.contains(FontSource::System),
        ) {
            (true, true) => Ok(self.clone()),
            (true, false) => Ok(Self::embedded_only()),
            (false, true) => Ok(Self::system_only()),
            (false, false) => Err(FontSourcePolicyError::EmptyIntersection),
        }
    }
}

impl Default for FontSourcePolicy {
    fn default() -> Self {
        Self::embedded_then_system()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FontSourcePolicyError {
    #[error("font-source policy must allow at least one source")]
    Empty,
    #[error("font-source policy contains duplicate source `{0}`")]
    Duplicate(FontSource),
    #[error("font-source policies have no source in common")]
    EmptyIntersection,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FontStackError {
    #[error("font stack must contain between 1 and 32 families")]
    InvalidLength,
    #[error("invalid CSS font family")]
    InvalidFamily,
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

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontStack {
    families: Arc<[String]>,
}

pub(super) fn validate_font_stack_families(
    families: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Vec<String>, FontStackError> {
    let families = families
        .into_iter()
        .map(Into::into)
        .map(validate_font_family)
        .collect::<Result<Vec<_>, _>>()?;
    if families.is_empty() || families.len() > MAX_FONT_STACK_ENTRIES {
        return Err(FontStackError::InvalidLength);
    }
    Ok(families)
}

impl FontStack {
    pub fn new(
        families: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, FontStackError> {
        Ok(Self {
            families: validate_font_stack_families(families)?.into(),
        })
    }

    pub fn single(family: impl Into<String>) -> Result<Self, FontStackError> {
        Self::new([family])
    }

    pub fn families(&self) -> &[String] {
        &self.families
    }

    pub fn as_css(&self) -> String {
        self.families
            .iter()
            .map(|family| {
                if is_css_generic_family(family) {
                    family.to_ascii_lowercase()
                } else if is_unquoted_css_font_family(family) && !is_css_wide_keyword(family) {
                    family.clone()
                } else {
                    let mut serialized = String::new();
                    cssparser::serialize_string(family, &mut serialized)
                        .expect("serializing a CSS string into String cannot fail");
                    serialized
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_unquoted_css_font_family(value: &str) -> bool {
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return false;
    }

    let mut serialized = String::new();
    cssparser::serialize_identifier(value, &mut serialized)
        .expect("serializing a CSS identifier into String cannot fail");
    serialized == value
}

fn is_css_generic_family(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
    )
}

impl Default for FontStack {
    fn default() -> Self {
        Self {
            families: Arc::from([
                "trebuchet ms".to_string(),
                "verdana".to_string(),
                "arial".to_string(),
                "sans-serif".to_string(),
            ]),
        }
    }
}

fn validate_font_family(family: String) -> Result<String, FontStackError> {
    let family = family.trim().to_string();
    if family.is_empty()
        || family.len() > MAX_FONT_FAMILY_BYTES
        || family.chars().any(|character| {
            character.is_control() || matches!(character, ';' | '{' | '}' | '<' | '>')
        })
    {
        return Err(FontStackError::InvalidFamily);
    }
    Ok(family)
}
