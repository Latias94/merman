use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use brotli_decompressor::{
    Allocator, BrotliDecompressStream, BrotliResult, BrotliState, SliceWrapper, SliceWrapperMut,
};
use sha2::{Digest, Sha256};
use ttf_parser::{Face, Permissions, PlatformId, Style, name_id};

use super::{
    FONT_CATALOG_FINGERPRINT_DOMAIN, FontAsset, FontAssetAdmissionReceipt, FontAssetFingerprint,
    FontAssetIdError, FontAssetSpec, FontCatalog, FontCatalogAdmissionReceipt, FontCatalogError,
    FontCatalogFingerprint, FontCatalogInner, FontCatalogSpec, FontContainer,
    FontEmbeddingPermissions, FontEmbeddingRequirement, FontFaceMetadata, FontFamilyAlias,
    FontFamilyAliasSpec, FontSource, FontStyle, GenericFamilySpec, GenericFontFamily,
    ThemeResourcePolicy, detect_font_container, normalize_family_key, update_len_prefixed,
};

const FONT_ASSET_FINGERPRINT_DOMAIN: &[u8] = b"merman-font-asset-v1";
const MAX_FONT_ASSET_ID_BYTES: usize = 128;
const MAX_FONT_FAMILY_NAME_BYTES: usize = 512;
const MAX_FONT_NAME_CANDIDATES: usize = 8;
const MAX_BROTLI_WORKING_BYTES: usize = 32 * 1024 * 1024;

struct CanonicalizedFontAsset {
    input_container: FontContainer,
    canonical_container: FontContainer,
    canonical_bytes: Arc<[u8]>,
    admission_receipt: FontAssetAdmissionReceipt,
}

pub(super) fn compile(
    mut spec: FontCatalogSpec,
    resources: &ThemeResourcePolicy,
) -> Result<FontCatalog, FontCatalogError> {
    for (index, asset) in spec.assets.iter().enumerate() {
        validate_asset_id(&asset.id)
            .map_err(|reason| FontCatalogError::InvalidAssetId { index, reason })?;
    }
    let mut indexed_assets = std::mem::take(&mut spec.assets)
        .into_iter()
        .enumerate()
        .collect::<Vec<_>>();
    indexed_assets.sort_by(|left, right| left.1.id.cmp(&right.1.id));
    for pair in indexed_assets.windows(2) {
        if pair[0].1.id == pair[1].1.id {
            return Err(FontCatalogError::DuplicateAssetId {
                id: pair[0].1.id.clone(),
            });
        }
    }

    let mut assets = Vec::with_capacity(indexed_assets.len());
    let mut faces = Vec::new();
    let mut canonical_assets = BTreeMap::<FontAssetFingerprint, Arc<[u8]>>::new();
    let mut total_decoded_bytes = 0usize;
    let mut total_tables = 0usize;
    for (asset_index, asset) in indexed_assets {
        let FontAssetSpec { id, bytes } = asset;
        let CanonicalizedFontAsset {
            input_container,
            canonical_container,
            canonical_bytes,
            admission_receipt,
        } = canonicalize_font_asset(asset_index, bytes, resources)?;
        let fingerprint = hash_font_asset(canonical_container, &canonical_bytes);
        let canonical_bytes = if let Some(retained) = canonical_assets.get(&fingerprint) {
            Arc::clone(retained)
        } else {
            total_decoded_bytes = total_decoded_bytes
                .checked_add(canonical_bytes.len())
                .ok_or(FontCatalogError::CatalogByteCountOverflow)?;
            resources.check_font_catalog_decoded_bytes(total_decoded_bytes)?;
            canonical_assets.insert(fingerprint, Arc::clone(&canonical_bytes));
            canonical_bytes
        };

        let asset_faces = parse_font_faces(
            &id,
            &canonical_bytes,
            canonical_container,
            spec.embedding,
            resources,
            faces.len(),
        )?;
        for face in &asset_faces {
            total_tables = total_tables
                .checked_add(face.table_count)
                .ok_or(FontCatalogError::CatalogCountOverflow)?;
            resources.check_font_table_count(total_tables)?;
        }

        assets.push(FontAsset {
            id,
            input_container,
            canonical_container,
            canonical_bytes,
            fingerprint,
            admission_receipt,
        });
        faces.extend(asset_faces);
    }

    faces.sort_by(|left, right| {
        left.family_name
            .cmp(&right.family_name)
            .then(left.weight.cmp(&right.weight))
            .then(left.style.cmp(&right.style))
            .then(left.asset_id.cmp(&right.asset_id))
            .then(left.face_index.cmp(&right.face_index))
    });
    let families = collect_family_names(&faces)?;
    let aliases = compile_aliases(spec.aliases, &families)?;
    let generic_families = compile_generic_families(spec.generic_families, &families, &aliases)?;
    let named_family_names = compile_named_family_names(&families, &aliases);
    let resolved_family_names =
        compile_resolved_family_names(&families, &aliases, &generic_families);
    let fingerprint = hash_font_catalog(
        &assets,
        &faces,
        &aliases,
        &generic_families,
        &spec.available_sources,
        spec.embedding,
    );
    Ok(FontCatalog(Arc::new(FontCatalogInner {
        assets,
        faces,
        aliases,
        generic_families,
        named_family_names,
        resolved_family_names,
        available_sources: spec.available_sources,
        embedding: spec.embedding,
        fingerprint,
        admission_receipt: FontCatalogAdmissionReceipt {
            deduplicated_decoded_bytes: total_decoded_bytes,
        },
    })))
}

fn compile_named_family_names(
    families: &BTreeMap<String, String>,
    aliases: &[FontFamilyAlias],
) -> BTreeMap<String, String> {
    let mut resolved = BTreeMap::new();
    for alias in aliases {
        resolved.insert(
            normalize_family_key(alias.alias()),
            alias.target().to_string(),
        );
    }
    for (key, family) in families {
        resolved
            .entry(key.clone())
            .or_insert_with(|| family.clone());
    }
    resolved
}

fn compile_resolved_family_names(
    families: &BTreeMap<String, String>,
    aliases: &[FontFamilyAlias],
    generic_families: &BTreeMap<GenericFontFamily, String>,
) -> BTreeMap<String, String> {
    let mut resolved = BTreeMap::new();
    for alias in aliases {
        resolved.insert(
            normalize_family_key(alias.alias()),
            alias.target().to_string(),
        );
    }
    for (generic, target) in generic_families {
        resolved
            .entry(generic.id().to_string())
            .or_insert_with(|| target.clone());
    }
    for (key, family) in families {
        resolved
            .entry(key.clone())
            .or_insert_with(|| family.clone());
    }
    resolved
}

