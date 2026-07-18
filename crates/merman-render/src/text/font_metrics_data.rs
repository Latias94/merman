//! Compact, versioned storage for generated font-metric profiles.

use super::{
    FontMetricsTable, FontMetricsVariant, SvgVerticalDomShape, SvgVerticalProfileSet,
    SvgVerticalSizeProfile,
};
#[cfg(any(test, feature = "font-metrics-generation"))]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

const MAGIC: &[u8; 8] = b"MRMFNT05";
const ASCII_PAIR_MIN: u32 = 0x21;
const ASCII_PAIR_MAX: u32 = 0x7e;
const MAX_PALETTE_LEN: usize = u8::MAX as usize + 1;
const SVG_VERTICAL_MIN_FONT_SIZE_PX: u8 = 1;
const SVG_VERTICAL_MAX_FONT_SIZE_PX: u8 = 64;
const MAX_SVG_VERTICAL_BUCKETS: usize = u8::MAX as usize + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontMetricsVariantData {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FontMetricsVariantData {
    #[cfg(any(test, feature = "font-metrics-generation"))]
    fn to_byte(self) -> u8 {
        match self {
            Self::Regular => 0,
            Self::Bold => 1,
            Self::Italic => 2,
            Self::BoldItalic => 3,
        }
    }

    fn from_byte(value: u8, offset: usize) -> Result<Self, FontMetricsCodecError> {
        match value {
            0 => Ok(Self::Regular),
            1 => Ok(Self::Bold),
            2 => Ok(Self::Italic),
            3 => Ok(Self::BoldItalic),
            _ => Err(FontMetricsCodecError::new(offset, "invalid font variant")),
        }
    }

    fn to_runtime(self) -> FontMetricsVariant {
        match self {
            Self::Regular => FontMetricsVariant::Regular,
            Self::Bold => FontMetricsVariant::Bold,
            Self::Italic => FontMetricsVariant::Italic,
            Self::BoldItalic => FontMetricsVariant::BoldItalic,
        }
    }

    #[cfg(feature = "font-metrics-generation")]
    pub fn rust_name(self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::Bold => "Bold",
            Self::Italic => "Italic",
            Self::BoldItalic => "BoldItalic",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FontMetricsTableData {
    pub font_key: String,
    pub variant: FontMetricsVariantData,
    pub default_em: f64,
    pub entries: Vec<(char, f64)>,
    pub kern_pairs: Vec<(u32, u32, f64)>,
    pub space_trigrams: Vec<(u32, u32, f64)>,
    pub trigrams: Vec<(u32, u32, u32, f64)>,
    pub svg_scale: f64,
    pub svg_bbox_overhang_left_default_em: f64,
    pub svg_bbox_overhang_right_default_em: f64,
    pub svg_bbox_overhang_left: Vec<(char, f64)>,
    pub svg_bbox_overhang_right: Vec<(char, f64)>,
    pub svg_vertical_glyphs: Vec<char>,
    pub svg_vertical_profiles: [SvgVerticalProfileSetData; SvgVerticalDomShapeData::COUNT],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SvgVerticalSizeProfileData {
    pub font_size_px: u8,
    pub bbox_y_height_buckets: Vec<(f64, f64)>,
    pub glyph_bucket_indices: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SvgVerticalDomShapeData {
    RawText,
    SingleTspan,
    CreateFormattedText,
    CreateFormattedTextMiddle,
}

impl SvgVerticalDomShapeData {
    pub const ALL: [Self; 4] = [
        Self::RawText,
        Self::SingleTspan,
        Self::CreateFormattedText,
        Self::CreateFormattedTextMiddle,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub const fn index(self) -> usize {
        match self {
            Self::RawText => 0,
            Self::SingleTspan => 1,
            Self::CreateFormattedText => 2,
            Self::CreateFormattedTextMiddle => 3,
        }
    }

    #[cfg(any(test, feature = "font-metrics-generation"))]
    fn to_byte(self) -> u8 {
        self.index() as u8
    }

    fn from_byte(value: u8, offset: usize) -> Result<Self, FontMetricsCodecError> {
        Self::ALL
            .get(usize::from(value))
            .copied()
            .ok_or_else(|| FontMetricsCodecError::new(offset, "invalid SVG vertical DOM shape"))
    }

    fn to_runtime(self) -> SvgVerticalDomShape {
        match self {
            Self::RawText => SvgVerticalDomShape::RawText,
            Self::SingleTspan => SvgVerticalDomShape::SingleTspan,
            Self::CreateFormattedText => SvgVerticalDomShape::CreateFormattedText,
            Self::CreateFormattedTextMiddle => SvgVerticalDomShape::CreateFormattedTextMiddle,
        }
    }

    #[cfg(feature = "font-metrics-generation")]
    pub fn audit_name(self) -> &'static str {
        match self {
            Self::RawText => "raw-text",
            Self::SingleTspan => "single-tspan",
            Self::CreateFormattedText => "create-formatted-text",
            Self::CreateFormattedTextMiddle => "create-formatted-text-middle",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SvgVerticalProfileSetData {
    Approximate {
        bbox_y_em: f64,
        bbox_height_em: f64,
        pair_union_max_delta_px: f64,
    },
    Profiled {
        approximate_bbox_y_em: f64,
        approximate_bbox_height_em: f64,
        pair_union_max_delta_px: f64,
        pair_union_exact: bool,
        profiles: Vec<SvgVerticalSizeProfileData>,
    },
    Alias(SvgVerticalDomShapeData),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontMetricsCodecError {
    offset: usize,
    message: &'static str,
}

impl FontMetricsCodecError {
    fn new(offset: usize, message: &'static str) -> Self {
        Self { offset, message }
    }
}

impl fmt::Display for FontMetricsCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "font metrics profile error at byte {}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for FontMetricsCodecError {}

fn validate_sorted_by<T, K: Ord>(
    entries: &[T],
    mut key: impl FnMut(&T) -> K,
    message: &'static str,
) -> Result<(), FontMetricsCodecError> {
    if entries
        .windows(2)
        .any(|pair| key(&pair[0]) >= key(&pair[1]))
    {
        return Err(FontMetricsCodecError::new(0, message));
    }
    Ok(())
}

fn validate_svg_vertical_profile_set(
    shape: SvgVerticalDomShapeData,
    profile_set: &SvgVerticalProfileSetData,
    glyph_count: usize,
) -> Result<(), FontMetricsCodecError> {
    let (bbox_y_em, bbox_height_em, pair_union_max_delta_px, profiles) = match profile_set {
        SvgVerticalProfileSetData::Approximate {
            bbox_y_em,
            bbox_height_em,
            pair_union_max_delta_px,
        } => (*bbox_y_em, *bbox_height_em, *pair_union_max_delta_px, None),
        SvgVerticalProfileSetData::Profiled {
            approximate_bbox_y_em,
            approximate_bbox_height_em,
            pair_union_max_delta_px,
            pair_union_exact: _,
            profiles,
        } => (
            *approximate_bbox_y_em,
            *approximate_bbox_height_em,
            *pair_union_max_delta_px,
            Some(profiles.as_slice()),
        ),
        SvgVerticalProfileSetData::Alias(target) => {
            if target.index() >= shape.index() {
                return Err(FontMetricsCodecError::new(
                    0,
                    "SVG vertical alias must target an earlier DOM shape",
                ));
            }
            return Ok(());
        }
    };
    if !bbox_y_em.is_finite()
        || !bbox_height_em.is_finite()
        || bbox_height_em < 0.0
        || !pair_union_max_delta_px.is_finite()
        || pair_union_max_delta_px < 0.0
    {
        return Err(FontMetricsCodecError::new(
            0,
            "SVG vertical approximation and pair proof must be finite with non-negative height and delta",
        ));
    }
    let Some(profiles) = profiles else {
        return Ok(());
    };
    if profiles.is_empty() || profiles.len() > usize::from(SVG_VERTICAL_MAX_FONT_SIZE_PX) {
        return Err(FontMetricsCodecError::new(
            0,
            "exact SVG vertical profile count is out of range",
        ));
    }
    if glyph_count == 0 {
        return Err(FontMetricsCodecError::new(
            0,
            "exact SVG vertical profiles require a non-empty glyph table",
        ));
    }
    for profile in profiles {
        if !(SVG_VERTICAL_MIN_FONT_SIZE_PX..=SVG_VERTICAL_MAX_FONT_SIZE_PX)
            .contains(&profile.font_size_px)
        {
            return Err(FontMetricsCodecError::new(
                0,
                "SVG vertical profile font size is out of range",
            ));
        }
        if profile.bbox_y_height_buckets.is_empty()
            || profile.bbox_y_height_buckets.len() > MAX_SVG_VERTICAL_BUCKETS
        {
            return Err(FontMetricsCodecError::new(
                0,
                "SVG vertical bucket count is out of range",
            ));
        }
        if profile.glyph_bucket_indices.len() != glyph_count {
            return Err(FontMetricsCodecError::new(
                0,
                "SVG vertical glyph mapping has the wrong length",
            ));
        }
        if profile
            .bbox_y_height_buckets
            .iter()
            .any(|(bbox_y, bbox_height)| {
                !bbox_y.is_finite() || !bbox_height.is_finite() || *bbox_height < 0.0
            })
        {
            return Err(FontMetricsCodecError::new(
                0,
                "SVG vertical bbox bucket must be finite with non-negative height",
            ));
        }
        if profile
            .glyph_bucket_indices
            .iter()
            .any(|index| usize::from(*index) >= profile.bbox_y_height_buckets.len())
        {
            return Err(FontMetricsCodecError::new(
                0,
                "SVG vertical bucket index is out of bounds",
            ));
        }
    }
    validate_sorted_by(
        profiles,
        |profile| profile.font_size_px,
        "SVG vertical profiles are not sorted",
    )
}

fn validate_table(table: &FontMetricsTableData) -> Result<(), FontMetricsCodecError> {
    if table.font_key.is_empty() || !table.font_key.is_ascii() {
        return Err(FontMetricsCodecError::new(
            0,
            "font key must be non-empty ASCII",
        ));
    }
    let values = std::iter::once(table.default_em)
        .chain(table.entries.iter().map(|entry| entry.1))
        .chain(table.kern_pairs.iter().map(|entry| entry.2))
        .chain(table.space_trigrams.iter().map(|entry| entry.2))
        .chain(table.trigrams.iter().map(|entry| entry.3))
        .chain(std::iter::once(table.svg_scale))
        .chain(std::iter::once(table.svg_bbox_overhang_left_default_em))
        .chain(std::iter::once(table.svg_bbox_overhang_right_default_em))
        .chain(table.svg_bbox_overhang_left.iter().map(|entry| entry.1))
        .chain(table.svg_bbox_overhang_right.iter().map(|entry| entry.1));
    if values.into_iter().any(|value| !value.is_finite()) {
        return Err(FontMetricsCodecError::new(
            0,
            "metric values must be finite",
        ));
    }
    validate_sorted_by(
        &table.svg_vertical_glyphs,
        |character| *character as u32,
        "SVG vertical glyphs are not sorted and unique",
    )?;
    for shape in SvgVerticalDomShapeData::ALL {
        validate_svg_vertical_profile_set(
            shape,
            &table.svg_vertical_profiles[shape.index()],
            table.svg_vertical_glyphs.len(),
        )?;
    }
    let valid_pair_key = |value: u32| (ASCII_PAIR_MIN..=ASCII_PAIR_MAX).contains(&value);
    if table
        .kern_pairs
        .iter()
        .chain(&table.space_trigrams)
        .any(|(left, right, _)| !valid_pair_key(*left) || !valid_pair_key(*right))
        || table
            .trigrams
            .iter()
            .any(|(a, b, c, _)| !valid_pair_key(*a) || !valid_pair_key(*b) || !valid_pair_key(*c))
    {
        return Err(FontMetricsCodecError::new(
            0,
            "pair and trigram keys must be printable ASCII",
        ));
    }
    validate_sorted_by(
        &table.entries,
        |entry| entry.0 as u32,
        "entries are not sorted",
    )?;
    validate_sorted_by(
        &table.kern_pairs,
        |entry| (entry.0, entry.1),
        "kern pairs are not sorted",
    )?;
    validate_sorted_by(
        &table.space_trigrams,
        |entry| (entry.0, entry.1),
        "space trigrams are not sorted",
    )?;
    validate_sorted_by(
        &table.trigrams,
        |entry| (entry.0, entry.1, entry.2),
        "trigrams are not sorted",
    )?;
    validate_sorted_by(
        &table.svg_bbox_overhang_left,
        |entry| entry.0 as u32,
        "left overhangs are not sorted",
    )?;
    validate_sorted_by(
        &table.svg_bbox_overhang_right,
        |entry| entry.0 as u32,
        "right overhangs are not sorted",
    )?;
    Ok(())
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn checked_u16(value: usize, message: &'static str) -> Result<u16, FontMetricsCodecError> {
    u16::try_from(value).map_err(|_| FontMetricsCodecError::new(0, message))
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn checked_u32(value: usize, message: &'static str) -> Result<u32, FontMetricsCodecError> {
    u32::try_from(value).map_err(|_| FontMetricsCodecError::new(0, message))
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn write_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn vertical_bucket_index_bits(bucket_count: usize) -> usize {
    if bucket_count <= 1 {
        0
    } else {
        usize::BITS as usize - (bucket_count - 1).leading_zeros() as usize
    }
}

fn packed_vertical_bucket_indices_len(
    glyph_count: usize,
    bucket_count: usize,
) -> Result<usize, FontMetricsCodecError> {
    glyph_count
        .checked_mul(vertical_bucket_index_bits(bucket_count))
        .and_then(|bits| bits.checked_add(7))
        .map(|bits| bits / 8)
        .ok_or_else(|| FontMetricsCodecError::new(0, "vertical mapping bit length overflow"))
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn pack_vertical_bucket_indices(
    indices: &[u8],
    bucket_count: usize,
) -> Result<Vec<u8>, FontMetricsCodecError> {
    let bit_width = vertical_bucket_index_bits(bucket_count);
    let mut packed = vec![0; packed_vertical_bucket_indices_len(indices.len(), bucket_count)?];
    for (glyph_index, bucket_index) in indices.iter().copied().enumerate() {
        if usize::from(bucket_index) >= bucket_count {
            return Err(FontMetricsCodecError::new(
                0,
                "vertical bucket index is out of bounds",
            ));
        }
        for bit in 0..bit_width {
            if bucket_index & (1 << bit) != 0 {
                let bit_offset = glyph_index * bit_width + bit;
                packed[bit_offset / 8] |= 1 << (bit_offset % 8);
            }
        }
    }
    Ok(packed)
}

fn unpack_vertical_bucket_indices(
    packed: &[u8],
    glyph_count: usize,
    bucket_count: usize,
) -> Result<Vec<u8>, FontMetricsCodecError> {
    if bucket_count == 0 || bucket_count > MAX_SVG_VERTICAL_BUCKETS {
        return Err(FontMetricsCodecError::new(
            0,
            "vertical bucket count is out of range",
        ));
    }
    let bit_width = vertical_bucket_index_bits(bucket_count);
    let expected_len = packed_vertical_bucket_indices_len(glyph_count, bucket_count)?;
    if packed.len() != expected_len {
        return Err(FontMetricsCodecError::new(
            0,
            "vertical mapping has the wrong byte length",
        ));
    }
    let used_bits = glyph_count * bit_width;
    if let Some(last) = packed.last()
        && !used_bits.is_multiple_of(8)
        && *last & !((1 << (used_bits % 8)) - 1) != 0
    {
        return Err(FontMetricsCodecError::new(
            0,
            "vertical mapping padding bits are non-zero",
        ));
    }

    let mut indices = Vec::with_capacity(glyph_count);
    for glyph_index in 0..glyph_count {
        let mut bucket_index = 0_u8;
        for bit in 0..bit_width {
            let bit_offset = glyph_index * bit_width + bit;
            let value = (packed[bit_offset / 8] >> (bit_offset % 8)) & 1;
            bucket_index |= value << bit;
        }
        if usize::from(bucket_index) >= bucket_count {
            return Err(FontMetricsCodecError::new(
                0,
                "vertical bucket index is out of bounds",
            ));
        }
        indices.push(bucket_index);
    }
    Ok(indices)
}

#[cfg(any(test, feature = "font-metrics-generation"))]
pub fn encode_font_metrics_profile(
    tables: &[FontMetricsTableData],
) -> Result<Vec<u8>, FontMetricsCodecError> {
    let mut identities = BTreeSet::new();
    for table in tables {
        validate_table(table)?;
        if !identities.insert((table.font_key.as_str(), table.variant)) {
            return Err(FontMetricsCodecError::new(0, "duplicate font table"));
        }
    }

    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    write_u16(
        &mut output,
        checked_u16(tables.len(), "too many font tables")?,
    );
    for table in tables {
        write_u16(
            &mut output,
            checked_u16(table.font_key.len(), "font key is too long")?,
        );
        output.extend_from_slice(table.font_key.as_bytes());
        output.push(table.variant.to_byte());

        let mut palette_bits = BTreeSet::new();
        for value in std::iter::once(table.default_em)
            .chain(table.entries.iter().map(|entry| entry.1))
            .chain(table.kern_pairs.iter().map(|entry| entry.2))
            .chain(table.space_trigrams.iter().map(|entry| entry.2))
            .chain(table.trigrams.iter().map(|entry| entry.3))
            .chain(std::iter::once(table.svg_scale))
            .chain(std::iter::once(table.svg_bbox_overhang_left_default_em))
            .chain(std::iter::once(table.svg_bbox_overhang_right_default_em))
            .chain(table.svg_bbox_overhang_left.iter().map(|entry| entry.1))
            .chain(table.svg_bbox_overhang_right.iter().map(|entry| entry.1))
        {
            palette_bits.insert(value.to_bits());
        }
        if palette_bits.len() > MAX_PALETTE_LEN {
            return Err(FontMetricsCodecError::new(
                0,
                "metric palette exceeds u8 index capacity",
            ));
        }
        write_u32(
            &mut output,
            checked_u32(palette_bits.len(), "metric palette is too large")?,
        );
        let mut palette = BTreeMap::new();
        for (index, bits) in palette_bits.into_iter().enumerate() {
            write_u64(&mut output, bits);
            palette.insert(bits, u8::try_from(index).expect("validated palette index"));
        }
        let palette_index = |value: f64| palette[&value.to_bits()];

        for value in [
            table.default_em,
            table.svg_scale,
            table.svg_bbox_overhang_left_default_em,
            table.svg_bbox_overhang_right_default_em,
        ] {
            output.push(palette_index(value));
        }
        write_char_metrics(&mut output, &table.entries, &palette, "too many entries")?;
        write_pair_metrics(&mut output, &table.kern_pairs, &palette)?;
        write_pair_metrics(&mut output, &table.space_trigrams, &palette)?;
        write_trigram_metrics(&mut output, &table.trigrams, &palette)?;
        write_char_metrics(
            &mut output,
            &table.svg_bbox_overhang_left,
            &palette,
            "too many left overhangs",
        )?;
        write_char_metrics(
            &mut output,
            &table.svg_bbox_overhang_right,
            &palette,
            "too many right overhangs",
        )?;
        write_u16(
            &mut output,
            checked_u16(
                table.svg_vertical_glyphs.len(),
                "too many SVG vertical glyphs",
            )?,
        );
        for character in &table.svg_vertical_glyphs {
            write_u32(&mut output, *character as u32);
        }
        for shape in SvgVerticalDomShapeData::ALL {
            write_svg_vertical_profile_set(
                &mut output,
                &table.svg_vertical_profiles[shape.index()],
            )?;
        }
    }
    Ok(output)
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn write_svg_vertical_profile_set(
    output: &mut Vec<u8>,
    profile_set: &SvgVerticalProfileSetData,
) -> Result<(), FontMetricsCodecError> {
    let (tag, bbox_y_em, bbox_height_em, pair_union_max_delta_px, pair_union_exact, profiles) =
        match profile_set {
            SvgVerticalProfileSetData::Approximate {
                bbox_y_em,
                bbox_height_em,
                pair_union_max_delta_px,
            } => (
                0,
                *bbox_y_em,
                *bbox_height_em,
                *pair_union_max_delta_px,
                false,
                None,
            ),
            SvgVerticalProfileSetData::Profiled {
                approximate_bbox_y_em,
                approximate_bbox_height_em,
                pair_union_max_delta_px,
                pair_union_exact,
                profiles,
            } => (
                1,
                *approximate_bbox_y_em,
                *approximate_bbox_height_em,
                *pair_union_max_delta_px,
                *pair_union_exact,
                Some(profiles.as_slice()),
            ),
            SvgVerticalProfileSetData::Alias(target) => {
                output.push(2);
                output.push(target.to_byte());
                return Ok(());
            }
        };
    output.push(tag);
    write_u64(output, bbox_y_em.to_bits());
    write_u64(output, bbox_height_em.to_bits());
    write_u64(output, pair_union_max_delta_px.to_bits());
    let Some(profiles) = profiles else {
        return Ok(());
    };
    output.push(u8::from(pair_union_exact));
    output.push(u8::try_from(profiles.len()).expect("validated SVG vertical profile count"));
    for profile in profiles {
        output.push(profile.font_size_px);
        write_u16(
            output,
            checked_u16(
                profile.bbox_y_height_buckets.len(),
                "too many SVG vertical bbox buckets",
            )?,
        );
        for (bbox_y, bbox_height) in &profile.bbox_y_height_buckets {
            write_u64(output, bbox_y.to_bits());
            write_u64(output, bbox_height.to_bits());
        }
        output.extend_from_slice(&pack_vertical_bucket_indices(
            &profile.glyph_bucket_indices,
            profile.bbox_y_height_buckets.len(),
        )?);
    }
    Ok(())
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn write_char_metrics(
    output: &mut Vec<u8>,
    entries: &[(char, f64)],
    palette: &BTreeMap<u64, u8>,
    count_error: &'static str,
) -> Result<(), FontMetricsCodecError> {
    write_u16(output, checked_u16(entries.len(), count_error)?);
    for (character, value) in entries {
        write_u32(output, *character as u32);
        output.push(palette[&value.to_bits()]);
    }
    Ok(())
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn write_pair_metrics(
    output: &mut Vec<u8>,
    entries: &[(u32, u32, f64)],
    palette: &BTreeMap<u64, u8>,
) -> Result<(), FontMetricsCodecError> {
    write_u32(output, checked_u32(entries.len(), "too many pair metrics")?);
    for (left, right, value) in entries {
        output.push(u8::try_from(*left).expect("validated ASCII pair key"));
        output.push(u8::try_from(*right).expect("validated ASCII pair key"));
        output.push(palette[&value.to_bits()]);
    }
    Ok(())
}

#[cfg(any(test, feature = "font-metrics-generation"))]
fn write_trigram_metrics(
    output: &mut Vec<u8>,
    entries: &[(u32, u32, u32, f64)],
    palette: &BTreeMap<u64, u8>,
) -> Result<(), FontMetricsCodecError> {
    write_u32(
        output,
        checked_u32(entries.len(), "too many trigram metrics")?,
    );
    for (first, second, third, value) in entries {
        output.push(u8::try_from(*first).expect("validated ASCII trigram key"));
        output.push(u8::try_from(*second).expect("validated ASCII trigram key"));
        output.push(u8::try_from(*third).expect("validated ASCII trigram key"));
        output.push(palette[&value.to_bits()]);
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], FontMetricsCodecError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| FontMetricsCodecError::new(self.offset, "length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| FontMetricsCodecError::new(self.offset, "truncated profile"))?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, FontMetricsCodecError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, FontMetricsCodecError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("checked length"),
        ))
    }

    fn u32(&mut self) -> Result<u32, FontMetricsCodecError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("checked length"),
        ))
    }

    fn u64(&mut self) -> Result<u64, FontMetricsCodecError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("checked length"),
        ))
    }
}

fn palette_value(reader: &mut Reader<'_>, palette: &[f64]) -> Result<f64, FontMetricsCodecError> {
    let offset = reader.offset;
    palette
        .get(usize::from(reader.u8()?))
        .copied()
        .ok_or_else(|| FontMetricsCodecError::new(offset, "palette index out of bounds"))
}

fn read_char_metrics(
    reader: &mut Reader<'_>,
    palette: &[f64],
) -> Result<Vec<(char, f64)>, FontMetricsCodecError> {
    let count = usize::from(reader.u16()?);
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let offset = reader.offset;
        let character = char::from_u32(reader.u32()?)
            .ok_or_else(|| FontMetricsCodecError::new(offset, "invalid Unicode scalar"))?;
        entries.push((character, palette_value(reader, palette)?));
    }
    Ok(entries)
}

fn read_pair_metrics(
    reader: &mut Reader<'_>,
    palette: &[f64],
) -> Result<Vec<(u32, u32, f64)>, FontMetricsCodecError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| FontMetricsCodecError::new(reader.offset, "pair count overflow"))?;
    let minimum_bytes = count
        .checked_mul(3)
        .ok_or_else(|| FontMetricsCodecError::new(reader.offset, "pair byte length overflow"))?;
    if reader.bytes.len().saturating_sub(reader.offset) < minimum_bytes {
        return Err(FontMetricsCodecError::new(
            reader.offset,
            "truncated pair metrics",
        ));
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let left = u32::from(reader.u8()?);
        let right = u32::from(reader.u8()?);
        entries.push((left, right, palette_value(reader, palette)?));
    }
    Ok(entries)
}

fn read_trigram_metrics(
    reader: &mut Reader<'_>,
    palette: &[f64],
) -> Result<Vec<(u32, u32, u32, f64)>, FontMetricsCodecError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| FontMetricsCodecError::new(reader.offset, "trigram count overflow"))?;
    let minimum_bytes = count
        .checked_mul(4)
        .ok_or_else(|| FontMetricsCodecError::new(reader.offset, "trigram byte length overflow"))?;
    if reader.bytes.len().saturating_sub(reader.offset) < minimum_bytes {
        return Err(FontMetricsCodecError::new(
            reader.offset,
            "truncated trigram metrics",
        ));
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let first = u32::from(reader.u8()?);
        let second = u32::from(reader.u8()?);
        let third = u32::from(reader.u8()?);
        entries.push((first, second, third, palette_value(reader, palette)?));
    }
    Ok(entries)
}

fn read_svg_vertical_glyphs(reader: &mut Reader<'_>) -> Result<Vec<char>, FontMetricsCodecError> {
    let count = usize::from(reader.u16()?);
    let scalar_bytes = count.checked_mul(4).ok_or_else(|| {
        FontMetricsCodecError::new(reader.offset, "SVG vertical glyph byte overflow")
    })?;
    if reader.bytes.len().saturating_sub(reader.offset) < scalar_bytes {
        return Err(FontMetricsCodecError::new(
            reader.offset,
            "truncated SVG vertical glyph table",
        ));
    }
    let mut glyphs = Vec::with_capacity(count);
    for _ in 0..count {
        let offset = reader.offset;
        let scalar = reader.u32()?;
        glyphs.push(char::from_u32(scalar).ok_or_else(|| {
            FontMetricsCodecError::new(offset, "invalid SVG vertical glyph Unicode scalar")
        })?);
    }
    Ok(glyphs)
}

fn read_finite_f64(reader: &mut Reader<'_>) -> Result<f64, FontMetricsCodecError> {
    let offset = reader.offset;
    let value = f64::from_bits(reader.u64()?);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(FontMetricsCodecError::new(
            offset,
            "non-finite SVG vertical value",
        ))
    }
}

fn read_svg_vertical_size_profiles(
    reader: &mut Reader<'_>,
    glyph_count: usize,
) -> Result<Vec<SvgVerticalSizeProfileData>, FontMetricsCodecError> {
    let count = usize::from(reader.u8()?);
    if count == 0 || count > usize::from(SVG_VERTICAL_MAX_FONT_SIZE_PX) {
        return Err(FontMetricsCodecError::new(
            reader.offset - 1,
            "exact SVG vertical profile count is out of range",
        ));
    }
    let mut profiles = Vec::with_capacity(count);
    for _ in 0..count {
        let font_size_px = reader.u8()?;
        let bucket_count = usize::from(reader.u16()?);
        if bucket_count == 0 || bucket_count > MAX_SVG_VERTICAL_BUCKETS {
            return Err(FontMetricsCodecError::new(
                reader.offset - 2,
                "SVG vertical bucket count is out of range",
            ));
        }
        let mapping_length = packed_vertical_bucket_indices_len(glyph_count, bucket_count)?;
        let minimum_bytes = bucket_count
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(mapping_length))
            .ok_or_else(|| {
                FontMetricsCodecError::new(
                    reader.offset,
                    "SVG vertical profile byte length overflow",
                )
            })?;
        if reader.bytes.len().saturating_sub(reader.offset) < minimum_bytes {
            return Err(FontMetricsCodecError::new(
                reader.offset,
                "truncated SVG vertical profile",
            ));
        }
        let mut bbox_y_height_buckets = Vec::with_capacity(bucket_count);
        for _ in 0..bucket_count {
            let bucket_offset = reader.offset;
            let bbox_y = f64::from_bits(reader.u64()?);
            let bbox_height = f64::from_bits(reader.u64()?);
            if !bbox_y.is_finite() || !bbox_height.is_finite() {
                return Err(FontMetricsCodecError::new(
                    bucket_offset,
                    "SVG vertical bbox bucket is non-finite",
                ));
            }
            if bbox_height < 0.0 {
                return Err(FontMetricsCodecError::new(
                    bucket_offset,
                    "SVG vertical bbox bucket has negative height",
                ));
            }
            bbox_y_height_buckets.push((bbox_y, bbox_height));
        }
        let mapping_offset = reader.offset;
        let glyph_bucket_indices =
            unpack_vertical_bucket_indices(reader.take(mapping_length)?, glyph_count, bucket_count)
                .map_err(|error| FontMetricsCodecError::new(mapping_offset, error.message))?;
        profiles.push(SvgVerticalSizeProfileData {
            font_size_px,
            bbox_y_height_buckets,
            glyph_bucket_indices,
        });
    }
    Ok(profiles)
}

fn read_svg_vertical_profile_set(
    reader: &mut Reader<'_>,
    glyph_count: usize,
) -> Result<SvgVerticalProfileSetData, FontMetricsCodecError> {
    let tag_offset = reader.offset;
    match reader.u8()? {
        0 => Ok(SvgVerticalProfileSetData::Approximate {
            bbox_y_em: read_finite_f64(reader)?,
            bbox_height_em: read_finite_f64(reader)?,
            pair_union_max_delta_px: read_finite_f64(reader)?,
        }),
        1 => Ok(SvgVerticalProfileSetData::Profiled {
            approximate_bbox_y_em: read_finite_f64(reader)?,
            approximate_bbox_height_em: read_finite_f64(reader)?,
            pair_union_max_delta_px: read_finite_f64(reader)?,
            pair_union_exact: match reader.u8()? {
                0 => false,
                1 => true,
                _ => {
                    return Err(FontMetricsCodecError::new(
                        reader.offset - 1,
                        "invalid SVG vertical pair-union proof flag",
                    ));
                }
            },
            profiles: read_svg_vertical_size_profiles(reader, glyph_count)?,
        }),
        2 => {
            let shape_offset = reader.offset;
            Ok(SvgVerticalProfileSetData::Alias(
                SvgVerticalDomShapeData::from_byte(reader.u8()?, shape_offset)?,
            ))
        }
        _ => Err(FontMetricsCodecError::new(
            tag_offset,
            "invalid SVG vertical profile-set tag",
        )),
    }
}

pub fn decode_font_metrics_profile(
    bytes: &[u8],
) -> Result<Vec<FontMetricsTableData>, FontMetricsCodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(FontMetricsCodecError::new(
            0,
            "invalid magic or schema version",
        ));
    }
    let table_count = usize::from(reader.u16()?);
    let mut tables = Vec::with_capacity(table_count);
    for _ in 0..table_count {
        let key_offset = reader.offset;
        let key_length = usize::from(reader.u16()?);
        let font_key = std::str::from_utf8(reader.take(key_length)?)
            .map_err(|_| FontMetricsCodecError::new(key_offset, "font key is not UTF-8"))?
            .to_string();
        let variant_offset = reader.offset;
        let variant = FontMetricsVariantData::from_byte(reader.u8()?, variant_offset)?;
        let palette_count = usize::try_from(reader.u32()?)
            .map_err(|_| FontMetricsCodecError::new(reader.offset, "palette count overflow"))?;
        if palette_count > MAX_PALETTE_LEN {
            return Err(FontMetricsCodecError::new(
                reader.offset,
                "metric palette exceeds u8 index capacity",
            ));
        }
        let palette_bytes = palette_count
            .checked_mul(8)
            .ok_or_else(|| FontMetricsCodecError::new(reader.offset, "palette byte overflow"))?;
        if reader.bytes.len().saturating_sub(reader.offset) < palette_bytes {
            return Err(FontMetricsCodecError::new(
                reader.offset,
                "truncated palette",
            ));
        }
        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let value = f64::from_bits(reader.u64()?);
            if !value.is_finite() {
                return Err(FontMetricsCodecError::new(
                    reader.offset - 8,
                    "non-finite palette value",
                ));
            }
            palette.push(value);
        }
        let default_em = palette_value(&mut reader, &palette)?;
        let svg_scale = palette_value(&mut reader, &palette)?;
        let svg_bbox_overhang_left_default_em = palette_value(&mut reader, &palette)?;
        let svg_bbox_overhang_right_default_em = palette_value(&mut reader, &palette)?;
        let entries = read_char_metrics(&mut reader, &palette)?;
        let kern_pairs = read_pair_metrics(&mut reader, &palette)?;
        let space_trigrams = read_pair_metrics(&mut reader, &palette)?;
        let trigrams = read_trigram_metrics(&mut reader, &palette)?;
        let svg_bbox_overhang_left = read_char_metrics(&mut reader, &palette)?;
        let svg_bbox_overhang_right = read_char_metrics(&mut reader, &palette)?;
        let svg_vertical_glyphs = read_svg_vertical_glyphs(&mut reader)?;
        let mut svg_vertical_profile_sets = Vec::with_capacity(SvgVerticalDomShapeData::COUNT);
        for _ in SvgVerticalDomShapeData::ALL {
            svg_vertical_profile_sets.push(read_svg_vertical_profile_set(
                &mut reader,
                svg_vertical_glyphs.len(),
            )?);
        }
        let svg_vertical_profiles = svg_vertical_profile_sets.try_into().map_err(|_| {
            FontMetricsCodecError::new(reader.offset, "wrong SVG vertical profile-set count")
        })?;
        let table = FontMetricsTableData {
            font_key,
            variant,
            default_em,
            entries,
            kern_pairs,
            space_trigrams,
            trigrams,
            svg_scale,
            svg_bbox_overhang_left_default_em,
            svg_bbox_overhang_right_default_em,
            svg_bbox_overhang_left,
            svg_bbox_overhang_right,
            svg_vertical_glyphs,
            svg_vertical_profiles,
        };
        validate_table(&table)
            .map_err(|error| FontMetricsCodecError::new(reader.offset, error.message))?;
        tables.push(table);
    }
    if reader.offset != bytes.len() {
        return Err(FontMetricsCodecError::new(
            reader.offset,
            "trailing profile bytes",
        ));
    }
    let mut identities = BTreeSet::new();
    if tables
        .iter()
        .any(|table| !identities.insert((table.font_key.as_str(), table.variant)))
    {
        return Err(FontMetricsCodecError::new(0, "duplicate font table"));
    }
    Ok(tables)
}

