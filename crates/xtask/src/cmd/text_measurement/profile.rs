//! Generalized font measurement profile generator.

use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static GENERATED_FILE_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const SVG_SCALE_PROBE_STRINGS: &[&str] = &[
    "AVATAR To Wa",
    "Merman 012345",
    "Quick brown 13",
    "[]{}()<>=+-_/:;",
    "ABCDEFGHIJKLM",
    "abcdefghijklm",
];

const MERMAID_CALCULATE_TEXT_DIMENSIONS_CONTEXT: &str = "mermaid-calculate-text-dimensions";
const MERMAID_CALCULATE_TEXT_DIMENSIONS_FONT_KEY: &str =
    "mermaid-calculate-text-dimensions-cssom-fallback";
const MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_FONT: &str = r#""Times New Roman", Times, serif"#;
const MERMAID_CALCULATE_TEXT_DIMENSIONS_REJECTED_FONT: &str =
    r#""trebuchet ms", verdana, arial, sans-serif;"#;
const MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_LINE_HEIGHT_PX: f64 = 17.0;
// Chrome exposes SVG advances on a 1/64px lattice. A 64x probe keeps pair/context facts in em
// space without accumulating the rounding noise of measuring every primitive at the 16px runtime
// baseline. The generated profile is still validated against the real 16px operation below.
const MERMAID_CALCULATE_TEXT_DIMENSIONS_METRICS_PROBE_SIZE_PX: f64 = 1024.0;
const SVG_VERTICAL_FALLBACK_PROBE_SIZE_PX: f64 = 1024.0;
const SVG_VERTICAL_MIN_FONT_SIZE_PX: u8 = 1;
const SVG_VERTICAL_MAX_FONT_SIZE_PX: u8 = 64;
const SVG_VERTICAL_PAIR_PROOF_SIZES_PX: [u8; 2] = [10, 16];
const MERMAID_ENTITY_PLACEHOLDER_CHARS: [char; 4] = ['ﬂ', '°', '¶', 'ß'];
const PINNED_FONT_METRICS_BROWSER_VERSION: &str = "131.0.6778.204";

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
struct SvgVerticalMeasurement {
    bbox_y: f64,
    bbox_height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
struct SvgVerticalShapeMeasurements {
    raw_text: SvgVerticalMeasurement,
    single_tspan: SvgVerticalMeasurement,
    create_formatted_text: SvgVerticalMeasurement,
    create_formatted_text_middle: SvgVerticalMeasurement,
}

impl SvgVerticalShapeMeasurements {
    fn get(self, shape: merman_render::text::SvgVerticalDomShapeData) -> SvgVerticalMeasurement {
        match shape {
            merman_render::text::SvgVerticalDomShapeData::RawText => self.raw_text,
            merman_render::text::SvgVerticalDomShapeData::SingleTspan => self.single_tspan,
            merman_render::text::SvgVerticalDomShapeData::CreateFormattedText => {
                self.create_formatted_text
            }
            merman_render::text::SvgVerticalDomShapeData::CreateFormattedTextMiddle => {
                self.create_formatted_text_middle
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SvgVerticalPairUnionProof {
    Pass {
        max_delta_px: f64,
    },
    Mismatch {
        font_size_px: u8,
        pair: (char, char),
        expected_y: f64,
        expected_height: f64,
        actual_y: f64,
        actual_height: f64,
        max_delta_px: f64,
    },
}

impl SvgVerticalPairUnionProof {
    fn max_delta_px(self) -> f64 {
        match self {
            Self::Pass { max_delta_px } | Self::Mismatch { max_delta_px, .. } => max_delta_px,
        }
    }

    fn merge(self, next: Self) -> Self {
        match (self, next) {
            (
                Self::Pass { max_delta_px: left },
                Self::Pass {
                    max_delta_px: right,
                },
            ) => Self::Pass {
                max_delta_px: left.max(right),
            },
            (
                Self::Pass { max_delta_px },
                Self::Mismatch {
                    font_size_px,
                    pair,
                    expected_y,
                    expected_height,
                    actual_y,
                    actual_height,
                    max_delta_px: mismatch_delta,
                },
            ) => Self::Mismatch {
                font_size_px,
                pair,
                expected_y,
                expected_height,
                actual_y,
                actual_height,
                max_delta_px: max_delta_px.max(mismatch_delta),
            },
            (
                Self::Mismatch {
                    font_size_px,
                    pair,
                    expected_y,
                    expected_height,
                    actual_y,
                    actual_height,
                    max_delta_px: mismatch_delta,
                },
                next,
            ) => Self::Mismatch {
                font_size_px,
                pair,
                expected_y,
                expected_height,
                actual_y,
                actual_height,
                max_delta_px: mismatch_delta.max(next.max_delta_px()),
            },
        }
    }
}

fn compact_svg_vertical_size_profile(
    font_size_px: u8,
    glyphs: &[char],
    measurements: &[SvgVerticalMeasurement],
) -> Result<merman_render::text::SvgVerticalSizeProfileData, XtaskError> {
    if measurements.len() != glyphs.len() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "SVG vertical DOM-shape probe at {font_size_px}px returned {} glyphs; expected {}",
            measurements.len(),
            glyphs.len()
        )));
    }
    if let Some(measurement) = measurements.iter().find(|measurement| {
        measurement.bbox_height < 0.0
            || !measurement.bbox_y.is_finite()
            || !measurement.bbox_height.is_finite()
    }) {
        return Err(XtaskError::SvgCompareFailed(format!(
            "SVG vertical DOM-shape probe at {font_size_px}px returned an invalid bbox: y={:?}, height={:?}",
            measurement.bbox_y, measurement.bbox_height
        )));
    }
    let bucket_bits = measurements
        .iter()
        .map(|measurement| {
            (
                measurement.bbox_y.to_bits(),
                measurement.bbox_height.to_bits(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    if bucket_bits.len() > usize::from(u8::MAX) + 1 {
        return Err(XtaskError::SvgCompareFailed(format!(
            "SVG vertical DOM-shape probe at {font_size_px}px exceeded the u8 bucket index capacity"
        )));
    }
    let mut bucket_index_by_bits = BTreeMap::new();
    let mut bbox_y_height_buckets = Vec::with_capacity(bucket_bits.len());
    for (index, (bbox_y_bits, bbox_height_bits)) in bucket_bits.into_iter().enumerate() {
        bucket_index_by_bits.insert(
            (bbox_y_bits, bbox_height_bits),
            u8::try_from(index).expect("validated vertical bucket index"),
        );
        bbox_y_height_buckets.push((
            f64::from_bits(bbox_y_bits),
            f64::from_bits(bbox_height_bits),
        ));
    }
    let glyph_bucket_indices = measurements
        .iter()
        .map(|measurement| {
            bucket_index_by_bits[&(
                measurement.bbox_y.to_bits(),
                measurement.bbox_height.to_bits(),
            )]
        })
        .collect();

    Ok(merman_render::text::SvgVerticalSizeProfileData {
        font_size_px,
        bbox_y_height_buckets,
        glyph_bucket_indices,
    })
}

fn verify_svg_vertical_pair_union(
    font_size_px: u8,
    char_measurements: &BTreeMap<char, SvgVerticalMeasurement>,
    pairs: &[(char, char)],
    pair_measurements: &[SvgVerticalMeasurement],
) -> Result<SvgVerticalPairUnionProof, XtaskError> {
    if pairs.len() != pair_measurements.len() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "SVG vertical pair-union proof at {font_size_px}px returned the wrong result count"
        )));
    }
    let mut first_mismatch = None;
    let mut max_delta_px = 0.0_f64;
    for ((left, right), measured) in pairs.iter().copied().zip(pair_measurements) {
        let left_bbox = char_measurements.get(&left).ok_or_else(|| {
            XtaskError::SvgCompareFailed(format!(
                "SVG vertical pair-union proof at {font_size_px}px is missing {left:?}"
            ))
        })?;
        let right_bbox = char_measurements.get(&right).ok_or_else(|| {
            XtaskError::SvgCompareFailed(format!(
                "SVG vertical pair-union proof at {font_size_px}px is missing {right:?}"
            ))
        })?;
        for (kind, measurement) in [
            ("left glyph", left_bbox),
            ("right glyph", right_bbox),
            ("pair", measured),
        ] {
            if !measurement.bbox_y.is_finite()
                || !measurement.bbox_height.is_finite()
                || measurement.bbox_height < 0.0
                || !(measurement.bbox_y + measurement.bbox_height).is_finite()
            {
                return Err(XtaskError::SvgCompareFailed(format!(
                    "SVG vertical pair-union proof at {font_size_px}px returned an invalid {kind} bbox for {left:?}{right:?}: y={:?}, height={:?}",
                    measurement.bbox_y, measurement.bbox_height
                )));
            }
        }
        let non_empty = [left_bbox, right_bbox]
            .into_iter()
            .filter(|bbox| bbox.bbox_height != 0.0)
            .collect::<Vec<_>>();
        let (expected_y, expected_height) = if non_empty.is_empty() {
            (0.0, 0.0)
        } else {
            let expected_y = non_empty
                .iter()
                .map(|bbox| bbox.bbox_y)
                .fold(f64::INFINITY, f64::min);
            let expected_bottom = non_empty
                .iter()
                .map(|bbox| bbox.bbox_y + bbox.bbox_height)
                .fold(f64::NEG_INFINITY, f64::max);
            (expected_y, (expected_bottom - expected_y).max(0.0))
        };
        let delta_px = (measured.bbox_y - expected_y)
            .abs()
            .max((measured.bbox_height - expected_height).abs());
        max_delta_px = max_delta_px.max(delta_px);
        if first_mismatch.is_none()
            && (measured.bbox_y.to_bits() != expected_y.to_bits()
                || measured.bbox_height.to_bits() != expected_height.to_bits())
        {
            first_mismatch = Some((
                (left, right),
                expected_y,
                expected_height,
                measured.bbox_y,
                measured.bbox_height,
            ));
        }
    }
    Ok(match first_mismatch {
        Some((pair, expected_y, expected_height, actual_y, actual_height)) => {
            SvgVerticalPairUnionProof::Mismatch {
                font_size_px,
                pair,
                expected_y,
                expected_height,
                actual_y,
                actual_height,
                max_delta_px,
            }
        }
        None => SvgVerticalPairUnionProof::Pass { max_delta_px },
    })
}

#[derive(Debug, Clone, PartialEq)]
struct SvgVerticalCapabilityData {
    approximate_bbox_y_em: f64,
    approximate_bbox_height_em: f64,
    pair_union_max_delta_px: f64,
    exact: bool,
    profiles: Vec<merman_render::text::SvgVerticalSizeProfileData>,
}

#[derive(Debug, Clone, PartialEq)]
struct SvgVerticalTableCapabilityData {
    glyphs: Vec<char>,
    shapes: [SvgVerticalCapabilityData; merman_render::text::SvgVerticalDomShapeData::COUNT],
}

fn finalize_svg_vertical_capability(
    approximate_bbox_y_em: f64,
    approximate_bbox_height_em: f64,
    profiles: Vec<merman_render::text::SvgVerticalSizeProfileData>,
    proof: SvgVerticalPairUnionProof,
) -> SvgVerticalCapabilityData {
    let max_delta_px = proof.max_delta_px();
    SvgVerticalCapabilityData {
        approximate_bbox_y_em,
        approximate_bbox_height_em,
        pair_union_max_delta_px: max_delta_px,
        exact: matches!(proof, SvgVerticalPairUnionProof::Pass { .. }),
        profiles,
    }
}

fn encode_svg_vertical_profile_sets(
    capabilities: [SvgVerticalCapabilityData; merman_render::text::SvgVerticalDomShapeData::COUNT],
) -> [merman_render::text::SvgVerticalProfileSetData;
    merman_render::text::SvgVerticalDomShapeData::COUNT] {
    use merman_render::text::{SvgVerticalDomShapeData, SvgVerticalProfileSetData};

    let mut encoded = Vec::with_capacity(SvgVerticalDomShapeData::COUNT);
    for shape in SvgVerticalDomShapeData::ALL {
        let capability = &capabilities[shape.index()];
        let alias = SvgVerticalDomShapeData::ALL[..shape.index()]
            .iter()
            .copied()
            .find(|candidate| {
                capability.exact
                    && capabilities[candidate.index()].exact
                    && *capability == capabilities[candidate.index()]
            });
        encoded.push(if let Some(target) = alias {
            SvgVerticalProfileSetData::Alias(target)
        } else {
            SvgVerticalProfileSetData::Profiled {
                approximate_bbox_y_em: capability.approximate_bbox_y_em,
                approximate_bbox_height_em: capability.approximate_bbox_height_em,
                pair_union_max_delta_px: capability.pair_union_max_delta_px,
                pair_union_exact: capability.exact,
                profiles: capability.profiles.clone(),
            }
        });
    }
    encoded.try_into().expect("one profile set per DOM shape")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum MeasurementContext {
    #[default]
    HtmlAndSvg,
    MermaidCalculateTextDimensions,
}

impl MeasurementContext {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "html-and-svg" => Some(Self::HtmlAndSvg),
            MERMAID_CALCULATE_TEXT_DIMENSIONS_CONTEXT => Some(Self::MermaidCalculateTextDimensions),
            _ => None,
        }
    }