fn canonicalize_font_asset(
    asset_index: usize,
    bytes: Arc<[u8]>,
    resources: &ThemeResourcePolicy,
) -> Result<CanonicalizedFontAsset, FontCatalogError> {
    let input_container = detect_font_container(&bytes)
        .ok_or(FontCatalogError::UnsupportedFontContainer { asset_index })?;
    let input_len = bytes.len();
    let mut compressed_block_size = None;
    let canonical_bytes = if input_container == FontContainer::Woff2 {
        resources.check_font_asset_compressed_bytes(input_len)?;
        let plan = preflight_woff2(&bytes, resources, asset_index)?;
        compressed_block_size = Some(plan.total_compressed_size);
        let declared_size = plan.declared_sfnt_size;

        let mut resource_error = None;
        let plan_for_decode = plan.clone();
        let decoded = wuff::decompress_woff2_with_custom_brotli(
            &bytes,
            &mut |compressed_data, expected_size| {
                if let Err(error) = resources.check_font_asset_decoded_bytes(expected_size) {
                    resource_error = Some(error);
                    return Err(Box::new(BrotliDecodeError));
                }
                if let Err(error) = resources.check_font_decoded_expansion(
                    plan_for_decode.total_compressed_size,
                    expected_size,
                ) {
                    resource_error = Some(error);
                    return Err(Box::new(BrotliDecodeError));
                }
                if expected_size != plan_for_decode.uncompressed_size {
                    return Err(Box::new(BrotliDecodeError));
                }
                let decoded = decompress_brotli_exact(compressed_data, expected_size)?;
                if let Err(error) =
                    validate_woff2_reconstruction(&decoded, &plan_for_decode, resources)
                {
                    if let FontCatalogError::ResourceLimit(resource) = error {
                        resource_error = Some(resource);
                        return Err(Box::new(BrotliDecodeError));
                    }
                    return Err(Box::new(error));
                }
                Ok(decoded)
            },
        );
        if let Some(error) = resource_error {
            return Err(FontCatalogError::ResourceLimit(error));
        }
        let decoded = decoded.map_err(|_| FontCatalogError::MalformedWoff2 { asset_index })?;
        if decoded.len() != declared_size {
            return Err(FontCatalogError::Woff2SizeMismatch {
                asset_index,
                declared: declared_size,
                actual: decoded.len(),
            });
        }
        Arc::<[u8]>::from(decoded)
    } else {
        resources.check_font_asset_decoded_bytes(input_len)?;
        bytes
    };

    resources.check_font_asset_decoded_bytes(canonical_bytes.len())?;
    resources.check_font_decoded_expansion(
        compressed_block_size.unwrap_or(input_len),
        canonical_bytes.len(),
    )?;
    let canonical_container = detect_font_container(&canonical_bytes)
        .filter(|container| *container != FontContainer::Woff2)
        .ok_or(FontCatalogError::MalformedSfnt { asset_index })?;
    Ok(CanonicalizedFontAsset {
        input_container,
        canonical_container,
        admission_receipt: FontAssetAdmissionReceipt {
            compressed_bytes: (input_container == FontContainer::Woff2).then_some(input_len),
            expansion_source_bytes: compressed_block_size.unwrap_or(input_len),
        },
        canonical_bytes,
    })
}

fn parse_font_faces(
    asset_id: &str,
    bytes: &[u8],
    container: FontContainer,
    embedding: FontEmbeddingRequirement,
    resources: &ThemeResourcePolicy,
    existing_face_count: usize,
) -> Result<Vec<FontFaceMetadata>, FontCatalogError> {
    let face_count = if container == FontContainer::Collection {
        let declared_count = bytes
            .get(8..12)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_be_bytes)
            .ok_or_else(|| FontCatalogError::MalformedCollection {
                asset_id: asset_id.to_string(),
            })?;
        if declared_count == 0 {
            return Err(FontCatalogError::MalformedCollection {
                asset_id: asset_id.to_string(),
            });
        }
        let declared_count =
            usize::try_from(declared_count).map_err(|_| FontCatalogError::CatalogCountOverflow)?;
        let declared_total = existing_face_count
            .checked_add(declared_count)
            .ok_or(FontCatalogError::CatalogCountOverflow)?;
        resources.check_font_face_count(declared_total)?;
        ttf_parser::fonts_in_collection(bytes)
            .filter(|count| *count > 0)
            .ok_or_else(|| FontCatalogError::MalformedCollection {
                asset_id: asset_id.to_string(),
            })?
    } else {
        1
    };
    let face_count =
        usize::try_from(face_count).map_err(|_| FontCatalogError::CatalogCountOverflow)?;
    let next_face_count = existing_face_count
        .checked_add(face_count)
        .ok_or(FontCatalogError::CatalogCountOverflow)?;
    resources.check_font_face_count(next_face_count)?;
    let mut faces = Vec::with_capacity(face_count);
    for face_index in 0..face_count as u32 {
        let face = Face::parse(bytes, face_index).map_err(|_| FontCatalogError::MalformedFace {
            asset_id: asset_id.to_string(),
            face_index,
        })?;
        let family_name =
            preferred_face_name(&face, &[name_id::TYPOGRAPHIC_FAMILY, name_id::FAMILY])
                .ok_or_else(|| FontCatalogError::MissingFamilyName {
                    asset_id: asset_id.to_string(),
                    face_index,
                })?;
        validate_family_name(&family_name).map_err(|_| FontCatalogError::InvalidFamilyName {
            asset_id: asset_id.to_string(),
            face_index,
        })?;
        let postscript_name = preferred_face_name(&face, &[name_id::POST_SCRIPT_NAME]);
        if let Some(name) = &postscript_name {
            validate_family_name(name).map_err(|_| FontCatalogError::InvalidPostscriptName {
                asset_id: asset_id.to_string(),
                face_index,
            })?;
        }
        let permissions = map_permissions(face.permissions());
        let outline_embedding_allowed = face.is_outline_embedding_allowed();
        let subsetting_allowed = face.is_subsetting_allowed();
        validate_embedding_permissions(
            asset_id,
            face_index,
            embedding,
            permissions,
            outline_embedding_allowed,
            subsetting_allowed,
        )?;
        faces.push(FontFaceMetadata {
            asset_id: asset_id.to_string(),
            face_index,
            family_name,
            postscript_name,
            style: match face.style() {
                Style::Normal => FontStyle::Normal,
                Style::Italic => FontStyle::Italic,
                Style::Oblique => FontStyle::Oblique,
            },
            weight: face.weight().to_number(),
            width: face.width().to_number(),
            permissions,
            subsetting_allowed,
            outline_embedding_allowed,
            table_count: face.raw_face().table_records.len() as usize,
        });
    }
    Ok(faces)
}

fn collect_family_names(
    faces: &[FontFaceMetadata],
) -> Result<BTreeMap<String, String>, FontCatalogError> {
    let mut families = BTreeMap::new();
    for face in faces {
        let key = normalize_family_key(&face.family_name);
        if let Some(existing) = families.get(&key) {
            if existing != &face.family_name {
                return Err(FontCatalogError::FamilyNameCollision);
            }
        } else {
            families.insert(key, face.family_name.clone());
        }
    }
    Ok(families)
}

fn compile_aliases(
    aliases: Vec<FontFamilyAliasSpec>,
    families: &BTreeMap<String, String>,
) -> Result<Vec<FontFamilyAlias>, FontCatalogError> {
    let mut compiled = BTreeMap::<String, FontFamilyAlias>::new();
    for (index, alias) in aliases.into_iter().enumerate() {
        validate_family_name(&alias.alias).map_err(|_| FontCatalogError::InvalidAlias { index })?;
        validate_family_name(&alias.target)
            .map_err(|_| FontCatalogError::InvalidAliasTarget { index })?;
        let alias_key = normalize_family_key(&alias.alias);
        if GenericFontFamily::from_css_keyword(&alias_key).is_some()
            || families.contains_key(&alias_key)
            || compiled.contains_key(&alias_key)
        {
            return Err(FontCatalogError::AliasCollision { alias: alias.alias });
        }
        let target_key = normalize_family_key(&alias.target);
        let target = families
            .get(&target_key)
            .ok_or(FontCatalogError::UnknownAliasTarget { index })?
            .clone();
        compiled.insert(
            alias_key,
            FontFamilyAlias {
                alias: alias.alias,
                target,
            },
        );
    }
    Ok(compiled.into_values().collect())
}

