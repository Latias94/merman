// Copyright 2024 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use fontdb::{Database, ID};
use svgtypes::FontFamily;

use self::layout::DatabaseExt;
use crate::{Cache, Font, FontStretch, FontStyle, Text};

pub(crate) mod flatten;

mod colr;
/// Provides access to the layout of a text node.
pub mod layout;

/// Checks the syntax of a BCP 47 tag (RFC 5646, sections 2.1 and 2.2).
/// Language selection does not require looking up subtags in the IANA registry.
pub(crate) fn is_language_tag(tag: &str) -> bool {
    // These grandfathered tags are the exceptions to the regular grammar.
    const IRREGULAR: &[&str] = &[
        "en-GB-oed",
        "i-ami",
        "i-bnn",
        "i-default",
        "i-enochian",
        "i-hak",
        "i-klingon",
        "i-lux",
        "i-mingo",
        "i-navajo",
        "i-pwn",
        "i-tao",
        "i-tay",
        "i-tsu",
        "sgn-BE-FR",
        "sgn-BE-NL",
        "sgn-CH-DE",
    ];
    let alpha = |s: &str| s.bytes().all(|b| b.is_ascii_alphabetic());
    let mut parts = tag.split('-').peekable();
    if !parts
        .clone()
        .all(|s| (1..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return false;
    }
    let language = parts.next().unwrap();
    if language.eq_ignore_ascii_case("x") {
        return parts.peek().is_some();
    }
    if parts.peek().is_none() {
        return language.len() >= 2 && alpha(language);
    }
    if IRREGULAR.iter().any(|s| s.eq_ignore_ascii_case(tag)) {
        return true;
    }
    if language.len() < 2 || !alpha(language) {
        return false;
    }
    if language.len() <= 3 {
        for _ in 0..3 {
            if parts.next_if(|s| s.len() == 3 && alpha(s)).is_none() {
                break;
            }
        }
    }
    // The optional script must precede the optional region.
    parts.next_if(|s| s.len() == 4 && alpha(s));
    parts.next_if(|s| {
        (s.len() == 2 && alpha(s)) || (s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit()))
    });

    // Variants and extension singletons must each be unique within the tag.
    let mut seen = std::collections::HashSet::new();
    while let Some(variant) =
        parts.next_if(|s| s.len() >= 5 || (s.len() == 4 && s.as_bytes()[0].is_ascii_digit()))
    {
        if !seen.insert(variant.to_ascii_lowercase()) {
            return false;
        }
    }
    while let Some(singleton) = parts.next() {
        if singleton.eq_ignore_ascii_case("x") {
            return parts.peek().is_some();
        }
        if singleton.len() != 1
            || !seen.insert(singleton.to_ascii_lowercase())
            || parts.next_if(|s| s.len() >= 2).is_none()
        {
            return false;
        }
        while parts.next_if(|s| s.len() >= 2).is_some() {}
    }
    true
}

// Use the same coordinates for shaping and outline extraction.
fn apply_variations(
    face: &mut rustybuzz::ttf_parser::Face<'_>,
    variations: &[crate::FontVariation],
    font_size: f32,
    optical_sizing: crate::FontOpticalSizing,
) {
    use rustybuzz::ttf_parser::Tag;

    if !face.is_variable() {
        return;
    }

    for variation in variations {
        face.set_variation(Tag::from_bytes(&variation.tag), variation.value);
    }
    if optical_sizing == crate::FontOpticalSizing::Auto
        && !variations.iter().any(|variation| variation.tag == *b"opsz")
    {
        face.set_variation(Tag::from_bytes(b"opsz"), font_size);
    }
}