    const fn cli_name(self) -> &'static str {
        match self {
            Self::HtmlAndSvg => "html-and-svg",
            Self::MermaidCalculateTextDimensions => MERMAID_CALCULATE_TEXT_DIMENSIONS_CONTEXT,
        }
    }

    const fn uses_body_attached_svg(self) -> bool {
        matches!(self, Self::MermaidCalculateTextDimensions)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MermaidCalculateTextDimensionsMetadata {
    base_font_size_px: f64,
    metrics_probe_font_size_px: f64,
    line_bbox_height_px: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FontVariant {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FontVariant {
    const ALL: [Self; 4] = [Self::Regular, Self::Bold, Self::Italic, Self::BoldItalic];

    const fn css_font_weight(self) -> &'static str {
        match self {
            Self::Regular | Self::Italic => "400",
            Self::Bold | Self::BoldItalic => "700",
        }
    }

    const fn css_font_style(self) -> &'static str {
        match self {
            Self::Regular | Self::Bold => "normal",
            Self::Italic | Self::BoldItalic => "italic",
        }
    }

    const fn profile_variant(self) -> merman_render::text::FontMetricsVariantData {
        match self {
            Self::Regular => merman_render::text::FontMetricsVariantData::Regular,
            Self::Bold => merman_render::text::FontMetricsVariantData::Bold,
            Self::Italic => merman_render::text::FontMetricsVariantData::Italic,
            Self::BoldItalic => merman_render::text::FontMetricsVariantData::BoldItalic,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalProbePlan {
    chars: Vec<char>,
    pairs: Vec<(char, char)>,
    internal_space_contexts: Vec<(char, char)>,
    trigrams: Vec<(char, char, char)>,
    svg_scale_strings: Vec<String>,
    overhang_chars: Vec<char>,
    variants: Vec<FontVariant>,
    svg_vertical_chars: Vec<char>,
    svg_vertical_pairs: Vec<(char, char)>,
    svg_vertical_font_sizes_px: Vec<u8>,
    svg_vertical_pair_proof_sizes_px: Vec<u8>,
}

fn canonical_probe_plan() -> CanonicalProbePlan {
    let mut chars = (b' '..=b'~').map(char::from).collect::<Vec<_>>();
    // NBSP is measured explicitly because the HTML probe uses it to expose the width of a lone
    // collapsible U+0020. Runtime measurement must not guess that the two code points are equal.
    chars.push('\u{00a0}');
    chars.extend(MERMAID_ENTITY_PLACEHOLDER_CHARS);
    let non_space_chars = chars
        .iter()
        .copied()
        .filter(|ch| ch.is_ascii() && !ch.is_whitespace())
        .collect::<Vec<_>>();
    let pairs = non_space_chars
        .iter()
        .copied()
        .flat_map(|left| {
            non_space_chars
                .iter()
                .copied()
                .map(move |right| (left, right))
        })
        .collect::<Vec<_>>();

    let svg_vertical_chars = (b' '..=b'~')
        .map(char::from)
        .chain(MERMAID_ENTITY_PLACEHOLDER_CHARS)
        .chain(std::iter::once('\u{200b}'))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let svg_vertical_pair_chars = svg_vertical_chars.clone();
    let svg_vertical_pairs = svg_vertical_pair_chars
        .iter()
        .copied()
        .flat_map(|left| {
            svg_vertical_pair_chars
                .iter()
                .copied()
                .map(move |right| (left, right))
        })
        .collect();

    CanonicalProbePlan {
        chars: chars.clone(),
        internal_space_contexts: pairs.clone(),
        pairs,
        // Ordinary trigrams are intentionally disabled until a canonical, fixture-independent
        // probe set has a demonstrated runtime benefit beyond pair and internal-space metrics.
        trigrams: Vec::new(),
        svg_scale_strings: SVG_SCALE_PROBE_STRINGS
            .iter()
            .map(|probe| (*probe).to_string())
            .collect(),
        overhang_chars: chars,
        variants: FontVariant::ALL.to_vec(),
        svg_vertical_chars,
        svg_vertical_pairs,
        svg_vertical_font_sizes_px: (SVG_VERTICAL_MIN_FONT_SIZE_PX..=SVG_VERTICAL_MAX_FONT_SIZE_PX)
            .collect(),
        svg_vertical_pair_proof_sizes_px: SVG_VERTICAL_PAIR_PROOF_SIZES_PX.to_vec(),
    }
}

fn normalize_font_key(s: &str) -> String {
    s.chars()
        .filter_map(|ch| {
            if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                None
            } else {
                Some(ch.to_ascii_lowercase())
            }
        })
        .collect()
}

fn extract_base_font_family(svg: &str) -> String {
    let Ok(doc) = roxmltree::Document::parse(svg) else {
        return String::new();
    };
    let Some(root) = doc.descendants().find(|node| node.has_tag_name("svg")) else {
        return String::new();
    };
    let id = root.attribute("id").unwrap_or_default();
    let Some(style_node) = doc.descendants().find(|node| node.has_tag_name("style")) else {
        return String::new();
    };
    let style_text = style_node.text().unwrap_or_default();
    if id.is_empty() || style_text.is_empty() {
        return String::new();
    }
    let pattern = format!(
        r#"#{id}\{{[^}}]*font-family:([^;}}]+)"#,
        id = regex::escape(id)
    );
    let Ok(regex) = Regex::new(&pattern) else {
        return String::new();
    };
    regex
        .captures(style_text)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().trim().to_string())
        .unwrap_or_default()
}

fn font_authority_from_svg_sources<'a>(
    svg_sources: impl IntoIterator<Item = &'a str>,
    explicit_font_families: &[&str],
) -> BTreeMap<String, String> {
    if !explicit_font_families.is_empty() {
        return explicit_font_families
            .iter()
            .map(|font_family| font_family.trim())
            .filter(|font_family| !font_family.is_empty())
            .filter_map(|font_family| {
                let font_key = normalize_font_key(font_family);
                (!font_key.is_empty()).then(|| (font_key, font_family.to_string()))
            })
            .collect();
    }

    let mut families = BTreeMap::new();
    for svg in svg_sources {
        let font_family = extract_base_font_family(svg);
        let font_key = normalize_font_key(&font_family);
        if !font_key.is_empty() {
            families.entry(font_key).or_insert(font_family);
        }
    }
    if !families.is_empty() {
        families
            .entry("sans-serif".to_string())
            .or_insert_with(|| "sans-serif".to_string());
    }
    families
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MeasurementSelection {
    fonts: BTreeMap<String, String>,
    probes: CanonicalProbePlan,
}

fn measurement_selection_from_svg_sources<'a>(
    svg_sources: impl IntoIterator<Item = &'a str>,
    explicit_font_families: &[&str],
) -> MeasurementSelection {
    MeasurementSelection {
        fonts: font_authority_from_svg_sources(svg_sources, explicit_font_families),
        probes: canonical_probe_plan(),
    }
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let mid = v.len() / 2;
    if v.len() % 2 == 1 {
        Some(v[mid])
    } else {
        Some((v[mid - 1] + v[mid]) / 2.0)
    }
}

fn write_generated_file_transactionally(path: &Path, contents: &[u8]) -> Result<(), XtaskError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let sequence = GENERATED_FILE_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("font_metrics.rs");
    let suffix = format!("{}-{sequence}", std::process::id());
    let temp_path = path.with_file_name(format!(".{file_name}.{suffix}.tmp"));
    let backup_path = path.with_file_name(format!(".{file_name}.{suffix}.backup"));

    let write_temp_result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)?;
        file.write_all(contents)?;
        file.sync_all()
    })();
    if let Err(source) = write_temp_result {
        let _ = fs::remove_file(&temp_path);
        return Err(XtaskError::WriteFile {
            path: temp_path.display().to_string(),
            source,
        });
    }

    let had_original = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => true,
        Ok(_) => {
            let _ = fs::remove_file(&temp_path);
            return Err(XtaskError::WriteFile {
                path: path.display().to_string(),
                source: std::io::Error::other("font metrics output path is not a file"),
            });
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => {
            let _ = fs::remove_file(&temp_path);
            return Err(XtaskError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };

    if had_original {
        fs::rename(path, &backup_path).map_err(|source| {
            let _ = fs::remove_file(&temp_path);
            XtaskError::WriteFile {
                path: backup_path.display().to_string(),
                source,
            }
        })?;
    }

    if let Err(source) = fs::rename(&temp_path, path) {
        let rollback_error = had_original
            .then(|| fs::rename(&backup_path, path).err())
            .flatten();
        let _ = fs::remove_file(&temp_path);
        return Err(XtaskError::WriteFile {
            path: path.display().to_string(),
            source: match rollback_error {
                Some(rollback_error) => std::io::Error::other(format!(
                    "failed to install generated font metrics: {source}; failed to restore backup: {rollback_error}"
                )),
                None => source,
            },
        });
    }

    if had_original && let Err(error) = fs::remove_file(&backup_path) {
        eprintln!(
            "warning: failed to remove generated font metrics backup {}: {error}",
            backup_path.display()
        );
    }
    Ok(())
}

fn compact_font_metrics_artifacts(
    tables: &[merman_render::text::FontMetricsTableData],
    binary_file_name: &str,
    calculate_text_dimensions: Option<MermaidCalculateTextDimensionsMetadata>,
) -> Result<(String, Vec<u8>), XtaskError> {
    let binary = merman_render::text::encode_font_metrics_profile(tables)
        .map_err(|error| XtaskError::SvgCompareFailed(error.to_string()))?;
    let decoded = merman_render::text::decode_font_metrics_profile(&binary)
        .map_err(|error| XtaskError::SvgCompareFailed(error.to_string()))?;
    if decoded.len() != tables.len() {
        return Err(XtaskError::SvgCompareFailed(
            "font metrics codec changed the table count".to_string(),
        ));
    }
    for (actual, expected) in decoded.iter().zip(tables) {
        if actual.font_key != expected.font_key
            || actual.variant != expected.variant
            || font_table_fact_bits(actual) != font_table_fact_bits(expected)
            || actual
                .entries
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>()
                != expected
                    .entries
                    .iter()
                    .map(|entry| entry.0)
                    .collect::<Vec<_>>()
            || actual
                .kern_pairs
                .iter()
                .map(|entry| (entry.0, entry.1))
                .collect::<Vec<_>>()
                != expected
                    .kern_pairs
                    .iter()
                    .map(|entry| (entry.0, entry.1))
                    .collect::<Vec<_>>()
            || actual
                .space_trigrams
                .iter()
                .map(|entry| (entry.0, entry.1))
                .collect::<Vec<_>>()
                != expected
                    .space_trigrams
                    .iter()
                    .map(|entry| (entry.0, entry.1))
                    .collect::<Vec<_>>()
            || actual
                .trigrams
                .iter()
                .map(|entry| (entry.0, entry.1, entry.2))
                .collect::<Vec<_>>()
                != expected
                    .trigrams
                    .iter()
                    .map(|entry| (entry.0, entry.1, entry.2))
                    .collect::<Vec<_>>()
            || actual
                .svg_bbox_overhang_left
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>()
                != expected
                    .svg_bbox_overhang_left
                    .iter()
                    .map(|entry| entry.0)
                    .collect::<Vec<_>>()
            || actual
                .svg_bbox_overhang_right
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>()
                != expected
                    .svg_bbox_overhang_right
                    .iter()
                    .map(|entry| entry.0)
                    .collect::<Vec<_>>()
            || actual.svg_vertical_glyphs != expected.svg_vertical_glyphs
            || actual.svg_vertical_profiles != expected.svg_vertical_profiles
        {
            return Err(XtaskError::SvgCompareFailed(format!(
                "font metrics codec round-trip changed {} {:?}",
                expected.font_key, expected.variant
            )));
        }
    }

    let digest = Sha256::digest(&binary);
    let mut source = String::new();
    let _ = writeln!(
        source,
        "// Generated by `xtask gen-font-metrics` from reusable browser font/DOM probes."
    );
    let _ = writeln!(
        source,
        "// Fixture ids and complete-text answers are intentionally excluded."
    );
    let _ = writeln!(source, "// Binary schema: MRMFNT05 (little-endian).");
    let _ = writeln!(source, "// Binary bytes: {}.", binary.len());
    let _ = writeln!(source, "// SHA-256: {digest:x}.");
    let _ = writeln!(
        source,
        "// Browser probe: Chrome {PINNED_FONT_METRICS_BROWSER_VERSION}."
    );
    let _ = writeln!(
        source,
        "// SVG vertical profiles: exact DOM-shape bbox facts at integer sizes 1..=64px; explicitly approximate 1024px-scaled fallback outside the canonical domain."
    );
    if let Some(metadata) = calculate_text_dimensions {
        let _ = writeln!(
            source,
            "// DOM operation: Mermaid calculateTextDimensions body-attached SVG `<text><tspan>`."
        );
        let _ = writeln!(
            source,
            "// Chrome {PINNED_FONT_METRICS_BROWSER_VERSION} CSSOM fallback resolves to {MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_FONT}."
        );
        let _ = writeln!(
            source,
            "// Canonical font facts probed at {}px; baseline line bbox: {}px at {}px.",
            metadata.metrics_probe_font_size_px,
            metadata.line_bbox_height_px,
            metadata.base_font_size_px
        );
    }
    for table in tables {
        let _ = writeln!(
            source,
            "// - {} {}: chars={}, pairs={}, spaces={}, trigrams={}, left-overhangs={}, right-overhangs={}, vertical-glyphs={}",
            table.font_key,
            table.variant.rust_name(),
            table.entries.len(),
            table.kern_pairs.len(),
            table.space_trigrams.len(),
            table.trigrams.len(),
            table.svg_bbox_overhang_left.len(),
            table.svg_bbox_overhang_right.len(),
            table.svg_vertical_glyphs.len(),
        );
        for shape in merman_render::text::SvgVerticalDomShapeData::ALL {
            use merman_render::text::SvgVerticalProfileSetData;
            let profile_set = &table.svg_vertical_profiles[shape.index()];
            let (capability, delta, sizes, buckets) = match profile_set {
                SvgVerticalProfileSetData::Approximate {
                    pair_union_max_delta_px,
                    ..
                } => ("approximate", *pair_union_max_delta_px, 0, 0),
                SvgVerticalProfileSetData::Profiled {
                    pair_union_max_delta_px,
                    pair_union_exact,
                    profiles,
                    ..
                } => (
                    if *pair_union_exact {
                        "exact"
                    } else {
                        "profiled-approximate-composition"
                    },
                    *pair_union_max_delta_px,
                    profiles.len(),
                    profiles
                        .iter()
                        .map(|profile| profile.bbox_y_height_buckets.len())
                        .sum(),
                ),
                SvgVerticalProfileSetData::Alias(target) => {
                    let _ = writeln!(
                        source,
                        "//   {}: alias={}, proof=bit-exact-canonical-domain",
                        shape.audit_name(),
                        target.audit_name(),
                    );
                    continue;
                }
            };
            let _ = writeln!(
                source,
                "//   {}: capability={}, pair-union-max-delta-px={:?}, sizes={}, buckets={}",
                shape.audit_name(),
                capability,
                delta,
                sizes,
                buckets,
            );
        }
    }
    let _ = writeln!(source);
    let _ = writeln!(
        source,
        "use crate::text::{{FontMetricsTable, FontMetricsVariant, decode_font_metrics_tables}};"
    );
    let _ = writeln!(source, "use std::sync::OnceLock;");
    let _ = writeln!(source);
    let profile_bytes =
        format!("const PROFILE_BYTES: &[u8] = include_bytes!({binary_file_name:?});");
    if profile_bytes.len() <= 100 {
        let _ = writeln!(source, "{profile_bytes}");
    } else {
        let _ = writeln!(source, "const PROFILE_BYTES: &[u8] =");
        let _ = writeln!(source, "    include_bytes!({binary_file_name:?});");
    }
    let _ = writeln!(
        source,
        "static FONT_METRICS_TABLES: OnceLock<&'static [FontMetricsTable]> = OnceLock::new();"
    );
    let _ = writeln!(source);
    let _ = writeln!(
        source,
        "fn font_metrics_tables() -> &'static [FontMetricsTable] {{"
    );
    let _ = writeln!(source, "    FONT_METRICS_TABLES.get_or_init(|| {{");
    let _ = writeln!(source, "        decode_font_metrics_tables(PROFILE_BYTES)");
    let _ = writeln!(
        source,
        "            .unwrap_or_else(|error| panic!(\"invalid generated font metrics profile: {{error}}\"))"
    );
    let _ = writeln!(source, "    }})");
    let _ = writeln!(source, "}}");
    let _ = writeln!(source);
    if calculate_text_dimensions.is_none() {
        let _ = writeln!(source, "pub fn lookup_font_metrics(");
        let _ = writeln!(source, "    font_key: &str,");
        let _ = writeln!(source, "    variant: FontMetricsVariant,");
        let _ = writeln!(source, ") -> Option<&'static FontMetricsTable> {{");
        let _ = writeln!(
            source,
            "    FontMetricsTable::lookup(font_metrics_tables(), font_key, variant)"
        );
        let _ = writeln!(source, "}}");
        let _ = writeln!(source);
    }
    let _ = writeln!(source, "pub fn lookup_exact_font_metrics(");
    let _ = writeln!(source, "    font_key: &str,");
    let _ = writeln!(source, "    variant: FontMetricsVariant,");
    let _ = writeln!(source, ") -> Option<&'static FontMetricsTable> {{");
    let _ = writeln!(
        source,
        "    FontMetricsTable::lookup_exact(font_metrics_tables(), font_key, variant)"
    );
    let _ = writeln!(source, "}}");
    Ok((source, binary))
}

fn font_table_fact_bits(table: &merman_render::text::FontMetricsTableData) -> Vec<u64> {
    let mut facts = std::iter::once(table.default_em)
        .chain(table.entries.iter().map(|entry| entry.1))
        .chain(table.kern_pairs.iter().map(|entry| entry.2))
        .chain(table.space_trigrams.iter().map(|entry| entry.2))
        .chain(table.trigrams.iter().map(|entry| entry.3))
        .chain(std::iter::once(table.svg_scale))
        .chain(std::iter::once(table.svg_bbox_overhang_left_default_em))
        .chain(std::iter::once(table.svg_bbox_overhang_right_default_em))
        .chain(table.svg_bbox_overhang_left.iter().map(|entry| entry.1))
        .chain(table.svg_bbox_overhang_right.iter().map(|entry| entry.1))
        .map(f64::to_bits)
        .collect::<Vec<_>>();
    use merman_render::text::SvgVerticalProfileSetData;
    for profile_set in &table.svg_vertical_profiles {
        match profile_set {
            SvgVerticalProfileSetData::Approximate {
                bbox_y_em,
                bbox_height_em,
                pair_union_max_delta_px,
            } => {
                facts.push(0);
                facts.extend([
                    bbox_y_em.to_bits(),
                    bbox_height_em.to_bits(),
                    pair_union_max_delta_px.to_bits(),
                ]);
            }
            SvgVerticalProfileSetData::Profiled {
                approximate_bbox_y_em,
                approximate_bbox_height_em,
                pair_union_max_delta_px,
                pair_union_exact,
                profiles,
            } => {
                facts.push(1);
                facts.extend([
                    approximate_bbox_y_em.to_bits(),
                    approximate_bbox_height_em.to_bits(),
                    pair_union_max_delta_px.to_bits(),
                ]);
                facts.push(u64::from(*pair_union_exact));
                for profile in profiles {
                    facts.push(u64::from(profile.font_size_px));
                    for (bbox_y, bbox_height) in &profile.bbox_y_height_buckets {
                        facts.extend([bbox_y.to_bits(), bbox_height.to_bits()]);
                    }
                    facts.extend(
                        profile
                            .glyph_bucket_indices
                            .iter()
                            .map(|index| u64::from(*index)),
                    );
                }
            }
            SvgVerticalProfileSetData::Alias(target) => {
                facts.extend([2, target.index() as u64]);
            }
        }
    }
    facts
}

fn write_compact_font_metrics_profile(
    out_path: &Path,
    tables: &[merman_render::text::FontMetricsTableData],
    calculate_text_dimensions: Option<MermaidCalculateTextDimensionsMetadata>,
) -> Result<(), XtaskError> {
    let binary_path = out_path.with_extension("bin");
    let binary_file_name = binary_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(XtaskError::Usage)?;
    let (source, binary) =
        compact_font_metrics_artifacts(tables, binary_file_name, calculate_text_dimensions)?;
    write_generated_file_transactionally(&binary_path, &binary)?;
    write_generated_file_transactionally(out_path, source.as_bytes())
}

pub(crate) fn gen_font_metrics(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut base_font_size_px: f64 = 16.0;
    let mut debug_text: Option<String> = None;
    let mut debug_dump: usize = 0;
    let mut backend: String = "browser".to_string();
    let mut browser_exe: Option<PathBuf> = None;
    let mut explicit_font_families: Vec<String> = Vec::new();
    let mut measurement_context = MeasurementContext::default();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--font-size" => {
                i += 1;
                base_font_size_px = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(16.0);
            }
            "--debug-text" => {
                i += 1;
                debug_text = args.get(i).map(|s| s.to_string());
            }
            "--debug-dump" => {
                i += 1;
                debug_dump = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
            }
            "--backend" => {
                i += 1;
                backend = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "browser".to_string());
            }
            "--browser-exe" => {
                i += 1;
                browser_exe = args.get(i).map(PathBuf::from);
            }
            "--font" => {
                i += 1;
                let font_family = args.get(i).ok_or(XtaskError::Usage)?.trim();
                if font_family.is_empty() {
                    return Err(XtaskError::Usage);
                }
                explicit_font_families.push(font_family.to_string());
            }
            "--context" => {
                i += 1;
                measurement_context = args
                    .get(i)
                    .and_then(|value| MeasurementContext::parse(value.trim()))
                    .ok_or(XtaskError::Usage)?;
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let out_path = out_path.ok_or(XtaskError::Usage)?;
    if !matches!(backend.as_str(), "browser" | "puppeteer") {
        return Err(XtaskError::Usage);
    }
    if !(base_font_size_px.is_finite() && base_font_size_px > 0.0) {
        return Err(XtaskError::Usage);
    }

    if measurement_context.uses_body_attached_svg()
        && (in_dir.is_some() || !explicit_font_families.is_empty())
    {
        return Err(XtaskError::Usage);
    }

    let mut svg_sources = Vec::new();
    if measurement_context == MeasurementContext::HtmlAndSvg && explicit_font_families.is_empty() {
        let in_dir = in_dir.as_deref().ok_or(XtaskError::Usage)?;
        let entries = fs::read_dir(in_dir).map_err(|source| XtaskError::ReadFile {
            path: in_dir.display().to_string(),
            source,
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_file_with_extension(&path, "svg") {
                continue;
            }
            svg_sources.push(
                fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })?,
            );
        }
    }

    let explicit_font_families = explicit_font_families
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let selection = match measurement_context {
        MeasurementContext::HtmlAndSvg => measurement_selection_from_svg_sources(
            svg_sources.iter().map(String::as_str),
            &explicit_font_families,
        ),
        MeasurementContext::MermaidCalculateTextDimensions => MeasurementSelection {
            fonts: BTreeMap::from([(
                MERMAID_CALCULATE_TEXT_DIMENSIONS_FONT_KEY.to_string(),
                MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_FONT.to_string(),
            )]),
            probes: canonical_probe_plan(),
        },
    };
    if selection.fonts.is_empty() {
        if !explicit_font_families.is_empty() {
            return Err(XtaskError::Usage);
        }
        return Err(XtaskError::SvgCompareFailed(format!(
            "no Mermaid root font-family declarations found under {}",
            in_dir
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<unspecified>".to_string())
        )));
    }
    let font_family_by_key = selection.fonts;
    let probe_plan = selection.probes;
    let metrics_probe_font_size_px = if measurement_context.uses_body_attached_svg() {
        MERMAID_CALCULATE_TEXT_DIMENSIONS_METRICS_PROBE_SIZE_PX
    } else {
        base_font_size_px
    };

    if let Some(debug_text) = debug_text.as_deref() {
        eprintln!(
            "debug-text={debug_text:?} is diagnostic-only and is not added to the canonical probes"
        );
    }
    if debug_dump > 0 {
        eprintln!(
            "measurement-context={} canonical probes: chars={} pairs={} internal_spaces={} trigrams={} svg_scale={} variants={}",
            measurement_context.cli_name(),
            probe_plan.chars.len(),
            probe_plan.pairs.len(),
            probe_plan.internal_space_contexts.len(),
            probe_plan.trigrams.len(),
            probe_plan.svg_scale_strings.len(),
            probe_plan.variants.len()
        );
        for probe in probe_plan.svg_scale_strings.iter().take(debug_dump) {
            eprintln!("  svg-scale={probe:?}");
        }
    }

    #[derive(Debug, Clone)]
    struct FontTable {
        font_key: String,
        variant: FontVariant,
        default_em: f64,
        entries: Vec<(char, f64)>,
        kern_pairs: Vec<(u32, u32, f64)>,
        /// Extra width adjustment (in `em`) for the pattern `a + ' ' + b`.
        ///
        /// In Chromium layout, the width contributed by a normal space can depend on surrounding
        /// glyphs (GPOS kerning around spaces, etc.). Measuring 2-char strings like `"e "` / `" T"`
        /// is unreliable because HTML collapses leading/trailing spaces. Instead, we capture the
        /// combined adjustment for internal spaces via these trigrams.
        space_trigrams: Vec<(u32, u32, f64)>,
        /// Extra width adjustment (in `em`) for the trigram pattern `a + b + c` (with no
        /// whitespace).
        ///
        /// The runtime schema retains this field for existing profiles. This generator leaves it
        /// empty rather than selecting ordinary trigrams from fixture substrings.
        trigrams: Vec<(u32, u32, u32, f64)>,
    }

    fn detect_windows_browser_exe() -> Option<PathBuf> {
        let candidates = [
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for p in candidates {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    fn verify_pinned_font_metrics_browser(browser_exe: &Path) -> Result<(), XtaskError> {
        let output = Command::new(browser_exe)
            .arg("--version")
            .output()
            .map_err(|source| {
                XtaskError::SvgCompareFailed(format!(
                    "failed to query font-metrics browser version: {source}"
                ))
            })?;
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() || !version.contains(PINNED_FONT_METRICS_BROWSER_VERSION) {
            return Err(XtaskError::SvgCompareFailed(format!(
                "font-metrics generation requires Chrome {PINNED_FONT_METRICS_BROWSER_VERSION}; got {version:?} from {}",
                browser_exe.display()
            )));
        }
        Ok(())
    }

    fn measure_text_widths_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        variant: FontVariant,
        strings: &[String],
        measurement_context: MeasurementContext,
    ) -> Result<Vec<f64>, XtaskError> {
        use std::process::Stdio;

        if strings.is_empty() {
            return Ok(Vec::new());
        }

        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "font_weight": variant.css_font_weight(),
            "font_style": variant.css_font_style(),
            "strings": strings,
            "measurement_context": measurement_context.cli_name(),
        })
        .to_string();

        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const fontWeight = input.font_weight;
const fontStyle = input.font_style;
const strings = input.strings;
const measurementContext = input.measurement_context;

 (async () => {
   const browser = await puppeteer.launch({
     headless: 'shell',
     executablePath: browserExe,
     args: [
       '--no-sandbox',
       '--disable-setuid-sandbox',
       // Match Mermaid CLI (Chromium) layout units more deterministically.
       '--force-device-scale-factor=1',
     ],
   });

   const page = await browser.newPage();
   await page.setViewport({ width: 800, height: 600, deviceScaleFactor: 1 });
   await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;} p{margin:0;}</style></head><body></body></html>`);

   const widths = await page.evaluate(({ strings, fontFamily, fontSizePx, fontWeight, fontStyle, measurementContext }) => {
     if (measurementContext === 'mermaid-calculate-text-dimensions') {
       const SVG_NS = 'http://www.w3.org/2000/svg';
       const svg = document.createElementNS(SVG_NS, 'svg');
       document.body.appendChild(svg);
       const out = [];
       for (const s of strings) {
         const text = document.createElementNS(SVG_NS, 'text');
         text.style.fontSize = `${fontSizePx}px`;
         text.style.fontWeight = fontWeight;
         text.style.fontStyle = fontStyle;
         text.style.fontFamily = fontFamily;
         const tspan = document.createElementNS(SVG_NS, 'tspan');
         const value = String(s);
         // A lone normal space collapses in this SVG DOM. Use NBSP only for the canonical
         // single-space advance; runtime trims edge spaces and uses this fact for internal spaces.
         tspan.textContent = value === ' ' ? '\u00A0' : value;
         text.appendChild(tspan);
         svg.appendChild(text);
         out.push(text.getComputedTextLength());
         text.remove();
       }
       return out;
     }

     const ff = String(fontFamily || '').replace(/;\s*$/, '');

     // Mimic Mermaid's single-line foreignObject label container.
     const div = document.createElement('div');
     div.style.display = 'table-cell';
     div.style.whiteSpace = 'nowrap';
     div.style.lineHeight = '1.5';
     div.style.maxWidth = '200px';
     div.style.textAlign = 'center';
     div.style.fontFamily = ff;
     div.style.fontSize = `${fontSizePx}px`;
     div.style.fontWeight = fontWeight;
     div.style.fontStyle = fontStyle;

     const span = document.createElement('span');
     span.className = 'nodeLabel';
     const p = document.createElement('p');
     span.appendChild(p);
     div.appendChild(span);
     document.body.appendChild(div);

     const out = [];
     for (const s of strings) {
       const ss = String(s);
       // A lone U+0020 would collapse away in HTML and measure as 0px. Use NBSP for that one
       // special case so we can still derive correct space advances for in-line spaces.
       p.textContent = ss === ' ' ? '\u00A0' : ss;
       out.push(div.getBoundingClientRect().width);
     }
     return out;
   }, { strings, fontFamily, fontSizePx, fontWeight, fontStyle, measurementContext });

  console.log(JSON.stringify(widths));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement failed".to_string(),
            ));
        }

        let widths_px: Vec<f64> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(widths_px.len());
        for w in widths_px {
            if w.is_finite() && w >= 0.0 {
                out.push(w);
            } else {
                out.push(0.0);
            }
        }
        Ok(out)
    }

    fn measure_svg_text_bbox_widths_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        variant: FontVariant,
        strings: &[String],
        measurement_context: MeasurementContext,
    ) -> Result<Vec<f64>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "font_weight": variant.css_font_weight(),
            "font_style": variant.css_font_style(),
            "strings": strings,
            "measurement_context": measurement_context.cli_name(),
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const fontWeight = input.font_weight;
const fontStyle = input.font_style;
const strings = input.strings;
const measurementContext = input.measurement_context;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const widths = await page.evaluate(({ strings, fontFamily, fontSizePx, fontWeight, fontStyle, measurementContext }) => {
    const out = [];
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '1000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    const bodyAttached = measurementContext === 'mermaid-calculate-text-dimensions';
    const ff = String(fontFamily || '').replace(/;\\s*$/, '');
    for (const s of strings) {
      const t = document.createElementNS(SVG_NS, 'text');
      t.setAttribute('x', '0');
      t.setAttribute('y', '0');
      if (bodyAttached) {
        t.style.fontFamily = fontFamily;
        t.style.fontSize = `${fontSizePx}px`;
        t.style.fontWeight = fontWeight;
        t.style.fontStyle = fontStyle;
        const tspan = document.createElementNS(SVG_NS, 'tspan');
        tspan.textContent = String(s);
        t.appendChild(tspan);
      } else {
        // Preserve spaces so `getComputedTextLength()` matches ordinary Mermaid SVG layout inputs.
        t.setAttribute('xml:space', 'preserve');
        t.setAttribute('style', `font-family:${ff};font-size:${fontSizePx}px;font-weight:${fontWeight};font-style:${fontStyle};white-space:pre;`);
        t.textContent = String(s);
      }
      svg.appendChild(t);
      out.push(t.getComputedTextLength());
      svg.removeChild(t);
    }
    return out;
  }, { strings, fontFamily, fontSizePx, fontWeight, fontStyle, measurementContext });

  console.log(JSON.stringify(widths));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;
        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser svg measurement failed".to_string(),
            ));
        }
        let widths_px: Vec<f64> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(widths_px.len());
        for w in widths_px {
            if w.is_finite() && w >= 0.0 {
                out.push(w);
            } else {
                out.push(0.0);
            }
        }
        Ok(out)
    }

    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct SvgTextBBoxMetrics {
        adv_px: f64,
        bbox_x: f64,
        bbox_w: f64,
        bbox_h: f64,
    }

    fn measure_svg_text_bbox_metrics_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        variant: FontVariant,
        strings: &[String],
        measurement_context: MeasurementContext,
    ) -> Result<Vec<SvgTextBBoxMetrics>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "font_weight": variant.css_font_weight(),
            "font_style": variant.css_font_style(),
            "strings": strings,
            "measurement_context": measurement_context.cli_name(),
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const fontWeight = input.font_weight;
const fontStyle = input.font_style;
const strings = input.strings;
const measurementContext = input.measurement_context;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const out = await page.evaluate(({ strings, fontFamily, fontSizePx, fontWeight, fontStyle, measurementContext }) => {
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '1000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    const bodyAttached = measurementContext === 'mermaid-calculate-text-dimensions';
    const ff = String(fontFamily || '').replace(/;\\s*$/, '');
    const res = [];
    for (const s of strings) {
      const t = document.createElementNS(SVG_NS, 'text');
      t.setAttribute('x', '0');
      t.setAttribute('y', '0');
      t.setAttribute('text-anchor', 'middle');
      if (bodyAttached) {
        t.style.fontFamily = fontFamily;
        t.style.fontSize = `${fontSizePx}px`;
        t.style.fontWeight = fontWeight;
        t.style.fontStyle = fontStyle;
        const tspan = document.createElementNS(SVG_NS, 'tspan');
        tspan.textContent = String(s);
        t.appendChild(tspan);
      } else {
        // Preserve spaces so bbox/advance measurements match ordinary Mermaid SVG output.
        t.setAttribute('xml:space', 'preserve');
        t.setAttribute('style', `font-family:${ff};font-size:${fontSizePx}px;font-weight:${fontWeight};font-style:${fontStyle};white-space:pre;`);
        t.textContent = String(s);
      }
      svg.appendChild(t);

      const adv = t.getComputedTextLength();
      const bb = t.getBBox();
      res.push({ adv_px: adv, bbox_x: bb.x, bbox_w: bb.width, bbox_h: bb.height });
      svg.removeChild(t);
    }
    return res;
  }, { strings, fontFamily, fontSizePx, fontWeight, fontStyle, measurementContext });

  console.log(JSON.stringify(out));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser svg measurement failed".to_string(),
            ));
        }
        let raw: Vec<SvgTextBBoxMetrics> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(raw.len());
        for m in raw {
            if m.adv_px.is_finite()
                && m.adv_px >= 0.0
                && m.bbox_x.is_finite()
                && m.bbox_w.is_finite()
                && m.bbox_h.is_finite()
            {
                out.push(m);
            } else {
                out.push(SvgTextBBoxMetrics {
                    adv_px: 0.0,
                    bbox_x: 0.0,
                    bbox_w: 0.0,
                    bbox_h: 0.0,
                });
            }
        }
        Ok(out)
    }

    fn measure_svg_vertical_batches_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        variant: FontVariant,
        batches: &[(f64, Vec<String>)],
        measurement_context: MeasurementContext,
    ) -> Result<Vec<Vec<SvgVerticalShapeMeasurements>>, XtaskError> {
        use std::process::Stdio;
        if batches.is_empty() {
            return Ok(Vec::new());
        }
        let batches = batches
            .iter()
            .map(|(font_size_px, strings)| {
                serde_json::json!({
                    "font_size_px": font_size_px,
                    "strings": strings,
                })
            })
            .collect::<Vec<_>>();
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_weight": variant.css_font_weight(),
            "font_style": variant.css_font_style(),
            "batches": batches,
            "measurement_context": measurement_context.cli_name(),
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: input.browser_exe,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--force-device-scale-factor=1'],
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 800, height: 600, deviceScaleFactor: 1 });
  await page.setContent('<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>');

	  const output = await page.evaluate(({ batches, fontFamily, fontWeight, fontStyle }) => {
	    const SVG_NS = 'http://www.w3.org/2000/svg';
	    const svg = document.createElementNS(SVG_NS, 'svg');
	    document.body.appendChild(svg);
	    const validFontFamily = String(fontFamily || '').replace(/;\s*$/, '');

	    const applyFont = (element, fontSizePx) => {
	      element.style.fontFamily = validFontFamily;
	      element.style.fontSize = `${fontSizePx}px`;
	      element.style.fontWeight = fontWeight;
	      element.style.fontStyle = fontStyle;
	    };
	    const readBBox = (element) => {
	      const bbox = element.getBBox();
	      return { bbox_y: bbox.y, bbox_height: bbox.height };
	    };

	    const measureRawText = (value, fontSizePx) => {
	      const text = document.createElementNS(SVG_NS, 'text');
	      text.setAttribute('x', '0');
	      text.setAttribute('y', '0');
	      text.setAttribute('xml:space', 'preserve');
	      text.style.whiteSpace = 'pre';
	      applyFont(text, fontSizePx);
	      text.textContent = String(value);
	      svg.appendChild(text);
	      const bbox = readBBox(text);
	      text.remove();
	      return bbox;
	    };

	    const measureSingleTspan = (value, fontSizePx) => {
	      const text = document.createElementNS(SVG_NS, 'text');
	      text.setAttribute('x', '0');
	      text.setAttribute('y', '0');
	      applyFont(text, fontSizePx);
	      const tspan = document.createElementNS(SVG_NS, 'tspan');
	      tspan.textContent = String(value);
	      text.appendChild(tspan);
	      svg.appendChild(text);
	      const bbox = readBBox(text);
	      text.remove();
	      return bbox;
	    };

	    const measureFormattedText = (value, fontSizePx, middle) => {
	      const owner = document.createElementNS(SVG_NS, 'g');
	      applyFont(owner, fontSizePx);
	      if (middle) {
	        owner.setAttribute('dy', '1em');
	        owner.setAttribute('alignment-baseline', 'middle');
	        owner.setAttribute('dominant-baseline', 'middle');
	        owner.setAttribute('text-anchor', 'middle');
	      }
	      const labelGroup = document.createElementNS(SVG_NS, 'g');
	      const background = document.createElementNS(SVG_NS, 'rect');
	      background.setAttribute('class', 'background');
	      background.setAttribute('style', 'stroke: none');
	      const text = document.createElementNS(SVG_NS, 'text');
	      text.setAttribute('y', '-10.1');
	      const outer = document.createElementNS(SVG_NS, 'tspan');
	      outer.setAttribute('class', 'text-outer-tspan');
	      outer.setAttribute('x', '0');
	      outer.setAttribute('y', '-0.1em');
	      outer.setAttribute('dy', '1.1em');
	      const inner = document.createElementNS(SVG_NS, 'tspan');
	      inner.setAttribute('class', 'text-inner-tspan');
	      inner.setAttribute('font-style', fontStyle);
	      inner.setAttribute('font-weight', fontWeight);
	      inner.textContent = String(value);
	      outer.appendChild(inner);
	      text.appendChild(outer);
	      labelGroup.appendChild(background);
	      labelGroup.appendChild(text);
	      owner.appendChild(labelGroup);
	      svg.appendChild(owner);
	      const bbox = readBBox(text);
	      owner.remove();
	      return bbox;
	    };

	    return batches.map(({ font_size_px: fontSizePx, strings }) => {
	      return strings.map((value) => {
	        return {
	          raw_text: measureRawText(value, fontSizePx),
	          single_tspan: measureSingleTspan(value, fontSizePx),
	          create_formatted_text: measureFormattedText(value, fontSizePx, false),
	          create_formatted_text_middle: measureFormattedText(value, fontSizePx, true),
	        };
	      });
    });
  }, {
    batches: input.batches,
    fontFamily: input.font_family,
    fontWeight: input.font_weight,
    fontStyle: input.font_style,
	  });

  console.log(JSON.stringify(output));
  await browser.close();
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
"#;

        let mut command = Command::new("node");
        command
            .current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = command.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser SVG vertical DOM-shape measurement failed".to_string(),
            ));
        }
        let measurements: Vec<Vec<SvgVerticalShapeMeasurements>> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        if measurements.len() != batches.len()
            || measurements.iter().zip(&batches).any(|(actual, expected)| {
                actual.len() != expected["strings"].as_array().map_or(0, Vec::len)
            })
            || measurements.iter().flatten().any(|measurements| {
                merman_render::text::SvgVerticalDomShapeData::ALL
                    .into_iter()
                    .map(|shape| measurements.get(shape))
                    .any(|measurement| {
                        !measurement.bbox_y.is_finite()
                            || !measurement.bbox_height.is_finite()
                            || measurement.bbox_height < 0.0
                    })
            })
        {
            return Err(XtaskError::SvgCompareFailed(
                "browser SVG vertical DOM-shape measurement returned invalid results".to_string(),
            ));
        }
        Ok(measurements)
    }

    fn build_tables_via_browser(
        font_family_by_key: &BTreeMap<String, String>,
        probe_plan: &CanonicalProbePlan,
        base_font_size_px: f64,
        browser_exe: Option<&Path>,
        measurement_context: MeasurementContext,
    ) -> Result<BTreeMap<(String, FontVariant), FontTable>, XtaskError> {
        let browser_exe = if let Some(p) = browser_exe {
            p.to_path_buf()
        } else if cfg!(windows) {
            detect_windows_browser_exe().ok_or_else(|| {
                XtaskError::SvgCompareFailed(
                    "no supported browser found for font measurement".into(),
                )
            })?
        } else {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement requires --browser-exe on this platform".into(),
            ));
        };

        let node_cwd = crate::cmd::mermaid_cli_root();

        let mut out = BTreeMap::new();
        for (font_key, font_family) in font_family_by_key {
            for variant in probe_plan.variants.iter().copied() {
                let chars = probe_plan.chars.clone();
                let char_strings = chars.iter().map(|ch| ch.to_string()).collect::<Vec<_>>();
                let widths_px = measure_text_widths_via_browser(
                    &node_cwd,
                    &browser_exe,
                    font_family,
                    base_font_size_px,
                    variant,
                    &char_strings,
                    measurement_context,
                )?;
                let mut measured: BTreeMap<char, f64> = BTreeMap::new();
                for (ch, w_px) in chars.iter().copied().zip(widths_px) {
                    let em = w_px / base_font_size_px.max(1.0);
                    if em.is_finite() && em >= 0.0 {
                        measured.insert(ch, em);
                    }
                }

                let char_em: BTreeMap<char, f64> = measured.clone();
                let mut entries = measured.into_iter().collect::<Vec<_>>();
                entries.sort_by_key(|a| a.0 as u32);

                let mut for_default = entries
                    .iter()
                    .filter(|(ch, _)| !ch.is_whitespace())
                    .map(|(_, v)| *v)
                    .collect::<Vec<_>>();
                let default_em = median(&mut for_default).unwrap_or_else(|| {
                    if entries.is_empty() {
                        0.6
                    } else {
                        entries.iter().map(|(_, v)| *v).sum::<f64>() / entries.len() as f64
                    }
                });

                let mut kern_pairs: Vec<(u32, u32, f64)> = Vec::new();
                if !probe_plan.pairs.is_empty() {
                    let pair_strings = probe_plan
                        .pairs
                        .iter()
                        .map(|(a, b)| format!("{a}{b}"))
                        .collect::<Vec<_>>();
                    let widths_px = measure_text_widths_via_browser(
                        &node_cwd,
                        &browser_exe,
                        font_family,
                        base_font_size_px,
                        variant,
                        &pair_strings,
                        measurement_context,
                    )?;
                    for ((a, b), w_px) in probe_plan.pairs.iter().copied().zip(widths_px) {
                        let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                        let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                        let pair_em = w_px / base_font_size_px.max(1.0);
                        if !(pair_em.is_finite() && a_em.is_finite() && b_em.is_finite()) {
                            continue;
                        }
                        let adj = pair_em - a_em - b_em;
                        if adj.abs() > 1e-9 && adj.is_finite() {
                            kern_pairs.push((a as u32, b as u32, adj));
                        }
                    }
                    kern_pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                }

                // Measure internal-space adjustments for `a + ' ' + b`.
                //
                // In Chromium, normal spaces can have context-dependent spacing due to kerning around
                // spaces and because U+0020 and U+00A0 are not guaranteed to share the same advance.
                // We cannot learn this from 2-char strings like `"e "` / `" T"` because HTML collapses
                // leading/trailing spaces, so we measure 3-char strings with the space in the middle.
                let mut space_trigrams: Vec<(u32, u32, f64)> = Vec::new();
                if !probe_plan.internal_space_contexts.is_empty() {
                    let space_strings = probe_plan
                        .internal_space_contexts
                        .iter()
                        .map(|(a, b)| format!("{a} {b}"))
                        .collect::<Vec<_>>();
                    let widths_px = measure_text_widths_via_browser(
                        &node_cwd,
                        &browser_exe,
                        font_family,
                        base_font_size_px,
                        variant,
                        &space_strings,
                        measurement_context,
                    )?;
                    let space_em = char_em.get(&' ').copied().unwrap_or(default_em);
                    for ((a, b), w_px) in probe_plan
                        .internal_space_contexts
                        .iter()
                        .copied()
                        .zip(widths_px)
                    {
                        let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                        let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                        let trigram_em = w_px / base_font_size_px.max(1.0);
                        if !(trigram_em.is_finite()
                            && a_em.is_finite()
                            && space_em.is_finite()
                            && b_em.is_finite())
                        {
                            continue;
                        }
                        let adj = trigram_em - a_em - space_em - b_em;
                        if adj.abs() > 1e-9 && adj.is_finite() {
                            space_trigrams.push((a as u32, b as u32, adj));
                        }
                    }
                    space_trigrams.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                }

                debug_assert!(probe_plan.trigrams.is_empty());
                let trigrams = Vec::new();

                out.insert(
                    (font_key.clone(), variant),
                    FontTable {
                        font_key: font_key.clone(),
                        variant,
                        default_em: default_em.max(0.1),
                        entries,
                        kern_pairs,
                        space_trigrams,
                        trigrams,
                    },
                );
            }
        }
        Ok(out)
    }

    let advance_tables = build_tables_via_browser(
        &font_family_by_key,
        &probe_plan,
        metrics_probe_font_size_px,
        browser_exe.as_deref(),
        measurement_context,
    )?;

    // Compare browser HTML and SVG measurements of the same fixed synthetic strings. Fixture
    // labels never participate in scale selection.
    let browser_exe = if let Some(path) = browser_exe.as_deref() {
        path.to_path_buf()
    } else if cfg!(windows) {
        detect_windows_browser_exe().ok_or_else(|| {
            XtaskError::SvgCompareFailed("no supported browser found for font measurement".into())
        })?
    } else {
        return Err(XtaskError::SvgCompareFailed(
            "browser measurement requires --browser-exe on this platform".into(),
        ));
    };
    let node_cwd = crate::cmd::mermaid_cli_root();
    verify_pinned_font_metrics_browser(&browser_exe)?;

    let mut svg_vertical_by_font: BTreeMap<(String, FontVariant), SvgVerticalTableCapabilityData> =
        BTreeMap::new();
    let vertical_char_strings = probe_plan
        .svg_vertical_chars
        .iter()
        .map(char::to_string)
        .collect::<Vec<_>>();
    let vertical_pair_strings = probe_plan
        .svg_vertical_pairs
        .iter()
        .map(|(left, right)| format!("{left}{right}"))
        .collect::<Vec<_>>();
    for (font_key, variant) in advance_tables.keys() {
        let Some(font_family) = font_family_by_key.get(font_key) else {
            continue;
        };
        let mut batches = probe_plan
            .svg_vertical_font_sizes_px
            .iter()
            .map(|font_size_px| (f64::from(*font_size_px), vertical_char_strings.clone()))
            .collect::<Vec<_>>();
        batches.push((
            SVG_VERTICAL_FALLBACK_PROBE_SIZE_PX,
            vertical_char_strings.clone(),
        ));
        batches.extend(
            probe_plan
                .svg_vertical_pair_proof_sizes_px
                .iter()
                .map(|font_size_px| (f64::from(*font_size_px), vertical_pair_strings.clone())),
        );
        let measurements = measure_svg_vertical_batches_via_browser(
            &node_cwd,
            &browser_exe,
            font_family,
            *variant,
            &batches,
            measurement_context,
        )?;
        let expected_batch_count = probe_plan.svg_vertical_font_sizes_px.len()
            + 1
            + probe_plan.svg_vertical_pair_proof_sizes_px.len();
        if measurements.len() != expected_batch_count {
            return Err(XtaskError::SvgCompareFailed(
                "browser SVG vertical measurement returned the wrong batch count".to_string(),
            ));
        }

        let mut shape_capabilities =
            Vec::with_capacity(merman_render::text::SvgVerticalDomShapeData::COUNT);
        for shape in merman_render::text::SvgVerticalDomShapeData::ALL {
            let mut profiles = Vec::with_capacity(probe_plan.svg_vertical_font_sizes_px.len());
            let mut char_measurements_by_size = BTreeMap::new();
            for (batch_index, font_size_px) in probe_plan
                .svg_vertical_font_sizes_px
                .iter()
                .copied()
                .enumerate()
            {
                let glyph_measurements = measurements[batch_index]
                    .iter()
                    .copied()
                    .map(|measurement| measurement.get(shape))
                    .collect::<Vec<_>>();
                char_measurements_by_size.insert(
                    font_size_px,
                    probe_plan
                        .svg_vertical_chars
                        .iter()
                        .copied()
                        .zip(glyph_measurements.iter().copied())
                        .collect::<BTreeMap<_, _>>(),
                );
                profiles.push(compact_svg_vertical_size_profile(
                    font_size_px,
                    &probe_plan.svg_vertical_chars,
                    &glyph_measurements,
                )?);
            }

            let fallback_batch_index = probe_plan.svg_vertical_font_sizes_px.len();
            let fallback_measurements = measurements[fallback_batch_index]
                .iter()
                .copied()
                .map(|measurement| measurement.get(shape))
                .filter(|measurement| measurement.bbox_height != 0.0)
                .collect::<Vec<_>>();
            let fallback_y = fallback_measurements
                .iter()
                .map(|measurement| measurement.bbox_y)
                .fold(f64::INFINITY, f64::min);
            let fallback_bottom = fallback_measurements
                .iter()
                .map(|measurement| measurement.bbox_y + measurement.bbox_height)
                .fold(f64::NEG_INFINITY, f64::max);
            let approximate_bbox_y_em = fallback_y / SVG_VERTICAL_FALLBACK_PROBE_SIZE_PX;
            let approximate_bbox_height_em =
                (fallback_bottom - fallback_y) / SVG_VERTICAL_FALLBACK_PROBE_SIZE_PX;
            if !(approximate_bbox_y_em.is_finite()
                && approximate_bbox_height_em.is_finite()
                && approximate_bbox_height_em > 0.0)
            {
                return Err(XtaskError::SvgCompareFailed(format!(
                    "browser SVG {} approximation is invalid for {font_key} {variant:?}",
                    shape.audit_name(),
                )));
            }

            let mut pair_union_proof = SvgVerticalPairUnionProof::Pass { max_delta_px: 0.0 };
            for (proof_index, font_size_px) in probe_plan
                .svg_vertical_pair_proof_sizes_px
                .iter()
                .copied()
                .enumerate()
            {
                let pair_batch_index = fallback_batch_index + 1 + proof_index;
                let pair_measurements = measurements[pair_batch_index]
                    .iter()
                    .copied()
                    .map(|measurement| measurement.get(shape))
                    .collect::<Vec<_>>();
                let char_measurements = char_measurements_by_size.get(&font_size_px).ok_or_else(
                    || {
                        XtaskError::SvgCompareFailed(format!(
                            "SVG {} pair-union proof size {font_size_px}px lacks character facts",
                            shape.audit_name(),
                        ))
                    },
                )?;
                pair_union_proof = pair_union_proof.merge(verify_svg_vertical_pair_union(
                    font_size_px,
                    char_measurements,
                    &probe_plan.svg_vertical_pairs,
                    &pair_measurements,
                )?);
            }
            shape_capabilities.push(finalize_svg_vertical_capability(
                approximate_bbox_y_em,
                approximate_bbox_height_em,
                profiles,
                pair_union_proof,
            ));
        }
        svg_vertical_by_font.insert(
            (font_key.clone(), *variant),
            SvgVerticalTableCapabilityData {
                glyphs: probe_plan.svg_vertical_chars.clone(),
                shapes: shape_capabilities
                    .try_into()
                    .expect("one capability per SVG vertical DOM shape"),
            },
        );
    }

    if measurement_context == MeasurementContext::MermaidCalculateTextDimensions {
        let baseline = measure_svg_text_bbox_metrics_via_browser(
            &node_cwd,
            &browser_exe,
            MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_FONT,
            base_font_size_px,
            FontVariant::Regular,
            &probe_plan.svg_scale_strings,
            measurement_context,
        )?;
        let cssom_fallback = measure_svg_text_bbox_metrics_via_browser(
            &node_cwd,
            &browser_exe,
            MERMAID_CALCULATE_TEXT_DIMENSIONS_REJECTED_FONT,
            base_font_size_px,
            FontVariant::Regular,
            &probe_plan.svg_scale_strings,
            measurement_context,
        )?;
        let same_resolution = baseline.iter().zip(&cssom_fallback).all(|(a, b)| {
            (a.adv_px - b.adv_px).abs() <= 1e-9
                && (a.bbox_x - b.bbox_x).abs() <= 1e-9
                && (a.bbox_w - b.bbox_w).abs() <= 1e-9
                && (a.bbox_h - b.bbox_h).abs() <= 1e-9
        });
        if baseline.len() != probe_plan.svg_scale_strings.len()
            || cssom_fallback.len() != baseline.len()
            || !same_resolution
        {
            return Err(XtaskError::SvgCompareFailed(format!(
                "{MERMAID_CALCULATE_TEXT_DIMENSIONS_CONTEXT} CSSOM fallback does not resolve to the pinned {MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_FONT} baseline"
            )));
        }

        let line_height = cssom_fallback
            .get(1)
            .map(|metrics| metrics.bbox_h.round())
            .unwrap_or_default();
        if line_height != MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_LINE_HEIGHT_PX {
            return Err(XtaskError::SvgCompareFailed(format!(
                "{MERMAID_CALCULATE_TEXT_DIMENSIONS_CONTEXT} baseline line bbox changed: expected {MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_LINE_HEIGHT_PX}px, got {line_height}px"
            )));
        }
    }

    let mut svg_scales_by_font: BTreeMap<(String, FontVariant), f64> = BTreeMap::new();
    for (font_key, variant) in advance_tables.keys() {
        let Some(font_family) = font_family_by_key.get(font_key) else {
            continue;
        };
        let html_widths = measure_text_widths_via_browser(
            &node_cwd,
            &browser_exe,
            font_family,
            metrics_probe_font_size_px,
            *variant,
            &probe_plan.svg_scale_strings,
            measurement_context,
        )?;
        let svg_widths = measure_svg_text_bbox_widths_via_browser(
            &node_cwd,
            &browser_exe,
            font_family,
            metrics_probe_font_size_px,
            *variant,
            &probe_plan.svg_scale_strings,
            measurement_context,
        )?;
        let mut scales = html_widths
            .into_iter()
            .zip(svg_widths)
            .filter_map(|(html_width, svg_width)| {
                (html_width.is_finite() && html_width > 0.0 && svg_width.is_finite())
                    .then_some(svg_width / html_width)
            })
            .filter(|scale| (0.5..=2.0).contains(scale))
            .collect::<Vec<_>>();
        if let Some(scale) = median(&mut scales) {
            svg_scales_by_font.insert((font_key.clone(), *variant), scale.clamp(0.5, 2.0));
        }
    }

    // Derive first/last-character bbox overhangs (relative to the `text-anchor=middle` position)
    // from browser SVG metrics. This models the fact that SVG `getBBox()` can be asymmetric due to
    // glyph overhangs. Overhangs are stored in `em` and applied on top of scaled advances.
    type SvgBBoxOverhangs = (f64, f64, Vec<(char, f64)>, Vec<(char, f64)>);
    let mut svg_bbox_overhangs_by_font: BTreeMap<(String, FontVariant), SvgBBoxOverhangs> =
        BTreeMap::new();
    if matches!(backend.as_str(), "browser" | "puppeteer") {
        for (font_key, variant) in advance_tables.keys() {
            let Some(font_family) = font_family_by_key.get(font_key) else {
                continue;
            };

            let chars = probe_plan.overhang_chars.clone();
            let strings = chars.iter().map(|ch| ch.to_string()).collect::<Vec<_>>();
            let metrics = measure_svg_text_bbox_metrics_via_browser(
                &node_cwd,
                &browser_exe,
                font_family,
                metrics_probe_font_size_px.max(1.0),
                *variant,
                &strings,
                measurement_context,
            )?;

            let mut left_all: Vec<f64> = Vec::new();
            let mut right_all: Vec<f64> = Vec::new();
            let mut left_by_char: BTreeMap<char, f64> = BTreeMap::new();
            let mut right_by_char: BTreeMap<char, f64> = BTreeMap::new();
            for (ch, m) in chars.iter().copied().zip(metrics) {
                let adv_px = m.adv_px;
                let bbox_x = m.bbox_x;
                let bbox_w = m.bbox_w;
                if !(adv_px.is_finite()
                    && adv_px >= 0.0
                    && bbox_x.is_finite()
                    && bbox_w.is_finite())
                {
                    continue;
                }
                let left_extent = (-bbox_x).max(0.0);
                let right_extent = (bbox_x + bbox_w).max(0.0);
                let half = (adv_px / 2.0).max(0.0);
                let denom = metrics_probe_font_size_px.max(1.0);
                let left_em = ((left_extent - half) / denom).clamp(-0.2, 0.2);
                let right_em = ((right_extent - half) / denom).clamp(-0.2, 0.2);
                left_all.push(left_em);
                right_all.push(right_em);
                left_by_char.insert(ch, left_em);
                right_by_char.insert(ch, right_em);
            }

            let default_left = median(&mut left_all).unwrap_or(0.0).clamp(-0.2, 0.2);
            let default_right = median(&mut right_all).unwrap_or(0.0).clamp(-0.2, 0.2);

            let mut left_entries: Vec<(char, f64)> = Vec::new();
            let mut right_entries: Vec<(char, f64)> = Vec::new();
            for (ch, v) in left_by_char {
                if (v - default_left).abs() > 1e-6 {
                    left_entries.push((ch, v));
                }
            }
            for (ch, v) in right_by_char {
                if (v - default_right).abs() > 1e-6 {
                    right_entries.push((ch, v));
                }
            }
            left_entries.sort_by_key(|(ch, _)| *ch as u32);
            right_entries.sort_by_key(|(ch, _)| *ch as u32);

            svg_bbox_overhangs_by_font.insert(
                (font_key.clone(), *variant),
                (default_left, default_right, left_entries, right_entries),
            );
        }
    }

    type FontTableWithBrowserFacts = (
        FontTable,
        f64,
        SvgBBoxOverhangs,
        SvgVerticalTableCapabilityData,
    );
    let mut tables: Vec<FontTableWithBrowserFacts> = Vec::new();
    for ((font_key, variant), t) in advance_tables {
        debug_assert_eq!(t.variant, variant);
        let scale = svg_scales_by_font
            .get(&(font_key.clone(), variant))
            .copied()
            .unwrap_or(1.0);
        let overhangs = svg_bbox_overhangs_by_font
            .get(&(font_key, variant))
            .cloned()
            .unwrap_or((0.0, 0.0, Vec::new(), Vec::new()));
        let vertical = svg_vertical_by_font
            .remove(&(t.font_key.clone(), variant))
            .ok_or_else(|| {
                XtaskError::SvgCompareFailed(format!(
                    "missing SVG vertical DOM-shape profiles for {} {variant:?}",
                    t.font_key
                ))
            })?;
        tables.push((t, scale, overhangs, vertical));
    }

    let profile_tables = tables
        .into_iter()
        .map(
            |(table, svg_scale, (left_default, right_default, left, right), vertical)| {
                merman_render::text::FontMetricsTableData {
                    font_key: table.font_key,
                    variant: table.variant.profile_variant(),
                    default_em: table.default_em,
                    entries: table.entries,
                    kern_pairs: table.kern_pairs,
                    space_trigrams: table.space_trigrams,
                    trigrams: table.trigrams,
                    svg_scale,
                    svg_bbox_overhang_left_default_em: left_default,
                    svg_bbox_overhang_right_default_em: right_default,
                    svg_bbox_overhang_left: left,
                    svg_bbox_overhang_right: right,
                    svg_vertical_glyphs: vertical.glyphs,
                    svg_vertical_profiles: encode_svg_vertical_profile_sets(vertical.shapes),
                }
            },
        )
        .collect::<Vec<_>>();
    let calculate_text_dimensions = measurement_context.uses_body_attached_svg().then_some(
        MermaidCalculateTextDimensionsMetadata {
            base_font_size_px,
            metrics_probe_font_size_px,
            line_bbox_height_px: MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_LINE_HEIGHT_PX,
        },
    );
    write_compact_font_metrics_profile(&out_path, &profile_tables, calculate_text_dimensions)
}

#[cfg(test)]
mod tests {
    use super::{
        FontVariant, MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_LINE_HEIGHT_PX,
        MERMAID_CALCULATE_TEXT_DIMENSIONS_FONT_KEY,
        MERMAID_CALCULATE_TEXT_DIMENSIONS_METRICS_PROBE_SIZE_PX, MERMAID_ENTITY_PLACEHOLDER_CHARS,
        MeasurementContext, MermaidCalculateTextDimensionsMetadata, SvgVerticalCapabilityData,
        SvgVerticalMeasurement, SvgVerticalPairUnionProof, canonical_probe_plan,
        compact_font_metrics_artifacts, compact_svg_vertical_size_profile,
        encode_svg_vertical_profile_sets, gen_font_metrics, measurement_selection_from_svg_sources,
        verify_svg_vertical_pair_union, write_generated_file_transactionally,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "merman-font-metrics-{name}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn approximate_vertical_profile_sets() -> [merman_render::text::SvgVerticalProfileSetData;
        merman_render::text::SvgVerticalDomShapeData::COUNT] {
        std::array::from_fn(
            |_| merman_render::text::SvgVerticalProfileSetData::Approximate {
                bbox_y_em: -0.9,
                bbox_height_em: 1.1,
                pair_union_max_delta_px: 0.125,
            },
        )
    }

    #[test]
    fn generated_font_metrics_replace_existing_output_transactionally() {
        let temp = TestDir::new("transactional-output");
        let output = temp.path().join("generated.rs");
        fs::write(&output, "old output").unwrap();

        write_generated_file_transactionally(&output, b"new output").unwrap();

        assert_eq!(fs::read_to_string(&output).unwrap(), "new output");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn compact_profile_artifacts_are_deterministic_and_self_auditing() {
        let table = merman_render::text::FontMetricsTableData {
            font_key: "audit".to_string(),
            variant: merman_render::text::FontMetricsVariantData::BoldItalic,
            default_em: f64::from_bits(0x3fe0_0000_0000_0001),
            entries: vec![(' ', 0.25), ('~', 0.75)],
            kern_pairs: vec![(33, 126, -0.0)],
            space_trigrams: vec![(33, 126, 0.125)],
            trigrams: vec![],
            svg_scale: 1.0,
            svg_bbox_overhang_left_default_em: 0.0,
            svg_bbox_overhang_right_default_em: 0.0,
            svg_bbox_overhang_left: vec![('!', 0.03125)],
            svg_bbox_overhang_right: vec![('~', 0.0625)],
            svg_vertical_glyphs: vec![],
            svg_vertical_profiles: approximate_vertical_profile_sets(),
        };

        let first = compact_font_metrics_artifacts(std::slice::from_ref(&table), "audit.bin", None)
            .unwrap();
        let second = compact_font_metrics_artifacts(&[table], "audit.bin", None).unwrap();

        assert_eq!(first, second);
        assert!(first.0.contains("Binary schema: MRMFNT05 (little-endian)"));
        assert!(first.0.contains("audit BoldItalic: chars=2, pairs=1"));
        assert!(first.0.contains(
            "raw-text: capability=approximate, pair-union-max-delta-px=0.125, sizes=0, buckets=0"
        ));
        assert!(
            first
                .0
                .contains("create-formatted-text-middle: capability=approximate")
        );
        assert!(first.0.contains("include_bytes!(\"audit.bin\")"));
        assert!(!first.0.contains("complete-label-answer"));
    }

    #[test]
    fn calculate_text_dimensions_profile_records_its_dom_and_font_resolution() {
        let table = merman_render::text::FontMetricsTableData {
            font_key: MERMAID_CALCULATE_TEXT_DIMENSIONS_FONT_KEY.to_string(),
            variant: merman_render::text::FontMetricsVariantData::Regular,
            default_em: 0.5,
            entries: vec![('M', 0.75)],
            kern_pairs: vec![],
            space_trigrams: vec![],
            trigrams: vec![],
            svg_scale: 1.0,
            svg_bbox_overhang_left_default_em: 0.0,
            svg_bbox_overhang_right_default_em: 0.0,
            svg_bbox_overhang_left: vec![],
            svg_bbox_overhang_right: vec![],
            svg_vertical_glyphs: vec![],
            svg_vertical_profiles: approximate_vertical_profile_sets(),
        };
        let metadata = MermaidCalculateTextDimensionsMetadata {
            base_font_size_px: 16.0,
            metrics_probe_font_size_px: MERMAID_CALCULATE_TEXT_DIMENSIONS_METRICS_PROBE_SIZE_PX,
            line_bbox_height_px: MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_LINE_HEIGHT_PX,
        };

        let (source, _) =
            compact_font_metrics_artifacts(&[table], "operation.bin", Some(metadata)).unwrap();

        assert!(source.contains("body-attached SVG `<text><tspan>`"));
        assert!(source.contains(r#"resolves to "Times New Roman", Times, serif"#));
        assert!(!source.contains("BASE_FONT_SIZE_PX"));
        assert!(!source.contains("LINE_BBOX_HEIGHT_PX"));
        assert!(source.contains("lookup_exact_font_metrics"));
    }

    #[test]
    fn checked_in_compact_profiles_match_the_generator_template() {
        let generated_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../merman-render/src/generated");
        for stem in [
            "mermaid_font_metrics_11_16_0",
            "mermaid_calculate_text_dimensions_font_metrics_11_16_0",
        ] {
            let binary_name = format!("{stem}.bin");
            let binary = fs::read(generated_dir.join(&binary_name)).expect("generated binary");
            let tables = merman_render::text::decode_font_metrics_profile(&binary)
                .expect("decode generated binary");
            let metadata = (stem == "mermaid_calculate_text_dimensions_font_metrics_11_16_0")
                .then_some(MermaidCalculateTextDimensionsMetadata {
                    base_font_size_px: 16.0,
                    metrics_probe_font_size_px:
                        MERMAID_CALCULATE_TEXT_DIMENSIONS_METRICS_PROBE_SIZE_PX,
                    line_bbox_height_px: MERMAID_CALCULATE_TEXT_DIMENSIONS_BASELINE_LINE_HEIGHT_PX,
                });
            let (source, regenerated_binary) =
                compact_font_metrics_artifacts(&tables, &binary_name, metadata)
                    .expect("render artifacts");

            assert_eq!(regenerated_binary, binary);
            assert_eq!(
                source,
                fs::read_to_string(generated_dir.join(format!("{stem}.rs")))
                    .expect("generated wrapper")
            );
        }
    }

    #[test]
    fn heuristic_profile_generation_is_rejected() {
        let temp = TestDir::new("heuristic-profile");
        let input = temp.path().join("input");
        let output = temp.path().join("generated.rs");
        fs::create_dir_all(&input).unwrap();

        let result = gen_font_metrics(vec![
            "--in".to_string(),
            input.display().to_string(),
            "--out".to_string(),
            output.display().to_string(),
            "--backend".to_string(),
            "heuristic".to_string(),
        ]);

        assert!(matches!(result, Err(crate::XtaskError::Usage)));
        assert!(!output.exists());
    }

    #[test]
    fn measurement_authority_ignores_fixture_labels_and_widths() {
        let first = r#"<svg id="diagram"><style>#diagram{font-family:"Trebuchet MS", verdana, sans-serif;font-size:16px}</style><foreignObject width="88.6875"><p>fixture-only alpha</p></foreignObject><text textLength="31">alpha</text></svg>"#;
        let changed = r#"<svg id="diagram"><style>#diagram{font-family:"Trebuchet MS", verdana, sans-serif;font-size:72px}</style><foreignObject width="9999"><p>completely different beta</p></foreignObject><text textLength="9876">beta</text></svg>"#;

        let first_selection = measurement_selection_from_svg_sources([first], &[]);
        let changed_selection = measurement_selection_from_svg_sources([changed], &[]);

        assert_eq!(first_selection, changed_selection);
        assert_eq!(first_selection.fonts.len(), 2);
        assert!(first_selection.fonts.contains_key("sans-serif"));
        assert!(
            first_selection
                .fonts
                .contains_key("trebuchetms,verdana,sans-serif")
        );
    }

    #[test]
    fn canonical_probe_plan_is_fixed_and_variant_explicit() {
        let probes = canonical_probe_plan();

        assert_eq!(probes.chars.len(), 100);
        assert!(probes.chars.contains(&'\u{00a0}'));
        assert!(
            MERMAID_ENTITY_PLACEHOLDER_CHARS
                .iter()
                .all(|character| probes.chars.contains(character))
        );
        assert_eq!(probes.pairs.len(), 94 * 94);
        assert_eq!(probes.internal_space_contexts, probes.pairs);
        assert!(probes.trigrams.is_empty());
        assert_eq!(probes.overhang_chars, probes.chars);
        assert_eq!(
            probes.variants,
            vec![
                FontVariant::Regular,
                FontVariant::Bold,
                FontVariant::Italic,
                FontVariant::BoldItalic,
            ]
        );
        assert_eq!(probes.svg_scale_strings.len(), 6);
        let expected_vertical_chars = (b' '..=b'~')
            .map(char::from)
            .chain(MERMAID_ENTITY_PLACEHOLDER_CHARS)
            .chain(std::iter::once('\u{200b}'))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(probes.svg_vertical_chars, expected_vertical_chars);
        assert_eq!(probes.svg_vertical_pairs.len(), 100 * 100);
        assert!(MERMAID_ENTITY_PLACEHOLDER_CHARS.iter().all(|character| {
            probes
                .svg_vertical_pairs
                .contains(&(*character, *character))
        }));
        assert_eq!(
            probes.svg_vertical_font_sizes_px,
            (1_u8..=64).collect::<Vec<_>>()
        );
        assert_eq!(probes.svg_vertical_pair_proof_sizes_px, vec![10, 16]);
    }

    #[test]
    fn vertical_profiles_preserve_shape_facts_and_alias_only_bit_exact_capabilities() {
        use merman_render::text::{SvgVerticalDomShapeData, SvgVerticalProfileSetData};

        let glyphs = vec![' ', 'A'];
        let measurements = [
            SvgVerticalMeasurement {
                bbox_y: 0.0,
                bbox_height: 0.0,
            },
            SvgVerticalMeasurement {
                bbox_y: -9.0,
                bbox_height: 11.05078125,
            },
        ];
        let profile = compact_svg_vertical_size_profile(10, &glyphs, &measurements).unwrap();
        assert_eq!(profile.bbox_y_height_buckets.len(), 2);

        let by_char = glyphs
            .iter()
            .copied()
            .zip(measurements)
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            verify_svg_vertical_pair_union(
                10,
                &by_char,
                &[(' ', 'A')],
                &[SvgVerticalMeasurement {
                    bbox_y: -9.0,
                    bbox_height: 11.05078125,
                }],
            )
            .unwrap(),
            SvgVerticalPairUnionProof::Pass { max_delta_px: 0.0 }
        );

        let capability = |bbox_y, bbox_height| SvgVerticalCapabilityData {
            approximate_bbox_y_em: bbox_y / 10.0,
            approximate_bbox_height_em: bbox_height / 10.0,
            pair_union_max_delta_px: 0.0,
            exact: true,
            profiles: vec![profile.clone()],
        };
        let encoded = encode_svg_vertical_profile_sets([
            capability(-9.0, 11.05078125),
            capability(-9.0, 11.05078125),
            capability(1.0, 11.05078125),
            capability(5.1875, 11.05078125),
        ]);
        assert_eq!(
            encoded[SvgVerticalDomShapeData::SingleTspan.index()],
            SvgVerticalProfileSetData::Alias(SvgVerticalDomShapeData::RawText)
        );
        assert!(matches!(
            encoded[SvgVerticalDomShapeData::CreateFormattedText.index()],
            SvgVerticalProfileSetData::Profiled { .. }
        ));
        assert_ne!(
            encoded[SvgVerticalDomShapeData::CreateFormattedText.index()],
            encoded[SvgVerticalDomShapeData::CreateFormattedTextMiddle.index()]
        );
    }

    #[test]
    fn explicit_fonts_are_authoritative_without_fixture_input() {
        let selection = measurement_selection_from_svg_sources(
            [r#"<svg id="ignored"><style>#ignored{font-family:Fixture Font}</style></svg>"#],
            &["System UI, sans-serif", "serif"],
        );

        assert_eq!(
            selection.fonts,
            std::collections::BTreeMap::from([
                ("serif".to_string(), "serif".to_string()),
                (
                    "systemui,sans-serif".to_string(),
                    "System UI, sans-serif".to_string(),
                ),
            ])
        );
    }

    #[test]
    fn calculate_text_dimensions_context_is_operation_owned() {
        assert_eq!(
            MeasurementContext::parse("mermaid-calculate-text-dimensions"),
            Some(MeasurementContext::MermaidCalculateTextDimensions)
        );
        assert!(MeasurementContext::MermaidCalculateTextDimensions.uses_body_attached_svg());
        assert!(!MeasurementContext::HtmlAndSvg.uses_body_attached_svg());
    }
}
