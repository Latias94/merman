//! Per-family Core Text matching; generic family resolution belongs to the caller.

use std::{collections::HashMap, ptr, sync::Arc};

use objc2_core_foundation::{
    CFArray, CFComparisonResult, CFDictionary, CFNumber, CFRetained, CFSet, CFString,
    CFStringCompareFlags, CFType, kCFTypeSetCallBacks,
};
use objc2_core_text::{
    CTFont, CTFontDescriptor, CTFontSymbolicTraits, CTFontTableOptions, kCTFontFamilyNameAttribute,
    kCTFontNameAttribute, kCTFontWeightTrait, kCTFontWidthTrait,
};
use usvg::fontdb::{
    Database, FaceInfo, Family, ID, Language, Query, Source, Stretch, Style, Weight,
};

use super::{NativeFace, font_covers};

// Core Text already selected one family. The ranking database only needs a shared key.
const QUERY_FAMILY: &str = "";

/// Matches one named family, keeping the selected native face alive for the caller.
/// Styles are preferences; a missing family must not select a substitute family.
pub(super) fn match_family(
    family: &str,
    weight: Weight,
    stretch: Stretch,
    style: Style,
    character: Option<char>,
) -> Option<NativeFace> {
    let mut candidates = Vec::new();
    let mut weight_groups = HashMap::new();
    for descriptor in family_descriptors(family)?.iter() {
        let Some(font) = font_from_descriptor(&descriptor) else {
            continue;
        };
        if character.is_some_and(|ch| !font_covers(&font, ch)) {
            continue;
        }
        let Some(face) = NativeFace::from_font(font) else {
            continue;
        };
        let (native_weight, native_stretch, native_style) = native_attributes(&face.font);
        let table = table_attributes(&face.font, native_stretch, native_style);
        if let (Some(attributes), Some(weight)) = (table, native_weight) {
            let (first, differs) = weight_groups.entry(attributes).or_insert((weight, false));
            *differs |= *first != weight;
        }
        let attributes = table.unwrap_or((
            native_weight.unwrap_or(Weight::NORMAL),
            native_stretch,
            native_style,
        ));
        candidates.push((face, attributes, native_weight));
    }

    let mut database = Database::new();
    // This database only ranks supported candidates. Its indices refer to retained faces.
    let source = Source::Binary(Arc::new(Vec::<u8>::new()));
    for (index, (_, attributes, native_weight)) in candidates.iter().enumerate() {
        let (mut weight, stretch, style) = *attributes;
        // Kaiti's Regular, Bold and Black all report OS/2 weight 400. Keep exact
        // table weights unless Core Text distinguishes faces in the same class.
        if weight_groups
            .get(attributes)
            .is_some_and(|(_, differs)| *differs)
        {
            weight = native_weight.unwrap_or(weight);
        }
        database.push_face_info(FaceInfo {
            id: ID::dummy(),
            source: source.clone(),
            index: u32::try_from(index).ok()?,
            families: vec![(QUERY_FAMILY.to_owned(), Language::English_UnitedStates)],
            post_script_name: String::new(),
            style,
            weight,
            stretch,
            monospaced: false,
        });
    }
    let id = database.query(&Query {
        families: &[Family::Name(QUERY_FAMILY)],
        weight,
        stretch,
        style,
    })?;
    let index = usize::try_from(database.face(id)?.index).ok()?;
    candidates.into_iter().nth(index).map(|(face, ..)| face)
}

/// Core Text can return an instance whose coordinates the snapshot cannot represent.
/// Keep its position in the cascade by trying supported faces of its actual family.
pub(super) fn fallback_face(font: CFRetained<CTFont>, character: char) -> Option<NativeFace> {
    if !font_covers(&font, character) {
        return None;
    }
    if let Some(face) = NativeFace::from_font(font.clone()) {
        return Some(face);
    }
    // SAFETY: The retained font owns its family identity independently of the descriptor.
    let family = unsafe { font.family_name() }.to_string();
    if super::is_last_resort(&family) {
        return None;
    }
    let (weight, stretch, style) = native_attributes(&font);
    match_family(
        &family,
        weight.unwrap_or(Weight::NORMAL),
        stretch,
        style,
        Some(character),
    )
}

