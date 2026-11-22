//! Per-family Core Text matching; generic family resolution belongs to the caller.

use std::{ptr, sync::Arc};

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

/// Matches one named family, keeping the selected native face alive for the caller.
/// Styles are preferences; a missing family must not select a substitute family.
pub(super) fn match_family(
    family: &str,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<CFRetained<CTFont>> {
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
    let descriptors: CFRetained<CFArray<CTFontDescriptor>> =
        unsafe { CFRetained::cast_unchecked(descriptors) };

    let mut database = Database::new();
    // Only query metadata is used. This empty source never reaches admission or shaping;
    // each index identifies a descriptor array row, not a physical TTC member.
    let source = Source::Binary(Arc::new(Vec::<u8>::new()));
    for (index, descriptor) in descriptors.iter().enumerate() {
        let Some(font) = font_from_descriptor(&descriptor) else {
            continue;
        };
        let (weight, stretch, style) = font_attributes(&font);
        database.push_face_info(FaceInfo {
            id: ID::dummy(),
            source: source.clone(),
            index: u32::try_from(index).ok()?,
            // Core Text has already resolved case/localized aliases. Use the authored
            // spelling as a query-local grouping key, not as native identity evidence.
            families: vec![(family.to_owned(), Language::English_UnitedStates)],
            post_script_name: String::new(),
            style,
            weight,
            stretch,
            monospaced: false,
        });
    }
    let id = database.query(&Query {
        families: &[Family::Name(family)],
        weight,
        stretch,
        style,
    })?;
    let index = usize::try_from(database.face(id)?.index).ok()?;
    let descriptor = descriptors.get(index)?;
    font_from_descriptor(&descriptor)
}

fn font_from_descriptor(descriptor: &CTFontDescriptor) -> Option<CFRetained<CTFont>> {
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

fn font_attributes(font: &CTFont) -> (Weight, Stretch, Style) {
    // SAFETY: The font is retained and these accessors do not mutate it.
    let native_italic =
        unsafe { font.symbolic_traits() }.contains(CTFontSymbolicTraits::TraitItalic);
    let native_style = if native_italic {
        Style::Italic
    } else {
        Style::Normal
    };
    // A named variable instance's OS/2 table describes the default physical face.
    // Keep its native traits for ranking; NativeFace still rejects non-default instances.
    // SAFETY: Core Text returns an immutable retained dictionary for this live font.
    let default_instance = unsafe { font.variation() }.is_none_or(|values| values.is_empty());
    if default_instance {
        // SAFETY: The live font returns immutable retained table data. Only this
        // candidate's OS/2 table is requested, never its file or unrelated families.
        let data =
            unsafe { font.table(u32::from_be_bytes(*b"OS/2"), CTFontTableOptions::NoOptions) };
        if let Some(data) = data
            // SAFETY: The retained immutable CFData outlives the borrowed table parser.
            && let Some(table) =
                ttf_parser::os2::Table::parse(unsafe { data.as_bytes_unchecked() })
        {
            let style = match table.style() {
                // Some legacy fonts report italic only through native/post traits.
                ttf_parser::Style::Normal => native_style,
                ttf_parser::Style::Italic => Style::Italic,
                ttf_parser::Style::Oblique => Style::Oblique,
            };
            return (Weight(table.weight().to_number()), table.width(), style);
        }
    }

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
    (css_weight(weight.unwrap_or(0.0)), stretch, native_style)
}

fn css_weight(weight: f64) -> Weight {
    // Skia 026cef465735b4dfb51bfd64b748017e902d2a67:
    // Invert SkCTFontGetNSFontWeightMapping's defaults only when the native instance
    // cannot use an exact OS/2 weight. This converts units; fontdb owns CSS ranking.
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
    use super::*;

    #[test]
    #[ignore = "requires macOS with registered Arial Regular, Bold, Italic and Bold Italic faces"]
    fn arial_missing_weights_follow_css_order_for_roman_and_italic() {
        for (style, regular, bold) in [
            (Style::Normal, "ArialMT", "Arial-BoldMT"),
            (Style::Italic, "Arial-ItalicMT", "Arial-BoldItalicMT"),
        ] {
            for (weight, expected) in [(400, regular), (500, regular), (600, bold), (700, bold)] {
                let font = match_family("Arial", Weight(weight), Stretch::Normal, style)
                    .expect("registered Arial family");
                // SAFETY: The font is live and the name is retained.
                assert_eq!(
                    unsafe { font.post_script_name() }.to_string(),
                    expected,
                    "Arial CSS weight {weight}, style {style:?}"
                );
            }
        }

        let mixed_case = match_family("aRiAl", Weight::MEDIUM, Stretch::Normal, Style::Italic)
            .expect("family matching accepts case variants");
        // SAFETY: The font is live and the name is retained.
        assert_eq!(
            unsafe { mixed_case.post_script_name() }.to_string(),
            "Arial-ItalicMT"
        );
    }

    #[test]
    #[ignore = "requires the macOS Core Text font registry"]
    fn unknown_family_does_not_select_a_substitute() {
        assert!(
            match_family(
                "MermanMissingFamily-46da48fb3f884be4",
                Weight::NORMAL,
                Stretch::Normal,
                Style::Normal,
            )
            .is_none()
        );
    }
}