pub(crate) fn decode_font_metrics_tables(
    bytes: &[u8],
) -> Result<&'static [FontMetricsTable], FontMetricsCodecError> {
    fn leak_profile_set(profile_set: SvgVerticalProfileSetData) -> SvgVerticalProfileSet {
        match profile_set {
            SvgVerticalProfileSetData::Approximate {
                bbox_y_em,
                bbox_height_em,
                pair_union_max_delta_px: _,
            } => SvgVerticalProfileSet::Approximate {
                bbox_y_em,
                bbox_height_em,
            },
            SvgVerticalProfileSetData::Profiled {
                approximate_bbox_y_em,
                approximate_bbox_height_em,
                pair_union_max_delta_px: _,
                pair_union_exact,
                profiles,
            } => SvgVerticalProfileSet::Profiled {
                approximate_bbox_y_em,
                approximate_bbox_height_em,
                pair_union_exact,
                profiles: Box::leak(
                    profiles
                        .into_iter()
                        .map(|profile| SvgVerticalSizeProfile {
                            font_size_px: profile.font_size_px,
                            bbox_y_height_buckets: Box::leak(
                                profile.bbox_y_height_buckets.into_boxed_slice(),
                            ),
                            glyph_bucket_indices: Box::leak(
                                profile.glyph_bucket_indices.into_boxed_slice(),
                            ),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
            },
            SvgVerticalProfileSetData::Alias(target) => {
                SvgVerticalProfileSet::Alias(target.to_runtime())
            }
        }
    }

    let tables = decode_font_metrics_profile(bytes)?
        .into_iter()
        .map(|table| FontMetricsTable {
            font_key: Box::leak(table.font_key.into_boxed_str()),
            variant: table.variant.to_runtime(),
            default_em: table.default_em,
            entries: Box::leak(table.entries.into_boxed_slice()),
            kern_pairs: Box::leak(table.kern_pairs.into_boxed_slice()),
            space_trigrams: Box::leak(table.space_trigrams.into_boxed_slice()),
            trigrams: Box::leak(table.trigrams.into_boxed_slice()),
            svg_scale: table.svg_scale,
            svg_bbox_overhang_left_default_em: table.svg_bbox_overhang_left_default_em,
            svg_bbox_overhang_right_default_em: table.svg_bbox_overhang_right_default_em,
            svg_bbox_overhang_left: Box::leak(table.svg_bbox_overhang_left.into_boxed_slice()),
            svg_bbox_overhang_right: Box::leak(table.svg_bbox_overhang_right.into_boxed_slice()),
            svg_vertical_glyphs: Box::leak(table.svg_vertical_glyphs.into_boxed_slice()),
            svg_vertical_profiles: table.svg_vertical_profiles.map(leak_profile_set),
        })
        .collect::<Vec<_>>();
    Ok(Box::leak(tables.into_boxed_slice()))
}

#[cfg(test)]
mod tests {
    use super::{
        FontMetricsTableData, FontMetricsVariantData, MAX_PALETTE_LEN, SvgVerticalDomShapeData,
        SvgVerticalProfileSetData, SvgVerticalSizeProfileData, decode_font_metrics_profile,
        encode_font_metrics_profile, unpack_vertical_bucket_indices,
    };

    #[cfg(any())]
    fn sample_vertical_glyphs() -> Vec<char> {
        (' '..='~')
            .chain(['°', '¶', 'ß', '\u{200b}', 'ﬂ'])
            .collect()
    }

    fn sample_table() -> FontMetricsTableData {
        FontMetricsTableData {
            font_key: "sample".to_string(),
            variant: FontMetricsVariantData::Regular,
            default_em: 0.5,
            entries: vec![(' ', 0.25), ('~', 0.75)],
            kern_pairs: vec![(33, 126, -0.125)],
            space_trigrams: vec![(33, 126, 0.125)],
            trigrams: vec![],
            svg_scale: 1.0,
            svg_bbox_overhang_left_default_em: 0.0,
            svg_bbox_overhang_right_default_em: 0.0,
            svg_bbox_overhang_left: vec![],
            svg_bbox_overhang_right: vec![],
            svg_vertical_glyphs: vec![],
            svg_vertical_profiles: std::array::from_fn(|_| {
                SvgVerticalProfileSetData::Approximate {
                    bbox_y_em: -0.9,
                    bbox_height_em: 1.1,
                    pair_union_max_delta_px: 0.0,
                }
            }),
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct SectionOffsets {
        palette_start: usize,
        entries_data: usize,
        kern_count: usize,
        kern_data: usize,
        trigram_count: usize,
        trigram_data: usize,
    }

    fn u16_at(bytes: &[u8], offset: usize) -> usize {
        usize::from(u16::from_le_bytes(
            bytes[offset..offset + 2].try_into().expect("u16 field"),
        ))
    }

    fn u32_at(bytes: &[u8], offset: usize) -> usize {
        usize::try_from(u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().expect("u32 field"),
        ))
        .expect("usize count")
    }

    fn first_table_section_offsets(bytes: &[u8]) -> SectionOffsets {
        let key_length = u16_at(bytes, 10);
        let palette_count_offset = 12 + key_length + 1;
        let palette_start = palette_count_offset + 4;
        let scalar_start = palette_start + u32_at(bytes, palette_count_offset) * 8;
        let entries_count = scalar_start + 4;
        let entries_data = entries_count + 2;
        let kern_count = entries_data + u16_at(bytes, entries_count) * 5;
        let kern_data = kern_count + 4;
        let space_count = kern_data + u32_at(bytes, kern_count) * 3;
        let space_data = space_count + 4;
        let trigram_count = space_data + u32_at(bytes, space_count) * 3;
        let trigram_data = trigram_count + 4;
        SectionOffsets {
            palette_start,
            entries_data,
            kern_count,
            kern_data,
            trigram_count,
            trigram_data,
        }
    }

    #[test]
    fn decoder_rejects_every_truncated_prefix_without_panicking() {
        let encoded = encode_font_metrics_profile(&[sample_table()]).expect("encoded profile");

        for length in 0..encoded.len() {
            let outcome =
                std::panic::catch_unwind(|| decode_font_metrics_profile(&encoded[..length]));
            assert!(
                matches!(outcome, Ok(Err(_))),
                "prefix ending at byte {length} must return an error: {outcome:?}"
            );
        }
    }

    #[test]
    fn decoder_rejects_truncated_and_corrupt_profiles() {
        let encoded = encode_font_metrics_profile(&[sample_table()]).expect("encoded profile");

        let mut previous_schema = encoded.clone();
        previous_schema[..8].copy_from_slice(b"MRMFNT04");
        assert!(
            decode_font_metrics_profile(&previous_schema)
                .expect_err("V4 profiles must not have a compatibility decode path")
                .to_string()
                .contains("schema version")
        );

        let mut bad_magic = encoded.clone();
        bad_magic[0] ^= 0xff;
        assert!(
            decode_font_metrics_profile(&bad_magic)
                .expect_err("bad magic")
                .to_string()
                .contains("magic")
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(
            decode_font_metrics_profile(&trailing)
                .expect_err("trailing byte")
                .to_string()
                .contains("trailing")
        );

        let key_length = usize::from(u16::from_le_bytes([encoded[10], encoded[11]]));
        let variant_offset = 12 + key_length;
        let mut bad_variant = encoded.clone();
        bad_variant[variant_offset] = 0xff;
        assert!(
            decode_font_metrics_profile(&bad_variant)
                .expect_err("bad variant")
                .to_string()
                .contains("variant")
        );

        let palette_count_offset = variant_offset + 1;
        let mut oversized_palette = encoded.clone();
        oversized_palette[palette_count_offset..palette_count_offset + 4].copy_from_slice(
            &u32::try_from(MAX_PALETTE_LEN + 1)
                .expect("palette limit")
                .to_le_bytes(),
        );
        assert_eq!(
            decode_font_metrics_profile(&oversized_palette)
                .expect_err("oversized palette")
                .to_string(),
            format!(
                "font metrics profile error at byte {}: metric palette exceeds u8 index capacity",
                palette_count_offset + 4
            )
        );

        let palette_count = u32::from_le_bytes(
            encoded[palette_count_offset..palette_count_offset + 4]
                .try_into()
                .expect("palette count"),
        ) as usize;
        let first_scalar_offset = palette_count_offset + 4 + palette_count * 8;
        let mut bad_palette_index = encoded;
        bad_palette_index[first_scalar_offset] = u8::MAX;
        assert!(
            decode_font_metrics_profile(&bad_palette_index)
                .expect_err("bad palette index")
                .to_string()
                .contains("palette index")
        );
    }

    #[test]
    fn decoder_rejects_invalid_palette_scalar_and_pair_data() {
        let encoded = encode_font_metrics_profile(&[sample_table()]).expect("encoded profile");
        let offsets = first_table_section_offsets(&encoded);

        let mut non_finite = encoded.clone();
        non_finite[offsets.palette_start..offsets.palette_start + 8]
            .copy_from_slice(&f64::INFINITY.to_bits().to_le_bytes());
        assert!(
            decode_font_metrics_profile(&non_finite)
                .expect_err("non-finite palette value")
                .to_string()
                .contains("non-finite palette")
        );

        let mut invalid_scalar = encoded.clone();
        invalid_scalar[offsets.entries_data..offsets.entries_data + 4]
            .copy_from_slice(&0x0000_d800_u32.to_le_bytes());
        assert!(
            decode_font_metrics_profile(&invalid_scalar)
                .expect_err("surrogate is not a Unicode scalar")
                .to_string()
                .contains("invalid Unicode scalar")
        );

        let mut invalid_pair = encoded;
        invalid_pair[offsets.kern_data] = b' ';
        assert!(
            decode_font_metrics_profile(&invalid_pair)
                .expect_err("space is outside the pair-key alphabet")
                .to_string()
                .contains("pair and trigram keys must be printable ASCII")
        );
    }

    #[test]
    fn decoder_rejects_unsorted_pairs_and_duplicate_tables() {
        let mut pair_table = sample_table();
        pair_table.kern_pairs = vec![(33, 125, -0.125), (33, 126, 0.125)];
        let mut unsorted = encode_font_metrics_profile(&[pair_table]).expect("sorted pair profile");
        let offsets = first_table_section_offsets(&unsorted);
        let second_pair_right = offsets.kern_data + 3 + 1;
        unsorted[second_pair_right] = 124;
        assert!(
            decode_font_metrics_profile(&unsorted)
                .expect_err("pair keys are no longer sorted")
                .to_string()
                .contains("kern pairs are not sorted")
        );

        let encoded = encode_font_metrics_profile(&[sample_table()]).expect("encoded profile");
        let mut duplicate = encoded.clone();
        duplicate[8..10].copy_from_slice(&2_u16.to_le_bytes());
        duplicate.extend_from_slice(&encoded[10..]);
        assert!(
            decode_font_metrics_profile(&duplicate)
                .expect_err("duplicate table identity")
                .to_string()
                .contains("duplicate font table")
        );
    }

    #[test]
    fn decoder_rejects_truncated_sections_and_oversized_counts() {
        let encoded = encode_font_metrics_profile(&[sample_table()]).expect("encoded profile");
        let offsets = first_table_section_offsets(&encoded);

        let pair_truncated = &encoded[..offsets.kern_data + 2];
        assert!(
            decode_font_metrics_profile(pair_truncated)
                .expect_err("partial pair record")
                .to_string()
                .contains("truncated pair metrics")
        );

        let mut oversized_pairs = encoded.clone();
        oversized_pairs[offsets.kern_count..offsets.kern_count + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            decode_font_metrics_profile(&oversized_pairs)
                .expect_err("oversized pair count")
                .to_string()
                .contains("pair")
        );

        let mut trigram_table = sample_table();
        trigram_table.trigrams = vec![(33, 34, 35, 0.125)];
        let trigram_encoded =
            encode_font_metrics_profile(&[trigram_table]).expect("trigram profile");
        let trigram_offsets = first_table_section_offsets(&trigram_encoded);
        let trigram_truncated = &trigram_encoded[..trigram_offsets.trigram_data + 3];
        assert!(
            decode_font_metrics_profile(trigram_truncated)
                .expect_err("partial trigram record")
                .to_string()
                .contains("truncated trigram metrics")
        );

        let mut oversized_trigrams = trigram_encoded;
        oversized_trigrams[trigram_offsets.trigram_count..trigram_offsets.trigram_count + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            decode_font_metrics_profile(&oversized_trigrams)
                .expect_err("oversized trigram count")
                .to_string()
                .contains("trigram")
        );
    }

    fn exact_profile_set(bbox_y: f64, bbox_height: f64) -> SvgVerticalProfileSetData {
        SvgVerticalProfileSetData::Profiled {
            approximate_bbox_y_em: bbox_y / 10.0,
            approximate_bbox_height_em: bbox_height / 10.0,
            pair_union_max_delta_px: 0.0,
            pair_union_exact: true,
            profiles: vec![SvgVerticalSizeProfileData {
                font_size_px: 10,
                bbox_y_height_buckets: vec![(0.0, 0.0), (bbox_y, bbox_height)],
                glyph_bucket_indices: vec![0, 1],
            }],
        }
    }

    #[test]
    fn v5_round_trip_preserves_independent_shapes_and_explicit_aliases() {
        let mut table = sample_table();
        table.svg_vertical_glyphs = vec![' ', 'A'];
        table.svg_vertical_profiles = [
            exact_profile_set(-9.0, 11.0),
            exact_profile_set(-8.0, 12.0),
            SvgVerticalProfileSetData::Alias(SvgVerticalDomShapeData::RawText),
            exact_profile_set(5.0, 11.0),
        ];

        let encoded = encode_font_metrics_profile(std::slice::from_ref(&table)).unwrap();
        assert_eq!(&encoded[..8], b"MRMFNT05");
        let decoded = decode_font_metrics_profile(&encoded).unwrap();
        assert_eq!(
            decoded[0].svg_vertical_profiles,
            table.svg_vertical_profiles
        );
        assert_ne!(
            decoded[0].svg_vertical_profiles[SvgVerticalDomShapeData::RawText.index()],
            decoded[0].svg_vertical_profiles[SvgVerticalDomShapeData::SingleTspan.index()],
        );
        assert_eq!(
            decoded[0].svg_vertical_profiles[SvgVerticalDomShapeData::CreateFormattedText.index()],
            SvgVerticalProfileSetData::Alias(SvgVerticalDomShapeData::RawText),
        );
    }

    #[test]
    fn aliases_must_target_an_earlier_dom_shape() {
        let mut table = sample_table();
        table.svg_vertical_profiles[SvgVerticalDomShapeData::RawText.index()] =
            SvgVerticalProfileSetData::Alias(SvgVerticalDomShapeData::SingleTspan);
        assert!(
            encode_font_metrics_profile(&[table])
                .expect_err("forward aliases could form a cycle")
                .to_string()
                .contains("earlier DOM shape")
        );
    }

    #[test]
    fn v5_vertical_profiles_preserve_native_f64_bits_and_packed_indices() {
        let precise_height = f64::from_bits(11.0_f64.to_bits() + 1);
        let mut table = sample_table();
        table.svg_vertical_glyphs = vec![' ', 'A'];
        table.svg_vertical_profiles[SvgVerticalDomShapeData::RawText.index()] =
            exact_profile_set(-9.0, precise_height);

        let decoded = decode_font_metrics_profile(
            &encode_font_metrics_profile(&[table]).expect("encode V5 profile"),
        )
        .expect("decode V5 profile");
        let SvgVerticalProfileSetData::Profiled { profiles, .. } =
            &decoded[0].svg_vertical_profiles[SvgVerticalDomShapeData::RawText.index()]
        else {
            panic!("raw text profile must remain exact");
        };
        assert_eq!(
            profiles[0].bbox_y_height_buckets[1].1.to_bits(),
            precise_height.to_bits()
        );
        assert!(
            unpack_vertical_bucket_indices(&[0b1110_0100], 3, 3)
                .expect_err("top bits are padding")
                .to_string()
                .contains("padding")
        );
    }
}