fn family_descriptors(family: &str) -> Option<CFRetained<CFArray<CTFontDescriptor>>> {
    if family.is_empty() {
        return None;
    }
    let family_name = CFString::from_str(family);
    // SAFETY: This is an immutable Core Text CFString constant.
    let family_key = unsafe { kCTFontFamilyNameAttribute };
    let attributes =
        CFDictionary::<CFString, CFString>::from_slices(&[family_key], &[&family_name]);
    // SAFETY: The key and its retained CFString value are valid descriptor attributes.
    let request = unsafe { CTFontDescriptor::with_attributes(attributes.as_opaque()) };

    let mut required_keys = [ptr::from_ref(family_key).cast()];
    // SAFETY: The array contains one live CFString pointer. The standard CFType
    // callbacks retain it and compare by value; None selects the default allocator.
    // objc2-core-foundation 0.3.2 has no safe CFSet slice constructor.
    let required =
        unsafe { CFSet::new(None, required_keys.as_mut_ptr(), 1, &kCFTypeSetCallBacks) }?;
    // SAFETY: The set contains only the family attribute key. Enumerate this family
    // without native style preferences, which do not implement CSS weight ordering.
    let descriptors = unsafe { request.matching_font_descriptors(Some(&required)) }?;
    // SAFETY: Core Text returns an immutable CFArray of retained CTFontDescriptor objects.
    Some(unsafe { CFRetained::cast_unchecked(descriptors) })
}

pub(super) fn font_from_descriptor(descriptor: &CTFontDescriptor) -> Option<CFRetained<CTFont>> {
    // Compare the normalized descriptor's identity, not the input spelling:
    // Core Text owns case folding and recognition of localized names/aliases.
    // SAFETY: The descriptor is live; this accessor returns a retained CF object.
    let expected_name = unsafe { descriptor.attribute(kCTFontNameAttribute) }?
        .downcast::<CFString>()
        .ok()?;
    if expected_name.length() == 0 {
        return None;
    }
    // SAFETY: The matched descriptor is live; zero size and null matrix use its
    // defaults. The returned CTFont is retained independently of the descriptor.
    let font = unsafe { CTFont::with_font_descriptor(descriptor, 0.0, ptr::null()) };
    // SAFETY: The font is live and the returned name is retained.
    let actual_name = unsafe { font.post_script_name() };
    if expected_name.compare(
        Some(&actual_name),
        CFStringCompareFlags::CompareCaseInsensitive,
    ) != CFComparisonResult::CompareEqualTo
    {
        // Creation can substitute a font if the matched face becomes unavailable.
        return None;
    }
    Some(font)
}

fn table_attributes(
    font: &CTFont,
    native_stretch: Stretch,
    native_style: Style,
) -> Option<(Weight, Stretch, Style)> {
    // Only supported default instances reach here, so OS/2 describes the physical face.
    // SAFETY: The live font returns immutable retained table data.
    let data = unsafe { font.table(u32::from_be_bytes(*b"OS/2"), CTFontTableOptions::NoOptions) }?;
    // SAFETY: The retained CFData outlives the borrowed table parser.
    let table = ttf_parser::os2::Table::parse(unsafe { data.as_bytes_unchecked() })?;
    // Some families mark every face as normal width in OS/2, including the
    // condensed Sukhumvit Set Thin. Preserve an explicit non-normal table width.
    let stretch = match table.width() {
        Stretch::Normal => native_stretch,
        stretch => stretch,
    };
    let style = match table.style() {
        // Some legacy fonts report italic only through native/post traits.
        ttf_parser::Style::Normal => native_style,
        ttf_parser::Style::Italic => Style::Italic,
        ttf_parser::Style::Oblique => Style::Oblique,
    };
    Some((Weight(table.weight().to_number()), stretch, style))
}