/// A shorthand for [FontResolver]'s font selection function.
///
/// This function receives a font specification (families + a style, weight,
/// stretch triple) and a font database and should return the ID of the font
/// that shall be used (if any).
///
/// In the basic case, the function will search the existing fonts in the
/// database to find a good match, e.g. via
/// [`Database::query`](fontdb::Database::query). This is what the [default
/// implementation](FontResolver::default_font_selector) does.
///
/// Users with more complex requirements can mutate the database to load
/// additional fonts dynamically. To perform mutation, it is recommended to call
/// `Arc::make_mut` on the provided database. (This call is not done outside of
/// the callback to not needless clone an underlying shared database if no
/// mutation will be performed.) It is important that the database is only
/// mutated additively. Removing fonts or replacing the entire database will
/// break things.
pub type FontSelectionFn<'a> =
    Box<dyn Fn(&Font, &mut Arc<Database>) -> Option<ID> + Send + Sync + 'a>;

/// A shorthand for [FontResolver]'s fallback selection function.
///
/// This function receives a specific character, a list of already used fonts,
/// and a font database. It should return the ID of a font that
/// - is not any of the already used fonts
/// - is as close as possible to the first already used font (if any)
/// - supports the given character
///
/// The function can search the existing database, but can also load additional
/// fonts dynamically. See the documentation of [`FontSelectionFn`] for more
/// details.
pub type FallbackSelectionFn<'a> =
    Box<dyn Fn(char, &[ID], &mut Arc<Database>) -> Option<ID> + Send + Sync + 'a>;

/// Selects a face and its relative size for a missing shaping cluster.
/// The callback receives the authored font, the cluster text, its language and
/// direction, and the faces already tried for that cluster.
pub type ClusterFallbackFn<'a> = Box<
    dyn Fn(&Font, &str, Option<&str>, bool, &[ID], &mut Arc<Database>) -> Option<(ID, f32)>
        + Send
        + Sync
        + 'a,
>;

/// Optional context for text shaping and missing-cluster selection.
#[derive(Default)]
pub struct TextFallback<'a> {
    pub(crate) language: Option<String>,
    pub(crate) select: Option<ClusterFallbackFn<'a>>,
}

impl std::fmt::Debug for TextFallback<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextFallback")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// A font resolver for `<text>` elements.
///
/// This type can be useful if you want to have an alternative font handling to
/// the default one. By default, only fonts specified upfront in
/// [`Options::fontdb`](crate::Options::fontdb) will be used. This type allows
/// you to load additional fonts on-demand and customize the font selection
/// process.
pub struct FontResolver<'a> {
    /// Resolver function that will be used when selecting a specific font
    /// for a generic [`Font`] specification.
    pub select_font: FontSelectionFn<'a>,

    /// Resolver function that will be used when selecting a fallback font for a
    /// character.
    pub select_fallback: FallbackSelectionFn<'a>,
}

impl Default for FontResolver<'_> {
    fn default() -> Self {
        FontResolver {
            select_font: FontResolver::default_font_selector(),
            select_fallback: FontResolver::default_fallback_selector(),
        }
    }
}