fn compile_generic_families(
    mappings: Vec<GenericFamilySpec>,
    families: &BTreeMap<String, String>,
    aliases: &[FontFamilyAlias],
) -> Result<BTreeMap<GenericFontFamily, String>, FontCatalogError> {
    let alias_targets = aliases
        .iter()
        .map(|alias| (normalize_family_key(&alias.alias), alias.target.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut compiled = BTreeMap::new();
    for mapping in mappings {
        if compiled.contains_key(&mapping.generic) {
            return Err(FontCatalogError::DuplicateGenericFamily {
                family: mapping.generic,
            });
        }
        validate_family_name(&mapping.target).map_err(|_| {
            FontCatalogError::InvalidGenericTarget {
                family: mapping.generic,
            }
        })?;
        let target_key = normalize_family_key(&mapping.target);
        let target = families
            .get(&target_key)
            .cloned()
            .or_else(|| alias_targets.get(&target_key).cloned())
            .ok_or(FontCatalogError::UnknownGenericTarget {
                family: mapping.generic,
            })?;
        compiled.insert(mapping.generic, target);
    }
    Ok(compiled)
}

fn validate_embedding_permissions(
    asset_id: &str,
    face_index: u32,
    requirement: FontEmbeddingRequirement,
    permissions: FontEmbeddingPermissions,
    outline_embedding_allowed: bool,
    subsetting_allowed: bool,
) -> Result<(), FontCatalogError> {
    let allowed = match requirement {
        FontEmbeddingRequirement::NoEmbedding => true,
        FontEmbeddingRequirement::FullFont => {
            permissions != FontEmbeddingPermissions::Restricted
                && permissions != FontEmbeddingPermissions::Unknown
                && outline_embedding_allowed
        }
        FontEmbeddingRequirement::Subset => {
            permissions != FontEmbeddingPermissions::Restricted
                && permissions != FontEmbeddingPermissions::Unknown
                && outline_embedding_allowed
                && subsetting_allowed
        }
    };
    if allowed {
        Ok(())
    } else {
        Err(FontCatalogError::EmbeddingDenied {
            asset_id: asset_id.to_string(),
            face_index,
            requirement,
            permissions,
        })
    }
}

struct FontNameCandidate<'a> {
    index: u16,
    platform_id: PlatformId,
    language_id: u16,
    raw: &'a [u8],
}

fn preferred_face_name(face: &Face<'_>, ids: &[u16]) -> Option<String> {
    for id in ids {
        let names = face.names();
        let mut candidates = Vec::<FontNameCandidate<'_>>::with_capacity(MAX_FONT_NAME_CANDIDATES);
        for index in 0..names.len() {
            let Some(name) = names.get(index) else {
                continue;
            };
            if name.name_id != *id
                || !name.is_unicode()
                || name.name.is_empty()
                || name.name.len() > MAX_FONT_FAMILY_NAME_BYTES * 2
                || name.name.len() % 2 != 0
            {
                continue;
            }
            let candidate = FontNameCandidate {
                index,
                platform_id: name.platform_id,
                language_id: name.language_id,
                raw: name.name,
            };
            let insert_at = candidates
                .binary_search_by(|existing| compare_name_candidates(existing, &candidate))
                .unwrap_or_else(|index| index);
            if insert_at < MAX_FONT_NAME_CANDIDATES {
                candidates.insert(insert_at, candidate);
                candidates.truncate(MAX_FONT_NAME_CANDIDATES);
            }
        }
        for candidate in candidates {
            let Some(candidate) = names.get(candidate.index) else {
                continue;
            };
            let Some(value) = candidate.to_string() else {
                continue;
            };
            let value = value.trim().to_string();
            if validate_family_name(&value).is_ok() {
                return Some(value);
            }
        }
    }
    None
}

fn compare_name_candidates(
    left: &FontNameCandidate<'_>,
    right: &FontNameCandidate<'_>,
) -> std::cmp::Ordering {
    name_candidate_rank(left)
        .cmp(&name_candidate_rank(right))
        .then_with(|| left.raw.cmp(right.raw))
}

fn name_candidate_rank(name: &FontNameCandidate<'_>) -> (u8, u8) {
    let platform = match name.platform_id {
        PlatformId::Windows => 0,
        PlatformId::Unicode => 1,
        _ => 2,
    };
    let language = if name.platform_id == PlatformId::Windows && name.language_id == 0x0409 {
        0
    } else {
        1
    };
    (platform, language)
}

fn map_permissions(permissions: Option<Permissions>) -> FontEmbeddingPermissions {
    match permissions {
        Some(Permissions::Installable) => FontEmbeddingPermissions::Installable,
        Some(Permissions::Editable) => FontEmbeddingPermissions::Editable,
        Some(Permissions::PreviewAndPrint) => FontEmbeddingPermissions::PreviewAndPrint,
        Some(Permissions::Restricted) => FontEmbeddingPermissions::Restricted,
        None => FontEmbeddingPermissions::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Woff2TableKind {
    Other,
    Glyf,
    Loca,
    Hmtx,
}

#[derive(Debug, Clone, Copy)]
struct Woff2TablePlan {
    tag: [u8; 4],
    kind: Woff2TableKind,
    offset: usize,
    length: usize,
    original_length: usize,
    transformed: bool,
}

#[derive(Debug, Clone)]
struct Woff2Preflight {
    asset_index: usize,
    declared_sfnt_size: usize,
    total_compressed_size: usize,
    compressed_offset: usize,
    uncompressed_size: usize,
    is_collection: bool,
    collection_version: Option<u32>,
    face_table_counts: Vec<usize>,
    face_count: usize,
    table_references: usize,
    tables: Vec<Woff2TablePlan>,
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn position(&self) -> usize {
        self.offset
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(length)?;
        let bytes = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(bytes)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(*self.take(1)?.first()?)
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes(self.take(2)?.try_into().ok()?))
    }

    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_be_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }

    fn variable_255_u16(&mut self) -> Option<u16> {
        match self.u8()? {
            253 => self.u16(),
            254 => Some(506 + u16::from(self.u8()?)),
            255 => Some(253 + u16::from(self.u8()?)),
            value => Some(u16::from(value)),
        }
    }

    fn variable_128_u32(&mut self) -> Option<u32> {
        let mut value = 0u32;
        for index in 0..5 {
            let byte = self.u8()?;
            if index == 0 && byte == 0x80 {
                return None;
            }
            if value & 0xfe00_0000 != 0 {
                return None;
            }
            value = (value << 7) | u32::from(byte & 0x7f);
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }
}

// WOFF2 table-directory flag indices, in the order defined by the format.
const WOFF2_KNOWN_TAGS: [[u8; 4]; 63] = [
    *b"cmap", *b"head", *b"hhea", *b"hmtx", *b"maxp", *b"name", *b"OS/2", *b"post", *b"cvt ",
    *b"fpgm", *b"glyf", *b"loca", *b"prep", *b"CFF ", *b"VORG", *b"EBDT", *b"EBLC", *b"gasp",
    *b"hdmx", *b"kern", *b"LTSH", *b"PCLT", *b"VDMX", *b"vhea", *b"vmtx", *b"BASE", *b"GDEF",
    *b"GPOS", *b"GSUB", *b"EBSC", *b"JSTF", *b"MATH", *b"CBDT", *b"CBLC", *b"COLR", *b"CPAL",
    *b"SVG ", *b"sbix", *b"acnt", *b"avar", *b"bdat", *b"bloc", *b"bsln", *b"cvar", *b"fdsc",
    *b"feat", *b"fmtx", *b"fvar", *b"gvar", *b"hsty", *b"just", *b"lcar", *b"mort", *b"morx",
    *b"opbd", *b"prop", *b"trak", *b"Zapf", *b"Silf", *b"Glat", *b"Gloc", *b"Feat", *b"Sill",
];

fn woff2_table_kind(tag: [u8; 4]) -> Woff2TableKind {
    match &tag {
        b"glyf" => Woff2TableKind::Glyf,
        b"loca" => Woff2TableKind::Loca,
        b"hmtx" => Woff2TableKind::Hmtx,
        _ => Woff2TableKind::Other,
    }
}

fn preflight_woff2(
    bytes: &[u8],
    resources: &ThemeResourcePolicy,
    asset_index: usize,
) -> Result<Woff2Preflight, FontCatalogError> {
    let malformed = || FontCatalogError::MalformedWoff2 { asset_index };
    let mut cursor = ByteCursor::new(bytes);
    if cursor.take(4) != Some(b"wOF2") {
        return Err(malformed());
    }
    let flavor = cursor.u32().ok_or_else(malformed)?;
    let length = cursor.u32().ok_or_else(malformed)? as usize;
    let table_count = usize::from(cursor.u16().ok_or_else(malformed)?);
    let _reserved = cursor.u16().ok_or_else(malformed)?;
    let declared_sfnt_size =
        usize::try_from(cursor.u32().ok_or_else(malformed)?).map_err(|_| malformed())?;
    let total_compressed_size =
        usize::try_from(cursor.u32().ok_or_else(malformed)?).map_err(|_| malformed())?;
    let _major = cursor.u16().ok_or_else(malformed)?;
    let _minor = cursor.u16().ok_or_else(malformed)?;
    let meta_offset =
        usize::try_from(cursor.u32().ok_or_else(malformed)?).map_err(|_| malformed())?;
    let meta_length =
        usize::try_from(cursor.u32().ok_or_else(malformed)?).map_err(|_| malformed())?;
    let _meta_original_length = cursor.u32().ok_or_else(malformed)?;
    let private_offset =
        usize::try_from(cursor.u32().ok_or_else(malformed)?).map_err(|_| malformed())?;
    let private_length =
        usize::try_from(cursor.u32().ok_or_else(malformed)?).map_err(|_| malformed())?;

    if length != bytes.len() || table_count == 0 || declared_sfnt_size == 0 {
        return Err(malformed());
    }
    resources.check_font_table_count(table_count)?;
    resources.check_font_asset_decoded_bytes(declared_sfnt_size)?;

    let mut tables = Vec::with_capacity(table_count);
    let mut compressed_table_size = 0usize;
    let mut font_tags = BTreeSet::new();
    for _ in 0..table_count {
        let flags = cursor.u8().ok_or_else(malformed)?;
        let tag_index = flags & 0x3f;
        let format = flags >> 6;
        let tag = if tag_index == 63 {
            cursor.u32().ok_or_else(malformed)?.to_be_bytes()
        } else {
            WOFF2_KNOWN_TAGS[usize::from(tag_index)]
        };
        // Collections may reuse tags across faces, but each face must have unique tags.
        if flavor != u32::from_be_bytes(*b"ttcf") && !font_tags.insert(tag) {
            return Err(malformed());
        }
        let kind = woff2_table_kind(tag);
        let original_length = usize::try_from(cursor.variable_128_u32().ok_or_else(malformed)?)
            .map_err(|_| malformed())?;
        resources.check_font_asset_decoded_bytes(original_length)?;
        let transformed = match kind {
            Woff2TableKind::Glyf | Woff2TableKind::Loca => format == 0,
            Woff2TableKind::Other | Woff2TableKind::Hmtx => format != 0,
        };
        let length = if transformed {
            usize::try_from(cursor.variable_128_u32().ok_or_else(malformed)?)
                .map_err(|_| malformed())?
        } else {
            original_length
        };
        let offset = compressed_table_size;
        compressed_table_size = compressed_table_size
            .checked_add(length)
            .ok_or_else(malformed)?;
        tables.push(Woff2TablePlan {
            tag,
            kind,
            offset,
            length,
            original_length,
            transformed,
        });
    }

    let (is_collection, collection_version, face_count, face_table_counts, table_references) =
        if flavor == u32::from_be_bytes(*b"ttcf") {
            let version = cursor.u32().ok_or_else(malformed)?;
            if version != 0x0001_0000 && version != 0x0002_0000 {
                return Err(malformed());
            }
            let face_count = usize::from(cursor.variable_255_u16().ok_or_else(malformed)?);
            if face_count == 0 {
                return Err(malformed());
            }
            resources.check_font_face_count(face_count)?;
            let mut table_references = 0usize;
            let mut face_table_counts = Vec::with_capacity(face_count);
            for _ in 0..face_count {
                let face_tables = usize::from(cursor.variable_255_u16().ok_or_else(malformed)?);
                let _face_flavor = cursor.u32().ok_or_else(malformed)?;
                if face_tables == 0 || face_tables > table_count {
                    return Err(malformed());
                }
                table_references = table_references
                    .checked_add(face_tables)
                    .ok_or_else(malformed)?;
                resources.check_font_table_count(table_references)?;
                let mut face_tags = BTreeSet::new();
                let mut glyf_index = None;
                let mut loca_index = None;
                for _ in 0..face_tables {
                    let table_index = usize::from(cursor.variable_255_u16().ok_or_else(malformed)?);
                    if table_index >= table_count {
                        return Err(malformed());
                    }
                    if !face_tags.insert(tables[table_index].tag) {
                        return Err(malformed());
                    }
                    match tables[table_index].kind {
                        Woff2TableKind::Glyf => glyf_index = Some(table_index),
                        Woff2TableKind::Loca => loca_index = Some(table_index),
                        _ => {}
                    }
                }
                match (glyf_index, loca_index) {
                    (Some(glyf), Some(loca)) if loca == glyf + 1 => {}
                    (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {
                        return Err(malformed());
                    }
                    (None, None) => {}
                }
                face_table_counts.push(face_tables);
            }
            (
                true,
                Some(version),
                face_count,
                face_table_counts,
                table_references,
            )
        } else {
            resources.check_font_face_count(1)?;
            (false, None, 1, vec![table_count], table_count)
        };

    let compressed_offset = cursor.position();
    let compressed_end = compressed_offset
        .checked_add(total_compressed_size)
        .ok_or_else(malformed)?;
    if compressed_end > bytes.len() {
        return Err(malformed());
    }
    if meta_offset != 0 {
        let meta_end = meta_offset.checked_add(meta_length).ok_or_else(malformed)?;
        if meta_offset >= bytes.len() || meta_end > bytes.len() {
            return Err(malformed());
        }
    }
    if private_offset != 0 {
        let private_end = private_offset
            .checked_add(private_length)
            .ok_or_else(malformed)?;
        if private_offset >= bytes.len() || private_end > bytes.len() {
            return Err(malformed());
        }
    }

    resources.check_font_decoded_expansion(total_compressed_size, declared_sfnt_size)?;
    resources.check_font_decoded_expansion(total_compressed_size, compressed_table_size)?;

    Ok(Woff2Preflight {
        asset_index,
        declared_sfnt_size,
        total_compressed_size,
        compressed_offset,
        uncompressed_size: compressed_table_size,
        is_collection,
        collection_version,
        face_table_counts,
        face_count,
        table_references,
        tables,
    })
}

fn validate_woff2_reconstruction(
    decompressed: &[u8],
    plan: &Woff2Preflight,
    resources: &ThemeResourcePolicy,
) -> Result<(), FontCatalogError> {
    if decompressed.len() != plan.uncompressed_size
        || plan.table_references < plan.tables.len()
        || plan
            .compressed_offset
            .checked_add(plan.total_compressed_size)
            .is_none()
    {
        return Err(FontCatalogError::MalformedWoff2 {
            asset_index: plan.asset_index,
        });
    }

    let mut output_bound = if plan.is_collection {
        let collection_header = 12usize
            .checked_add(plan.face_count.checked_mul(4).ok_or(
                FontCatalogError::MalformedWoff2 {
                    asset_index: plan.asset_index,
                },
            )?)
            .and_then(|value| {
                if plan.collection_version == Some(0x0002_0000) {
                    value.checked_add(12)
                } else {
                    Some(value)
                }
            })
            .ok_or(FontCatalogError::MalformedWoff2 {
                asset_index: plan.asset_index,
            })?;
        collection_header
            .checked_add(
                plan.face_table_counts
                    .iter()
                    .try_fold(0usize, |total, count| {
                        total
                            .checked_add(12)
                            .and_then(|value| value.checked_add(count.checked_mul(16)?))
                    })
                    .ok_or(FontCatalogError::MalformedWoff2 {
                        asset_index: plan.asset_index,
                    })?,
            )
            .ok_or(FontCatalogError::MalformedWoff2 {
                asset_index: plan.asset_index,
            })?
    } else {
        12usize
            .checked_add(plan.tables.len().checked_mul(16).ok_or(
                FontCatalogError::MalformedWoff2 {
                    asset_index: plan.asset_index,
                },
            )?)
            .ok_or(FontCatalogError::MalformedWoff2 {
                asset_index: plan.asset_index,
            })?
    };

    for table in &plan.tables {
        let end =
            table
                .offset
                .checked_add(table.length)
                .ok_or(FontCatalogError::MalformedWoff2 {
                    asset_index: plan.asset_index,
                })?;
        let table_bytes =
            decompressed
                .get(table.offset..end)
                .ok_or(FontCatalogError::MalformedWoff2 {
                    asset_index: plan.asset_index,
                })?;
        let table_bound = if table.transformed && table.kind == Woff2TableKind::Glyf {
            validate_transformed_glyf_table(table_bytes, resources, plan.asset_index)?.0
        } else if table.transformed
            && matches!(table.kind, Woff2TableKind::Hmtx | Woff2TableKind::Loca)
        {
            4 * 65_535
        } else {
            table.original_length
        };
        output_bound = output_bound
            .checked_add(table_bound)
            .and_then(|value| value.checked_add(3))
            .ok_or(FontCatalogError::MalformedWoff2 {
                asset_index: plan.asset_index,
            })?;
    }
    resources.check_font_asset_decoded_bytes(output_bound)?;
    Ok(())
}

fn validate_transformed_glyf_table(
    bytes: &[u8],
    resources: &ThemeResourcePolicy,
    asset_index: usize,
) -> Result<(usize, usize), FontCatalogError> {
    const GLYF_SUBSTREAM_COUNT: usize = 7;
    const WUFF_POINT_BYTES: usize = 16;
    let malformed = || FontCatalogError::MalformedWoff2 { asset_index };
    let mut header = ByteCursor::new(bytes);
    let _reserved = header.u16().ok_or_else(malformed)?;
    let flags = header.u16().ok_or_else(malformed)?;
    let num_glyphs = usize::from(header.u16().ok_or_else(malformed)?);
    let _index_format = header.u16().ok_or_else(malformed)?;
    let mut stream_lengths = [0usize; GLYF_SUBSTREAM_COUNT];
    for length in &mut stream_lengths {
        *length = usize::try_from(header.u32().ok_or_else(malformed)?).map_err(|_| malformed())?;
    }

    let mut stream_data = ByteCursor::new(&bytes[header.position()..]);
    let mut streams = Vec::with_capacity(GLYF_SUBSTREAM_COUNT);
    for length in stream_lengths {
        streams.push(stream_data.take(length).ok_or_else(malformed)?);
    }

    let n_contours_stream = streams[0];
    let n_points_stream = streams[1];
    let composite_stream = streams[4];
    let unsplit_bbox_stream = streams[5];
    let instruction_stream = streams[6];
    let bitmap_length = ((num_glyphs + 31) >> 5) * 4;
    if bitmap_length > unsplit_bbox_stream.len() {
        return Err(malformed());
    }
    let overlap_bitmap_length = if flags & 1 != 0 {
        (num_glyphs + 7) >> 3
    } else {
        0
    };
    if stream_data.take(overlap_bitmap_length).is_none() || n_contours_stream.len() < num_glyphs * 2
    {
        return Err(malformed());
    }

    let mut contours = ByteCursor::new(n_contours_stream);
    let mut points = ByteCursor::new(n_points_stream);
    let mut total_contours = 0usize;
    let mut total_points = 0usize;
    let mut max_points = 0usize;
    for _ in 0..num_glyphs {
        let n_contours = contours.i16().ok_or_else(malformed)?;
        if n_contours < -1 {
            return Err(malformed());
        }
        if n_contours <= 0 {
            continue;
        }
        let n_contours = usize::try_from(n_contours).map_err(|_| malformed())?;
        total_contours = total_contours
            .checked_add(n_contours)
            .ok_or_else(malformed)?;
        let mut glyph_points = 0usize;
        for _ in 0..n_contours {
            glyph_points = glyph_points
                .checked_add(usize::from(
                    points.variable_255_u16().ok_or_else(malformed)?,
                ))
                .ok_or_else(malformed)?;
        }
        total_points = total_points
            .checked_add(glyph_points)
            .ok_or_else(malformed)?;
        max_points = max_points.max(glyph_points);
    }

    let point_scratch = max_points
        .checked_mul(WUFF_POINT_BYTES)
        .ok_or_else(malformed)?;
    resources.check_font_asset_decoded_bytes(point_scratch)?;
    let output_bound = num_glyphs
        .checked_mul(15)
        .and_then(|value| value.checked_add(total_contours.checked_mul(2)?))
        .and_then(|value| value.checked_add(total_points.checked_mul(5)?))
        .and_then(|value| value.checked_add(composite_stream.len()))
        .and_then(|value| value.checked_add(instruction_stream.len()))
        .and_then(|value| value.checked_add(3usize.checked_mul(num_glyphs)?))
        .and_then(|value| value.checked_add((num_glyphs + 1).checked_mul(4)?))
        .ok_or_else(malformed)?;
    resources.check_font_asset_decoded_bytes(output_bound)?;
    Ok((output_bound, num_glyphs))
}

fn validate_asset_id(id: &str) -> Result<(), FontAssetIdError> {
    if id.is_empty() {
        return Err(FontAssetIdError::Empty);
    }
    if id.len() > MAX_FONT_ASSET_ID_BYTES {
        return Err(FontAssetIdError::TooLong);
    }
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(FontAssetIdError::InvalidCharacter);
    }
    Ok(())
}

fn validate_family_name(name: &str) -> Result<(), ()> {
    if name.trim().is_empty()
        || name.len() > MAX_FONT_FAMILY_NAME_BYTES
        || name.chars().any(|character| character.is_control())
    {
        return Err(());
    }
    Ok(())
}

fn hash_font_asset(container: FontContainer, canonical_bytes: &[u8]) -> FontAssetFingerprint {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, FONT_ASSET_FINGERPRINT_DOMAIN);
    update_len_prefixed(&mut hasher, container.id().as_bytes());
    update_len_prefixed(&mut hasher, canonical_bytes);
    FontAssetFingerprint(hasher.finalize().into())
}

fn hash_font_catalog(
    assets: &[FontAsset],
    faces: &[FontFaceMetadata],
    aliases: &[FontFamilyAlias],
    generic_families: &BTreeMap<GenericFontFamily, String>,
    available_sources: &BTreeSet<FontSource>,
    embedding: FontEmbeddingRequirement,
) -> FontCatalogFingerprint {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, FONT_CATALOG_FINGERPRINT_DOMAIN);
    update_len_prefixed(
        &mut hasher,
        match embedding {
            FontEmbeddingRequirement::NoEmbedding => b"no-embedding",
            FontEmbeddingRequirement::FullFont => b"full-font",
            FontEmbeddingRequirement::Subset => b"subset",
        },
    );
    for source in available_sources {
        update_len_prefixed(&mut hasher, source.id().as_bytes());
    }
    for asset in assets {
        update_len_prefixed(&mut hasher, asset.id.as_bytes());
        update_len_prefixed(&mut hasher, asset.fingerprint.as_bytes());
    }
    for face in faces {
        update_len_prefixed(&mut hasher, face.asset_id.as_bytes());
        hasher.update(face.face_index.to_be_bytes());
        update_len_prefixed(&mut hasher, face.family_name.as_bytes());
        update_len_prefixed(
            &mut hasher,
            face.postscript_name.as_deref().unwrap_or("").as_bytes(),
        );
        update_len_prefixed(&mut hasher, face.style.id().as_bytes());
        hasher.update(face.weight.to_be_bytes());
        hasher.update(face.width.to_be_bytes());
        update_len_prefixed(&mut hasher, face.permissions.id().as_bytes());
        hasher.update([face.subsetting_allowed as u8]);
        hasher.update([face.outline_embedding_allowed as u8]);
        hasher.update((face.table_count as u64).to_be_bytes());
    }
    for alias in aliases {
        update_len_prefixed(&mut hasher, alias.alias.as_bytes());
        update_len_prefixed(&mut hasher, alias.target.as_bytes());
    }
    for (generic, target) in generic_families {
        update_len_prefixed(&mut hasher, generic.id().as_bytes());
        update_len_prefixed(&mut hasher, target.as_bytes());
    }
    FontCatalogFingerprint(hasher.finalize().into())
}