fn native_attributes(font: &CTFont) -> (Option<Weight>, Stretch, Style) {
    // SAFETY: The font is retained and these accessors do not mutate it.
    let native_italic =
        unsafe { font.symbolic_traits() }.contains(CTFontSymbolicTraits::TraitItalic);
    let native_style = if native_italic {
        Style::Italic
    } else {
        Style::Normal
    };
    // SAFETY: Core Text's immutable traits dictionary has CFString keys and CFType values.
    let traits: CFRetained<CFDictionary<CFString, CFType>> =
        unsafe { CFRetained::cast_unchecked(font.traits()) };
    let number = |key| {
        traits
            .get(key)?
            .downcast::<CFNumber>()
            .ok()?
            .as_f64()
            .filter(|value| value.is_finite())
    };
    // SAFETY: These are immutable Core Text trait keys.
    let (weight, width) = unsafe { (number(kCTFontWeightTrait), number(kCTFontWidthTrait)) };
    // Invert SkTypeface_mac_ct.cpp::SkCTFontCTWidthForCSSWidth's OS/2 class scale.
    let stretch = match (width.unwrap_or(0.0) * 10.0 + 5.0).round().clamp(1.0, 9.0) as u8 {
        1 => Stretch::UltraCondensed,
        2 => Stretch::ExtraCondensed,
        3 => Stretch::Condensed,
        4 => Stretch::SemiCondensed,
        6 => Stretch::SemiExpanded,
        7 => Stretch::Expanded,
        8 => Stretch::ExtraExpanded,
        9 => Stretch::UltraExpanded,
        _ => Stretch::Normal,
    };
    (weight.map(css_weight), stretch, native_style)
}

fn css_weight(weight: f64) -> Weight {
    // Skia 026cef465735b4dfb51bfd64b748017e902d2a67:
    // Invert SkCTFontGetNSFontWeightMapping's defaults for missing or ambiguous
    // OS/2 weights. This converts units; fontdb owns CSS ranking.
    const WEIGHTS: [f64; 11] = [
        -1.00, -0.80, -0.60, -0.40, 0.00, 0.23, 0.30, 0.40, 0.56, 0.62, 1.00,
    ];
    for (index, pair) in WEIGHTS.windows(2).enumerate() {
        if weight <= pair[1] {
            let fraction = ((weight - pair[0]) / (pair[1] - pair[0])).clamp(0.0, 1.0);
            return Weight(((index as f64 + fraction) * 100.0).round().max(1.0) as u16);
        }
    }
    Weight(1000)
}

#[cfg(test)]
mod tests {
    use super::super::named;
    use super::*;

    #[test]
    #[ignore = "requires macOS with registered Arial Regular, Bold, Italic and Bold Italic faces"]
    fn arial_missing_weights_follow_css_order_for_roman_and_italic() {
        for (style, regular, bold) in [
            (Style::Normal, "ArialMT", "Arial-BoldMT"),
            (Style::Italic, "Arial-ItalicMT", "Arial-BoldItalicMT"),
        ] {
            for (weight, expected) in [(400, regular), (500, regular), (600, bold), (700, bold)] {
                let font = named("Arial", Weight(weight), Stretch::Normal, style)
                    .expect("registered Arial family");
                assert_eq!(
                    font.post_script_name, expected,
                    "Arial CSS weight {weight}, style {style:?}"
                );
            }
        }

        let mixed_case = named("aRiAl", Weight::MEDIUM, Stretch::Normal, Style::Italic)
            .expect("family matching accepts case variants");
        assert_eq!(mixed_case.post_script_name, "Arial-ItalicMT");
    }

    #[test]
    #[ignore = "requires the macOS Core Text font registry"]
    fn unknown_family_does_not_select_a_substitute() {
        assert!(
            named(
                "MermanMissingFamily-46da48fb3f884be4",
                Weight::NORMAL,
                Stretch::Normal,
                Style::Normal,
            )
            .is_none()
        );
    }