impl FontResolver<'_> {
    /// Creates a default font selection resolver.
    ///
    /// The default implementation forwards to
    /// [`query`](fontdb::Database::query) on the font database specified in the
    /// [`Options`](crate::Options).
    pub fn default_font_selector() -> FontSelectionFn<'static> {
        Box::new(move |font, fontdb| {
            let mut name_list = Vec::new();
            for family in &font.families {
                name_list.push(match family {
                    FontFamily::Serif => fontdb::Family::Serif,
                    FontFamily::SansSerif => fontdb::Family::SansSerif,
                    FontFamily::Cursive => fontdb::Family::Cursive,
                    FontFamily::Fantasy => fontdb::Family::Fantasy,
                    FontFamily::Monospace => fontdb::Family::Monospace,
                    FontFamily::Named(s) => fontdb::Family::Name(s),
                });
            }

            // Use the default font as fallback.
            name_list.push(fontdb::Family::Serif);

            let stretch = match font.stretch {
                FontStretch::UltraCondensed => fontdb::Stretch::UltraCondensed,
                FontStretch::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
                FontStretch::Condensed => fontdb::Stretch::Condensed,
                FontStretch::SemiCondensed => fontdb::Stretch::SemiCondensed,
                FontStretch::Normal => fontdb::Stretch::Normal,
                FontStretch::SemiExpanded => fontdb::Stretch::SemiExpanded,
                FontStretch::Expanded => fontdb::Stretch::Expanded,
                FontStretch::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
                FontStretch::UltraExpanded => fontdb::Stretch::UltraExpanded,
            };

            let style = match font.style {
                FontStyle::Normal => fontdb::Style::Normal,
                FontStyle::Italic => fontdb::Style::Italic,
                FontStyle::Oblique => fontdb::Style::Oblique,
            };

            let query = fontdb::Query {
                families: &name_list,
                weight: fontdb::Weight(font.weight),
                stretch,
                style,
            };

            let id = fontdb.query(&query);
            if id.is_none() {
                log::warn!(
                    "No match for '{}' font-family.",
                    font.families
                        .iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            id
        })
    }

    /// Creates a default font fallback selection resolver.
    ///
    /// The default implementation searches through the entire `fontdb`
    /// to find a font that has the correct style and supports the character.
    pub fn default_fallback_selector() -> FallbackSelectionFn<'static> {
        Box::new(|c, exclude_fonts, fontdb| {
            let base_font_id = exclude_fonts[0];

            // Iterate over fonts and check if any of them support the specified char.
            for face in fontdb.faces() {
                // Ignore fonts, that were used for shaping already.
                if exclude_fonts.contains(&face.id) {
                    continue;
                }

                // Check that the new face has the same style.
                let base_face = fontdb.face(base_font_id)?;
                if base_face.style != face.style
                    && base_face.weight != face.weight
                    && base_face.stretch != face.stretch
                {
                    continue;
                }

                if !fontdb.has_char(face.id, c) {
                    continue;
                }

                let base_family = base_face
                    .families
                    .iter()
                    .find(|f| f.1 == fontdb::Language::English_UnitedStates)
                    .unwrap_or(&base_face.families[0]);

                let new_family = face
                    .families
                    .iter()
                    .find(|f| f.1 == fontdb::Language::English_UnitedStates)
                    .unwrap_or(&base_face.families[0]);

                log::warn!("Fallback from {} to {}.", base_family.0, new_family.0);
                return Some(face.id);
            }

            None
        })
    }
}

impl std::fmt::Debug for FontResolver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FontResolver { .. }")
    }
}

/// Convert a text into its paths. This is done in two steps:
/// 1. We convert the text into glyphs and position them according to the rules specified
///    in the SVG specification. While doing so, we also calculate the text bbox (which
///    is not based on the outlines of a glyph, but instead the glyph metrics as well
///    as decoration spans).
/// 2. We convert all of the positioned glyphs into outlines.
pub(crate) fn convert(
    text: &mut Text,
    options: &crate::Options,
    cache: &mut Cache,
    probe_end_anchor: bool,
    no_primary_face: &mut bool,
) -> Option<Option<f32>> {
    let (layout, end) = layout::layout_text(
        text,
        options,
        &mut cache.fontdb,
        probe_end_anchor,
        no_primary_face,
    )?;
    text.layouted = layout.spans;
    text.bounding_box = layout.bbox.to_rect();
    text.abs_bounding_box = layout.bbox.transform(text.abs_transform)?.to_rect();

    let (group, stroke_bbox) =
        flatten::flatten(&text.id, text.rendering_mode, &text.layouted, cache)?;
    text.flattened = Box::new(group);
    text.stroke_bounding_box = stroke_bbox.to_rect();
    text.abs_stroke_bounding_box = stroke_bbox.transform(text.abs_transform)?.to_rect();

    let end_x = end.and_then(|end| {
        end.bbox.transform(text.abs_transform)?;
        // The legacy probe also required the second text to survive outline conversion.
        let (_, stroke_bbox) = flatten::flatten("", text.rendering_mode, &end.spans, cache)?;
        stroke_bbox.transform(text.abs_transform)?;
        Some(end.bbox.x())
    });
    Some(end_x)
}