struct AllocationBudget {
    current: AtomicUsize,
    denied: AtomicBool,
    maximum: usize,
}

impl AllocationBudget {
    fn new(maximum: usize) -> Self {
        Self {
            current: AtomicUsize::new(0),
            denied: AtomicBool::new(false),
            maximum,
        }
    }

    fn acquire(&self, bytes: usize) -> bool {
        let mut current = self.current.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_add(bytes) else {
                self.denied.store(true, Ordering::Relaxed);
                return false;
            };
            if next > self.maximum {
                self.denied.store(true, Ordering::Relaxed);
                return false;
            }
            match self.current.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(observed) => current = observed,
            }
        }
    }

    fn release(&self, bytes: usize) {
        self.current.fetch_sub(bytes, Ordering::AcqRel);
    }

    fn denied(&self) -> bool {
        self.denied.load(Ordering::Acquire)
    }
}

struct Rebox<T> {
    data: Box<[T]>,
    budget: Option<Arc<AllocationBudget>>,
    bytes: usize,
}

impl<T> Default for Rebox<T> {
    fn default() -> Self {
        Self {
            data: Vec::new().into_boxed_slice(),
            budget: None,
            bytes: 0,
        }
    }
}

impl<T> Drop for Rebox<T> {
    fn drop(&mut self) {
        if let Some(budget) = &self.budget {
            budget.release(self.bytes);
        }
    }
}