    #[test]
    #[ignore = "optional qualification with the system STIX Two Text variable fonts installed"]
    fn installed_stix_family_keeps_its_physical_default_faces() {
        let root = std::path::Path::new("/System/Library/Fonts/Supplemental");
        for (style, file) in [
            (Style::Normal, "STIXTwoText.ttf"),
            (Style::Italic, "STIXTwoText-Italic.ttf"),
        ] {
            let path = root.join(file);
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let physical = ttf_parser::Face::parse(&bytes, 0).unwrap();
            let Some(axis) = physical
                .variation_axes()
                .into_iter()
                .find(|axis| axis.tag == ttf_parser::Tag::from_bytes(b"wght"))
            else {
                continue;
            };
            let expected = physical
                .names()
                .into_iter()
                .filter(|name| name.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
                .find_map(|name| name.to_string())
                .expect("the physical STIX face has a PostScript name");
            for weight in [Weight::NORMAL, Weight::MEDIUM, Weight::BOLD] {
                let face = named("STIX Two Text", weight, Stretch::Normal, style)
                    .expect("the installed variable face is usable");
                assert_eq!(face.path, path);
                assert_eq!(face.post_script_name, expected, "{weight:?} {style:?}");
            }

            let base = named("STIX Two Text", Weight::NORMAL, Stretch::Normal, style).unwrap();
            let non_default = if axis.max_value > axis.def_value {
                axis.max_value
            } else {
                axis.min_value
            };
            let axis_id = CFNumber::new_i32(i32::from_be_bytes(*b"wght"));
            // SAFETY: The live descriptor and axis identifier describe this variable font.
            let instance = unsafe {
                let descriptor = base
                    .font
                    .font_descriptor()
                    .copy_with_variation(&axis_id, non_default.into());
                CTFont::with_font_descriptor(&descriptor, 0.0, ptr::null())
            };
            // SAFETY: Core Text returns the live font's non-default coordinates.
            assert!(unsafe { instance.variation() }.is_some_and(|values| !values.is_empty()));
            let fallback = fallback_face(instance, 'A').expect("the default STIX face covers A");
            assert_eq!(fallback.path, path);
            assert_eq!(fallback.post_script_name, expected);
        }
    }

    #[test]
    #[ignore = "optional qualification with the registered Kaiti SC collection installed"]
    fn installed_kaiti_faces_with_shared_table_weight_follow_native_traits() {
        let Some(descriptors) = family_descriptors("Kaiti SC") else {
            return;
        };
        let mut weights = HashMap::new();
        for descriptor in descriptors.iter() {
            let Some(face) = font_from_descriptor(&descriptor).and_then(NativeFace::from_font)
            else {
                continue;
            };
            if !face.path.is_file() {
                continue;
            }
            let (Some(weight), stretch, Style::Normal) = native_attributes(&face.font) else {
                continue;
            };
            if table_attributes(&face.font, stretch, Style::Normal)
                == Some((Weight::NORMAL, Stretch::Normal, Style::Normal))
            {
                weights.entry(weight).or_insert_with(Vec::new).push(face);
            }
        }
        if weights.len() < 2 {
            return;
        }
        for (weight, expected) in weights {
            let face = named("Kaiti SC", weight, Stretch::Normal, Style::Normal)
                .expect("the installed Kaiti face is usable");
            assert!(
                expected.iter().any(|candidate| candidate.path == face.path
                    && candidate.post_script_name == face.post_script_name),
                "Kaiti SC {weight:?} selected {}",
                face.post_script_name
            );
        }
    }

    #[test]
    #[ignore = "optional qualification with the registered Sukhumvit Set collection"]
    fn condensed_native_face_is_ranked_by_width_before_weight() {
        let Some(descriptors) = family_descriptors("Sukhumvit Set") else {
            return;
        };
        if !descriptors
            .iter()
            .filter_map(|desc| font_from_descriptor(&desc))
            .any(|font| native_attributes(&font).1 == Stretch::Condensed)
        {
            return;
        }
        for (stretch, weight) in [
            (Stretch::Condensed, Weight::NORMAL),
            (Stretch::Normal, Weight(200)),
        ] {
            let face = named("Sukhumvit Set", weight, stretch, Style::Normal).unwrap();
            assert_eq!(
                native_attributes(&face.font).1,
                stretch,
                "{}",
                face.post_script_name
            );
        }
    }

    #[cfg(feature = "png")]
    #[test]
    #[ignore = "requires macOS Arial and system Ethiopic coverage"]
    fn bold_arial_paints_ethiopic_with_strict_coverage() {
        use crate::{NativeFontOptions, RasterOptions, svg_to_png_with_report};
        use merman_model::OperationControl;
        use merman_render::environment::RenderEnvironment;

        let fonts = NativeFontOptions::default()
            .with_system_fonts(true)
            .load_platform(&OperationControl::new())
            .unwrap()
            .with_strict_glyph_coverage(true);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><text x="4" y="48" font-family="Arial" font-size="32" font-weight="700">ሀ</text></svg>"#;
        let svg = merman_render::svg::finalize_resvg_svg(source, &session).unwrap();
        let options = RasterOptions::default().with_font_context(fonts);
        let (png, report) =
            svg_to_png_with_report(&svg, &options).expect("strict export must cover U+1200");
        assert!(report.fonts().used_system_fonts());
        assert_eq!(report.fonts().notdef_glyph_count(), 0);
        let pixels = tiny_skia::Pixmap::decode_png(&png).unwrap();
        assert!(pixels.pixels().iter().any(|pixel| pixel.alpha() > 0));
    }
}