impl<T> SliceWrapper<T> for Rebox<T> {
    fn slice(&self) -> &[T] {
        &self.data
    }
}

impl<T> SliceWrapperMut<T> for Rebox<T> {
    fn slice_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}

struct BudgetedAlloc<T> {
    budget: Arc<AllocationBudget>,
    marker: PhantomData<T>,
}

impl<T> BudgetedAlloc<T> {
    fn new(budget: Arc<AllocationBudget>) -> Self {
        Self {
            budget,
            marker: PhantomData,
        }
    }
}

impl<T: Clone + Default> Allocator<T> for BudgetedAlloc<T> {
    type AllocatedMemory = Rebox<T>;

    fn alloc_cell(&mut self, len: usize) -> Self::AllocatedMemory {
        let Some(bytes) = len.checked_mul(size_of::<T>()) else {
            self.budget.denied.store(true, Ordering::Relaxed);
            return Rebox::default();
        };
        if bytes == 0 || !self.budget.acquire(bytes) {
            return Rebox::default();
        }

        let mut values = Vec::new();
        if values.try_reserve_exact(len).is_err() {
            self.budget.release(bytes);
            self.budget.denied.store(true, Ordering::Relaxed);
            return Rebox::default();
        }
        values.resize(len, T::default());
        Rebox {
            data: values.into_boxed_slice(),
            budget: Some(Arc::clone(&self.budget)),
            bytes,
        }
    }

    fn free_cell(&mut self, data: Self::AllocatedMemory) {
        drop(data);
    }
}

#[derive(Debug, thiserror::Error)]
#[error("bounded Brotli decode failed")]
struct BrotliDecodeError;

fn decompress_brotli_exact(
    compressed_data: &[u8],
    expected_size: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut output = vec![0u8; expected_size];
    let mut available_in = compressed_data.len();
    let mut input_offset = 0usize;
    let mut available_out = output.len();
    let mut output_offset = 0usize;
    let mut total_out = 0usize;
    let budget = Arc::new(AllocationBudget::new(MAX_BROTLI_WORKING_BYTES));
    let mut state = BrotliState::new_strict(
        BudgetedAlloc::<u8>::new(Arc::clone(&budget)),
        BudgetedAlloc::<u32>::new(Arc::clone(&budget)),
        BudgetedAlloc::new(Arc::clone(&budget)),
    );
    let result = BrotliDecompressStream(
        &mut available_in,
        &mut input_offset,
        compressed_data,
        &mut available_out,
        &mut output_offset,
        &mut output,
        &mut total_out,
        &mut state,
    );
    if budget.denied()
        || !matches!(result, BrotliResult::ResultSuccess)
        || output_offset != expected_size
    {
        return Err(Box::new(BrotliDecodeError));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::super::resources::{
        ThemeResourceLimitExceeded, ThemeResourceLimitId, ThemeResourcePolicy,
    };
    use super::super::FontStack;
    use super::*;

    const EXCALIFONT_WOFF2: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    const FONT_AWESOME_OTF: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/fonts/FontAwesome-4.6.3.otf"
    ));

    fn excalifont_spec() -> FontCatalogSpec {
        FontCatalogSpec::new([FontAssetSpec::new("excalifont", EXCALIFONT_WOFF2)])
            .with_available_sources([FontSource::Embedded])
            .with_embedding_requirement(FontEmbeddingRequirement::FullFont)
    }

    fn canonical_excalifont_ttf() -> Vec<u8> {
        FontCatalogSpec::compile(excalifont_spec(), &ThemeResourcePolicy::interactive())
            .unwrap()
            .assets()[0]
            .canonical_bytes()
            .to_vec()
    }

    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn sfnt_table_offset(bytes: &[u8], tag: &[u8; 4]) -> Option<usize> {
        let table_count = usize::from(read_u16(bytes, 4));
        for index in 0..table_count {
            let record_offset = 12 + index * 16;
            if bytes.get(record_offset..record_offset + 4)? == tag {
                return usize::try_from(read_u32(bytes, record_offset + 8)).ok();
            }
        }
        None
    }

    fn with_embedding_flags(mut bytes: Vec<u8>, flags: u16) -> Vec<u8> {
        let os2_offset = sfnt_table_offset(&bytes, b"OS/2").expect("fixture must contain OS/2");
        bytes[os2_offset + 8..os2_offset + 10].copy_from_slice(&flags.to_be_bytes());
        bytes
    }

    fn shift_sfnt_table_offsets(bytes: &mut [u8], base_offset: usize) {
        let table_count = usize::from(read_u16(bytes, 4));
        for index in 0..table_count {
            let offset_field = 12 + index * 16 + 8;
            let shifted = read_u32(bytes, offset_field)
                .checked_add(u32::try_from(base_offset).unwrap())
                .unwrap();
            write_u32(bytes, offset_field, shifted);
        }
    }

    fn ttc_from_sfnts(fonts: &[&[u8]]) -> Vec<u8> {
        assert!(!fonts.is_empty());
        let header_len = 12 + fonts.len() * 4;
        let mut collection = vec![0u8; header_len];
        collection[..4].copy_from_slice(b"ttcf");
        write_u32(&mut collection, 4, 0x0001_0000);
        write_u32(&mut collection, 8, u32::try_from(fonts.len()).unwrap());

        for (index, font) in fonts.iter().enumerate() {
            while collection.len() % 4 != 0 {
                collection.push(0);
            }
            let offset = collection.len();
            write_u32(
                &mut collection,
                12 + index * 4,
                u32::try_from(offset).unwrap(),
            );
            let mut shifted = font.to_vec();
            shift_sfnt_table_offsets(&mut shifted, offset);
            collection.extend_from_slice(&shifted);
        }
        collection
    }

    // This block is a valid Brotli stream for a 36-byte, zero-glyph glyf transform.
    const EMPTY_GLYF_BROTLI: &[u8] = &[27, 35, 0, 248, 39, 0, 162, 140, 0, 64];

    fn woff2_directory_fixture(directory: &[u8], table_count: u16, collection: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; 48];
        bytes[..4].copy_from_slice(b"wOF2");
        bytes[4..8].copy_from_slice(if collection.is_empty() {
            &[0, 1, 0, 0]
        } else {
            b"ttcf"
        });
        bytes[12..14].copy_from_slice(&table_count.to_be_bytes());
        write_u32(&mut bytes, 16, 128);
        write_u32(&mut bytes, 20, EMPTY_GLYF_BROTLI.len() as u32);
        bytes.extend_from_slice(directory);
        bytes.extend_from_slice(collection);
        bytes.extend_from_slice(EMPTY_GLYF_BROTLI);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        let length = bytes.len() as u32;
        write_u32(&mut bytes, 8, length);
        bytes
    }

    #[test]
    fn duplicate_loca_is_rejected_before_font_decompression() {
        let bytes = woff2_directory_fixture(&[10, 0, 36, 11, 2, 0, 11, 2, 0], 3, &[]);
        let error = canonicalize_font_asset(7, bytes.into(), &ThemeResourcePolicy::interactive())
            .err()
            .expect("duplicate loca must be rejected without panicking");
        assert!(matches!(
            error,
            FontCatalogError::MalformedWoff2 { asset_index: 7 }
        ));
    }

    #[test]
    fn woff2_preflight_rejects_duplicate_tags_in_both_encodings() {
        for tag_index in 0..63u8 {
            let flags = if matches!(tag_index, 10 | 11) {
                tag_index | 0xc0
            } else {
                tag_index
            };
            let bytes = woff2_directory_fixture(&[flags, 0, flags, 0], 2, &[]);
            assert!(
                matches!(
                    preflight_woff2(&bytes, &ThemeResourcePolicy::interactive(), 0),
                    Err(FontCatalogError::MalformedWoff2 { .. })
                ),
                "tag index {tag_index}"
            );
        }
        for directory in [
            vec![0, 0, 63, b'c', b'm', b'a', b'p', 0],
            vec![63, b'T', b'E', b'S', b'T', 0, 63, b'T', b'E', b'S', b'T', 0],
            vec![11, 2, 0, 63, b'l', b'o', b'c', b'a', 2, 0],
        ] {
            let bytes = woff2_directory_fixture(&directory, 2, &[]);
            assert!(matches!(
                preflight_woff2(&bytes, &ThemeResourcePolicy::interactive(), 0),
                Err(FontCatalogError::MalformedWoff2 { .. })
            ));
        }
    }

    #[test]
    fn woff2_collection_checks_tags_per_face_and_allows_shared_tables() {
        // Two distinct glyf/loca pairs and one cmap shared by both faces are valid.
        let directory = [10, 0, 36, 11, 2, 0, 10, 0, 36, 11, 2, 0, 0, 0];
        let collection = [
            0, 1, 0, 0, 2, 3, 0, 1, 0, 0, 0, 1, 4, 3, 0, 1, 0, 0, 2, 3, 4,
        ];
        let bytes = woff2_directory_fixture(&directory, 5, &collection);
        let plan = preflight_woff2(&bytes, &ThemeResourcePolicy::interactive(), 0)
            .expect("different faces can have the same tags or share a table");
        assert_eq!(plan.face_table_counts, [3, 3]);

        for indices in [[0, 1], [0, 0]] {
            let mut collection = vec![0, 1, 0, 0, 1, 2, 0, 1, 0, 0];
            collection.extend_from_slice(&indices);
            let bytes = woff2_directory_fixture(&[0, 0, 0, 0], 2, &collection);
            assert!(matches!(
                preflight_woff2(&bytes, &ThemeResourcePolicy::interactive(), 0),
                Err(FontCatalogError::MalformedWoff2 { .. })
            ));
        }
    }

    #[test]
    fn woff2_is_canonicalized_once_into_an_immutable_sfnt_catalog() {
        let catalog =
            FontCatalogSpec::compile(excalifont_spec(), &ThemeResourcePolicy::interactive())
                .unwrap();

        assert!(catalog.requires_prepared_text_layout());
        assert_eq!(catalog.assets().len(), 1);
        assert_eq!(catalog.faces().len(), 1);
        assert_eq!(catalog.assets()[0].input_container(), FontContainer::Woff2);
        assert_eq!(
            catalog.assets()[0].canonical_container(),
            FontContainer::TrueType
        );
        assert!(
            catalog.assets()[0]
                .canonical_bytes()
                .starts_with(&[0, 1, 0, 0])
        );
        assert_eq!(catalog.faces()[0].family_name(), "Excalifont");
        let cloned = catalog.clone();
        assert!(std::ptr::eq(
            catalog.assets()[0].canonical_bytes().as_ptr(),
            cloned.assets()[0].canonical_bytes().as_ptr()
        ));
    }

    #[test]
    fn raw_truetype_and_cff_opentype_are_admitted_without_host_font_lookup() {
        let ttf = canonical_excalifont_ttf();
        let spec = FontCatalogSpec::new([
            FontAssetSpec::new("excalifont.ttf", &ttf),
            FontAssetSpec::new("fontawesome.otf", FONT_AWESOME_OTF),
        ]);

        let catalog = FontCatalogSpec::compile(spec, &ThemeResourcePolicy::interactive()).unwrap();
        let ttf_asset = catalog
            .assets()
            .iter()
            .find(|asset| asset.id() == "excalifont.ttf")
            .unwrap();
        let otf_asset = catalog
            .assets()
            .iter()
            .find(|asset| asset.id() == "fontawesome.otf")
            .unwrap();

        assert_eq!(ttf_asset.input_container(), FontContainer::TrueType);
        assert_eq!(ttf_asset.canonical_container(), FontContainer::TrueType);
        assert_eq!(otf_asset.input_container(), FontContainer::OpenType);
        assert_eq!(otf_asset.canonical_container(), FontContainer::OpenType);
        assert!(
            catalog
                .faces()
                .iter()
                .any(|face| face.family_name() == "Excalifont")
        );
        assert!(
            catalog
                .faces()
                .iter()
                .any(|face| face.family_name() == "FontAwesome")
        );
    }

    #[test]
    fn raw_sfnt_reuses_caller_owned_shared_bytes() {
        let bytes = Arc::<[u8]>::from(canonical_excalifont_ttf());
        let input = Arc::clone(&bytes);
        let spec = FontCatalogSpec::new([FontAssetSpec::from_shared_bytes("font.ttf", input)]);

        let catalog = FontCatalogSpec::compile(spec, &ThemeResourcePolicy::interactive()).unwrap();

        assert!(std::ptr::eq(
            bytes.as_ptr(),
            catalog.assets()[0].canonical_bytes().as_ptr()
        ));
    }

    #[test]
    fn truetype_collections_preserve_each_valid_face_index() {
        let ttf = canonical_excalifont_ttf();
        let ttc = ttc_from_sfnts(&[&ttf, &ttf]);
        let spec = FontCatalogSpec::new([FontAssetSpec::new("excalifont.ttc", &ttc)]);

        let catalog = FontCatalogSpec::compile(spec, &ThemeResourcePolicy::interactive()).unwrap();

        assert_eq!(
            catalog.assets()[0].input_container(),
            FontContainer::Collection
        );
        assert_eq!(
            catalog.assets()[0].canonical_container(),
            FontContainer::Collection
        );
        assert_eq!(catalog.faces().len(), 2);
        assert_eq!(catalog.faces()[0].face_index(), 0);
        assert_eq!(catalog.faces()[1].face_index(), 1);
    }

    #[test]
    fn malformed_font_containers_and_collection_offsets_fail_closed() {
        let unsupported = FontCatalogSpec::new([FontAssetSpec::new("garbage", b"not a font")]);
        assert!(matches!(
            FontCatalogSpec::compile(unsupported, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::UnsupportedFontContainer { .. })
        ));

        let mut empty_collection = vec![0u8; 12];
        empty_collection[..4].copy_from_slice(b"ttcf");
        write_u32(&mut empty_collection, 4, 0x0001_0000);
        let empty = FontCatalogSpec::new([FontAssetSpec::new("empty.ttc", empty_collection)]);
        assert!(matches!(
            FontCatalogSpec::compile(empty, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::MalformedCollection { .. })
        ));

        let mut oversized_collection = vec![0u8; 12];
        oversized_collection[..4].copy_from_slice(b"ttcf");
        write_u32(&mut oversized_collection, 4, 0x0001_0000);
        write_u32(&mut oversized_collection, 8, u32::MAX);
        let oversized =
            FontCatalogSpec::new([FontAssetSpec::new("oversized.ttc", oversized_collection)]);
        assert!(matches!(
            FontCatalogSpec::compile(oversized, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::ResourceLimit(
                ThemeResourceLimitExceeded {
                    limit: "max_font_faces" | "font_faces_hard_cap",
                    ..
                }
            ))
        ));

        let ttf = canonical_excalifont_ttf();
        let mut invalid_offset = ttc_from_sfnts(&[&ttf, &ttf]);
        write_u32(&mut invalid_offset, 16, u32::MAX);
        let invalid = FontCatalogSpec::new([FontAssetSpec::new("invalid.ttc", invalid_offset)]);
        assert!(matches!(
            FontCatalogSpec::compile(invalid, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::MalformedFace { face_index: 1, .. })
        ));
    }

    #[test]
    fn restricted_embedding_permissions_are_enforced_at_catalog_compile_time() {
        let restricted = with_embedding_flags(canonical_excalifont_ttf(), 0x0002);
        let inspect_only = FontCatalogSpec::new([FontAssetSpec::new("restricted", &restricted)]);
        let catalog =
            FontCatalogSpec::compile(inspect_only, &ThemeResourcePolicy::interactive()).unwrap();
        assert_eq!(
            catalog.faces()[0].permissions(),
            FontEmbeddingPermissions::Restricted
        );

        let embedded = FontCatalogSpec::new([FontAssetSpec::new("restricted", &restricted)])
            .with_embedding_requirement(FontEmbeddingRequirement::FullFont);
        assert!(matches!(
            FontCatalogSpec::compile(embedded, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::EmbeddingDenied {
                requirement: FontEmbeddingRequirement::FullFont,
                permissions: FontEmbeddingPermissions::Restricted,
                ..
            })
        ));
    }

    #[test]
    fn staged_resource_checks_reject_font_payloads_before_catalog_retention() {
        let compressed = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxFontAssetCompressedBytes, 1)
            .unwrap();
        let error = FontCatalogSpec::compile(excalifont_spec(), &compressed).unwrap_err();
        assert!(matches!(
            error,
            FontCatalogError::ResourceLimit(ThemeResourceLimitExceeded {
                limit: "max_font_asset_compressed_bytes",
                ..
            })
        ));

        let decoded = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxFontAssetDecodedBytes, 1)
            .unwrap();
        let error = FontCatalogSpec::compile(excalifont_spec(), &decoded).unwrap_err();
        assert!(matches!(
            error,
            FontCatalogError::ResourceLimit(ThemeResourceLimitExceeded {
                limit: "max_font_asset_decoded_bytes",
                ..
            })
        ));

        let ratio = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxFontDecodedExpansionRatio, 1)
            .unwrap();
        let error = FontCatalogSpec::compile(excalifont_spec(), &ratio).unwrap_err();
        assert!(matches!(
            error,
            FontCatalogError::ResourceLimit(ThemeResourceLimitExceeded {
                limit: "max_font_decoded_expansion_ratio",
                ..
            })
        ));

        let disabled = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxFontAssets, 0)
            .unwrap();
        let error = FontCatalogSpec::compile(excalifont_spec(), &disabled).unwrap_err();
        assert!(matches!(
            error,
            FontCatalogError::ResourceLimit(ThemeResourceLimitExceeded {
                limit: "max_font_assets",
                ..
            })
        ));
    }

    #[test]
    fn catalog_fingerprint_is_independent_of_asset_and_mapping_insertion_order() {
        let xiaolai = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        let left = FontCatalogSpec::new([
            FontAssetSpec::new("excalifont", EXCALIFONT_WOFF2),
            FontAssetSpec::new("xiaolai", xiaolai),
        ])
        .with_alias("Sketch", "Excalifont")
        .with_generic_family(GenericFontFamily::Cursive, "Excalifont");
        let right = FontCatalogSpec::new([
            FontAssetSpec::new("xiaolai", xiaolai),
            FontAssetSpec::new("excalifont", EXCALIFONT_WOFF2),
        ])
        .with_generic_family(GenericFontFamily::Cursive, "Excalifont")
        .with_alias("Sketch", "Excalifont");

        let policy = ThemeResourcePolicy::interactive();
        let left = FontCatalogSpec::compile(left, &policy).unwrap();
        let right = FontCatalogSpec::compile(right, &policy).unwrap();

        assert_eq!(left.fingerprint(), right.fingerprint());
        assert_eq!(
            left.fingerprint().to_hex(),
            "808f4aea7bdc111db629bbc31f4cd26a986697d9e16acb554b08e94da30cde80"
        );
    }

    #[test]
    fn identical_canonical_font_bytes_share_content_identity_and_storage() {
        let spec = FontCatalogSpec::new([
            FontAssetSpec::new("first", EXCALIFONT_WOFF2),
            FontAssetSpec::new("second", EXCALIFONT_WOFF2),
        ]);
        let catalog = FontCatalogSpec::compile(spec, &ThemeResourcePolicy::interactive()).unwrap();

        assert_eq!(
            catalog.assets()[0].fingerprint(),
            catalog.assets()[1].fingerprint()
        );
        assert!(std::ptr::eq(
            catalog.assets()[0].canonical_bytes().as_ptr(),
            catalog.assets()[1].canonical_bytes().as_ptr()
        ));
    }

    #[test]
    fn aliases_and_generic_families_resolve_only_to_catalog_owned_names() {
        let spec = excalifont_spec()
            .with_alias("Sketch", "Excalifont")
            .with_generic_family(GenericFontFamily::Cursive, "Sketch");
        let catalog = FontCatalogSpec::compile(spec, &ThemeResourcePolicy::interactive()).unwrap();

        assert_eq!(catalog.aliases()[0].alias(), "Sketch");
        assert_eq!(catalog.aliases()[0].target(), "Excalifont");
        assert_eq!(
            catalog.generic_family(GenericFontFamily::Cursive),
            Some("Excalifont")
        );
        let requested = FontStack::new(["Arial", "Sketch", "cursive", "Excalifont"])
            .expect("fixture stack should be valid");
        let admitted = catalog
            .admit_font_stack(&requested)
            .expect("at least one requested family belongs to the catalog");
        assert_eq!(admitted.families(), &["Excalifont".to_string()]);
        assert!(
            catalog
                .admit_font_stack(&FontStack::single("Arial").unwrap())
                .is_none()
        );

        let unknown_alias = excalifont_spec().with_alias("Sketch", "Missing");
        assert!(matches!(
            FontCatalogSpec::compile(unknown_alias, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::UnknownAliasTarget { .. })
        ));

        let duplicate_generic = excalifont_spec()
            .with_generic_family(GenericFontFamily::Cursive, "Excalifont")
            .with_generic_family(GenericFontFamily::Cursive, "Excalifont");
        assert!(matches!(
            FontCatalogSpec::compile(duplicate_generic, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::DuplicateGenericFamily {
                family: GenericFontFamily::Cursive
            })
        ));

        let generic_keyword_alias = excalifont_spec().with_alias("cursive", "Excalifont");
        assert!(matches!(
            FontCatalogSpec::compile(
                generic_keyword_alias,
                &ThemeResourcePolicy::interactive()
            ),
            Err(FontCatalogError::AliasCollision { alias }) if alias == "cursive"
        ));
    }

    #[test]
    fn duplicate_asset_ids_and_alias_collisions_fail_closed() {
        let duplicate = FontCatalogSpec::new([
            FontAssetSpec::new("same", EXCALIFONT_WOFF2),
            FontAssetSpec::new("same", EXCALIFONT_WOFF2),
        ]);
        assert!(matches!(
            FontCatalogSpec::compile(duplicate, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::DuplicateAssetId { .. })
        ));

        let alias_collision = excalifont_spec()
            .with_alias("Excalifont", "Excalifont")
            .with_alias("excalifont", "Excalifont");
        assert!(matches!(
            FontCatalogSpec::compile(alias_collision, &ThemeResourcePolicy::interactive()),
            Err(FontCatalogError::AliasCollision { .. })
        ));
    }
}
