//! Per-family Core Text matching; generic family resolution belongs to the caller.

use std::{collections::HashMap, ptr};

use objc2_core_foundation::{
    CFArray, CFBoolean, CFComparisonResult, CFDictionary, CFNumber, CFRetained, CFString,
    CFStringCompareFlags, CFType,
};
use objc2_core_text::{
    CTFont, CTFontCollection, CTFontDescriptor, CTFontSymbolicTraits, CTFontUIFontType,
    kCTFontCollectionDisallowAutoActivationOption, kCTFontDownloadableAttribute,
    kCTFontDownloadedAttribute, kCTFontFamilyNameAttribute, kCTFontNameAttribute,
    kCTFontSymbolicTrait, kCTFontTraitsAttribute, kCTFontWeightTrait, kCTFontWidthTrait,
};
use usvg::fontdb::{Stretch, Style, Weight};

use super::super::{css_width, ranked_axis_faces, ranked_faces};
use super::variations::MatchingAxes;
use super::{
    LOCAL_FONT_OPTIONS, NAME, NativeFace, NativeFontError, OperationControl, checkpoint,
    read_table, table_bytes,
};

type CssStyle = (Weight, Stretch, Style);
type NativeStyle = (Option<Weight>, Stretch, Style);

#[derive(Clone, Copy)]
struct FaceAttributes {
    table: Option<CssStyle>,
    native: NativeStyle,
    axes: MatchingAxes,
}

/// Offers the local family in CSS order, stopping when the caller accepts a face.
pub(super) fn match_family<T>(
    family: &str,
    style: CssStyle,
    control: Option<&OperationControl>,
    mut accept: impl FnMut(NativeFace) -> Result<Option<T>, NativeFontError>,
) -> Result<Option<T>, NativeFontError> {
    checkpoint(control)?;
    let descriptors = family_descriptors(family);
    checkpoint(control)?;
    let Some(descriptors) = descriptors else {
        return Ok(None);
    };
    let mut candidates = Vec::new();
    for descriptor in descriptors.iter() {
        checkpoint(control)?;
        let Some(font) = font_from_descriptor(&descriptor, control)? else {
            continue;
        };
        let native = descriptor_attributes(&descriptor).unwrap_or_else(|| native_attributes(&font));
        let table = table_attributes(&font, native.1, native.2, control)?;
        candidates.push((
            (font, descriptor),
            FaceAttributes {
                table,
                native,
                axes: MatchingAxes::default(),
            },
        ));
    }
    if candidates.len() > 1 {
        let (weight, stretch, _) = style;
        for ((font, _), attributes) in &mut candidates {
            attributes.axes =
                MatchingAxes::for_font(font, weight.0.into(), css_width(stretch), control)?;
        }
    }
    rank_family(
        candidates,
        style,
        |(font, descriptor)| match NativeFace::from_font(font, Some(&descriptor), control)? {
            Some(face) => accept(face),
            None => Ok(None),
        },
    )
}

fn rank_family<T, R>(
    candidates: Vec<(T, FaceAttributes)>,
    (weight, stretch, style): CssStyle,
    mut accept: impl FnMut(T) -> Result<Option<R>, NativeFontError>,
) -> Result<Option<R>, NativeFontError> {
    let mut weight_groups = HashMap::new();
    for (_, attributes) in &candidates {
        if let (Some(attributes), Some(weight)) = (attributes.table, attributes.native.0) {
            let (first, differs) = weight_groups.entry(attributes).or_insert((weight, false));
            *differs |= *first != weight;
        }
    }

    let has_axes = candidates
        .iter()
        .any(|(_, attributes)| attributes.axes.weight.is_some() || attributes.axes.width.is_some());
    let candidates = candidates
        .into_iter()
        .map(|(candidate, attributes)| {
            let (native_weight, native_stretch, native_style) = attributes.native;
            let css = attributes.table.unwrap_or((
                native_weight.unwrap_or(Weight::NORMAL),
                native_stretch,
                native_style,
            ));
            let (mut weight, stretch, style) = css;
            // Kaiti's Regular, Bold and Black all report OS/2 weight 400. Keep exact
            // table weights unless Core Text distinguishes faces in the same class.
            if weight_groups.get(&css).is_some_and(|(_, differs)| *differs) {
                weight = native_weight.unwrap_or(weight);
            }
            let attainable = (
                candidate,
                attributes.axes.weight.unwrap_or(weight.0.into()),
                attributes.axes.width.unwrap_or_else(|| css_width(stretch)),
                style,
            );
            (attainable, weight, stretch, style)
        })
        .collect();
    let ordered = ranked_faces(candidates, weight, stretch, style);
    // Prefer the existing exact style (including named instances) on a range tie.
    let ordered = if has_axes {
        ranked_axis_faces(ordered, weight.0.into(), css_width(stretch), style)
    } else {
        ordered
            .into_iter()
            .map(|(candidate, ..)| candidate)
            .collect()
    };
    for candidate in ordered {
        if let Some(selected) = accept(candidate)? {
            return Ok(Some(selected));
        }
    }
    Ok(None)
}

/// A different CSS style may be available in the native choice's public family.
pub(super) fn fallback_family(font: &CTFont, style: CssStyle, unsupported: bool) -> Option<String> {
    let (weight, stretch, slant) = native_attributes(font);
    if !unsupported
        && weight.is_none_or(|weight| weight == style.0)
        && (stretch, slant) == (style.1, style.2)
    {
        return None;
    }
    // SAFETY: The retained font owns its family identity independently of the descriptor.
    let family = unsafe { font.family_name() }.to_string();
    (!family.is_empty() && !family.starts_with('.') && !super::is_last_resort(&family))
        .then_some(family)
}

pub(super) fn system_ui(
    (weight, stretch, style): CssStyle,
    language: Option<&str>,
    control: Option<&OperationControl>,
) -> Result<Option<NativeFace>, NativeFontError> {
    let ui_type = if weight >= Weight::SEMIBOLD {
        CTFontUIFontType::EmphasizedSystem
    } else {
        CTFontUIFontType::System
    };
    let language = language.map(CFString::from_str);
    checkpoint(control)?;
    // SAFETY: The language string, when provided, is retained for the call.
    let Some(font) =
        (unsafe { CTFont::new_ui_font_for_language(ui_type, 0.0, language.as_deref()) })
    else {
        return Ok(None);
    };
    let mut traits = match stretch {
        Stretch::UltraCondensed
        | Stretch::ExtraCondensed
        | Stretch::Condensed
        | Stretch::SemiCondensed => CTFontSymbolicTraits::TraitCondensed,
        Stretch::SemiExpanded
        | Stretch::Expanded
        | Stretch::ExtraExpanded
        | Stretch::UltraExpanded => CTFontSymbolicTraits::TraitExpanded,
        Stretch::Normal => CTFontSymbolicTraits::empty(),
    };
    if style != Style::Normal {
        traits |= CTFontSymbolicTraits::TraitItalic;
    }
    let styled = if traits.is_empty() {
        None
    } else {
        checkpoint(control)?;
        // SAFETY: The font is live and a null matrix preserves its transform. Core
        // Text looks within this localized UI font's family for the requested traits.
        unsafe { font.copy_with_symbolic_traits(0.0, ptr::null(), traits, traits) }
    };
    for font in styled.into_iter().chain([font]) {
        let Some(face) = NativeFace::from_font(font, None, control)? else {
            continue;
        };
        let eligible = match control {
            Some(control) => face.check_eligibility(None, true, control)?,
            None => face.implicit_eligible(None),
        };
        if eligible {
            return Ok(Some(face));
        }
    }
    Ok(None)
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

    // Match the family without letting Core Text search for missing fonts.
    // SAFETY: This is an immutable Core Text CFString constant.
    let local_only_key = unsafe { kCTFontCollectionDisallowAutoActivationOption };
    let enabled = CFNumber::new_i32(1);
    let options = CFDictionary::from_slices(&[local_only_key], &[&*enabled]);
    let requests = CFArray::from_objects(&[&*request]);
    // SAFETY: The array contains retained descriptors and the option is a CFNumber.
    let descriptors = unsafe {
        let collection = CTFontCollection::with_font_descriptors(
            Some(requests.as_opaque()),
            Some(options.as_opaque()),
        );
        collection.matching_font_descriptors()
    }?;
    // SAFETY: Core Text returns an immutable CFArray of retained CTFontDescriptor objects.
    Some(unsafe { CFRetained::cast_unchecked(descriptors) })
}

pub(super) fn descriptor_is_available(descriptor: &CTFontDescriptor) -> bool {
    let flag = |key| {
        // SAFETY: The descriptor and attribute key are live.
        unsafe { descriptor.attribute(key) }
            .and_then(|value| value.downcast::<CFBoolean>().ok())
            .is_some_and(|value| value.value())
    };
    // SAFETY: These keys are immutable Core Text constants.
    unsafe { !flag(kCTFontDownloadableAttribute) || flag(kCTFontDownloadedAttribute) }
}

pub(super) fn font_from_descriptor(
    descriptor: &CTFontDescriptor,
    control: Option<&OperationControl>,
) -> Result<Option<CFRetained<CTFont>>, NativeFontError> {
    checkpoint(control)?;
    if !descriptor_is_available(descriptor) {
        return Ok(None);
    }

    // Compare the normalized descriptor's identity, not the input spelling:
    // Core Text owns case folding and recognition of localized names/aliases.
    // SAFETY: The descriptor is live; this accessor returns a retained CF object.
    let Some(expected_name) = (unsafe { descriptor.attribute(kCTFontNameAttribute) })
        .and_then(|name| name.downcast::<CFString>().ok())
        .filter(|name| name.length() > 0)
    else {
        return Ok(None);
    };
    // SAFETY: The matched descriptor is live; zero size and null matrix use its
    // defaults. The returned CTFont is retained independently of the descriptor.
    let font = unsafe {
        CTFont::with_font_descriptor_and_options(descriptor, 0.0, ptr::null(), LOCAL_FONT_OPTIONS)
    };
    // SAFETY: The font is live and the returned name is retained.
    let actual_name = unsafe { font.post_script_name() };
    if expected_name.compare(
        Some(&actual_name),
        CFStringCompareFlags::CompareCaseInsensitive,
    ) != CFComparisonResult::CompareEqualTo
    {
        // A variation can change the reported name. Require the descriptor's
        // name in the physical SFNT to distinguish it from a substituted face.
        let name = read_table(&font, NAME, control)?;
        if !name
            .as_deref()
            .map(table_bytes)
            .and_then(ttf_parser::name::Table::parse)
            .is_some_and(|names| {
                super::super::post_script_name_matches(names, &expected_name.to_string())
            })
        {
            return Ok(None);
        }
    }
    Ok(Some(font))
}

fn table_attributes(
    font: &CTFont,
    native_stretch: Stretch,
    native_style: Style,
    control: Option<&OperationControl>,
) -> Result<Option<CssStyle>, NativeFontError> {
    let data = read_table(font, u32::from_be_bytes(*b"OS/2"), control)?;
    let Some(table) = data
        .as_deref()
        .map(table_bytes)
        .and_then(ttf_parser::os2::Table::parse)
    else {
        return Ok(None);
    };
    // Some families mark every face as normal width in OS/2, including the
    // condensed Sukhumvit Set Thin. Preserve an explicit non-normal table width.
    let stretch = match table.width() {
        ttf_parser::Width::Normal => native_stretch,
        width => stretch_from_width_class(width.to_number()),
    };
    let style = match table.style() {
        // Some legacy fonts report italic only through native/post traits.
        ttf_parser::Style::Normal => native_style,
        ttf_parser::Style::Italic => Style::Italic,
        ttf_parser::Style::Oblique => Style::Oblique,
    };
    Ok(Some((Weight(table.weight().to_number()), stretch, style)))
}

fn descriptor_attributes(descriptor: &CTFontDescriptor) -> Option<NativeStyle> {
    // SAFETY: The descriptor is live and the attribute key is immutable.
    let traits = unsafe { descriptor.attribute(kCTFontTraitsAttribute) }?
        .downcast::<CFDictionary>()
        .ok()?;
    // SAFETY: A descriptor's traits use CFString keys and CF object values.
    let traits: CFRetained<CFDictionary<CFString, CFType>> =
        unsafe { CFRetained::cast_unchecked(traits) };
    // SAFETY: This is an immutable Core Text key for the symbolic trait bitset.
    let symbolic = traits
        .get(unsafe { kCTFontSymbolicTrait })?
        .downcast::<CFNumber>()
        .ok()?
        .as_i64()?;
    Some(trait_attributes(
        &traits,
        CTFontSymbolicTraits::from_bits_retain(symbolic as u32),
    ))
}

fn native_attributes(font: &CTFont) -> NativeStyle {
    // SAFETY: The font is retained and these accessors do not mutate it.
    let symbolic = unsafe { font.symbolic_traits() };
    // SAFETY: Core Text's immutable traits dictionary has CFString keys and CFType values.
    let traits: CFRetained<CFDictionary<CFString, CFType>> =
        unsafe { CFRetained::cast_unchecked(font.traits()) };
    trait_attributes(&traits, symbolic)
}

fn trait_attributes(
    traits: &CFDictionary<CFString, CFType>,
    symbolic: CTFontSymbolicTraits,
) -> NativeStyle {
    let style = if symbolic.contains(CTFontSymbolicTraits::TraitItalic) {
        Style::Italic
    } else {
        Style::Normal
    };
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
    let width = (width.unwrap_or(0.0) * 10.0 + 5.0).round().clamp(1.0, 9.0) as u16;
    (
        weight.map(css_weight),
        stretch_from_width_class(width),
        style,
    )
}

fn stretch_from_width_class(width: u16) -> Stretch {
    match width {
        1 => Stretch::UltraCondensed,
        2 => Stretch::ExtraCondensed,
        3 => Stretch::Condensed,
        4 => Stretch::SemiCondensed,
        6 => Stretch::SemiExpanded,
        7 => Stretch::Expanded,
        8 => Stretch::ExtraExpanded,
        9 => Stretch::UltraExpanded,
        _ => Stretch::Normal,
    }
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
    #[ignore = "requires the macOS system UI font catalog"]
    fn system_ui_matches_core_text_for_each_requested_language() {
        for tag in ["en", "zh-Hans", "zh-Hant", "ja", "ko"] {
            let language = CFString::from_str(tag);
            // SAFETY: The language is live and zero requests the system UI point size.
            let font = unsafe {
                CTFont::new_ui_font_for_language(CTFontUIFontType::System, 0.0, Some(&language))
            }
            .unwrap();
            let expected = NativeFace::from_font(font, None, None)
                .unwrap()
                .filter(|face| face.implicit_eligible(None));
            let actual = system_ui(
                (Weight::NORMAL, Stretch::Normal, Style::Normal),
                Some(tag),
                None,
            )
            .unwrap();
            assert_eq!(actual, expected, "{tag}");
            if tag == "en" {
                assert!(actual.unwrap().covers_cluster("A".into()));
            }
        }
    }

    #[test]
    #[ignore = "requires a registered macOS Arial face"]
    fn descriptor_matching_accepts_only_local_font_assets() {
        let face = named("Arial", Weight::NORMAL, Stretch::Normal, Style::Normal)
            .expect("registered Arial face");
        // SAFETY: The live font returns its retained descriptor.
        let descriptor = unsafe { face.font.font_descriptor() };
        assert!(font_from_descriptor(&descriptor, None).unwrap().is_some());
        // SAFETY: These keys are immutable Core Text constants.
        let keys = unsafe { [kCTFontDownloadableAttribute, kCTFontDownloadedAttribute] };
        for downloaded in [true, false] {
            let attributes = CFDictionary::from_slices(
                &keys,
                &[CFBoolean::new(true), CFBoolean::new(downloaded)],
            );
            // SAFETY: The descriptor and its new boolean attributes stay live through creation.
            let asset = unsafe { descriptor.copy_with_attributes(attributes.as_opaque()) };
            assert_eq!(
                font_from_descriptor(&asset, None).unwrap().is_some(),
                downloaded,
                "asset downloaded: {downloaded}"
            );
        }
    }

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
    fn installed_stix_family_keeps_physical_identity_across_instances() {
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
                assert_eq!(face.path.as_deref(), Some(path.as_path()));
                assert_eq!(face.post_script_name, expected, "{weight:?} {style:?}");
            }

            let base = named("STIX Two Text", Weight::NORMAL, Stretch::Normal, style).unwrap();
            let non_default = if axis.max_value > axis.def_value {
                axis.max_value
            } else {
                axis.min_value
            };
            let axis_id = CFNumber::new_i32(i32::from_be_bytes(*b"wght"));
            for value in [axis.def_value, non_default] {
                // SAFETY: The live descriptor and axis identifier describe this variable font.
                let descriptor = unsafe {
                    base.font
                        .font_descriptor()
                        .copy_with_variation(&axis_id, value.into())
                };
                let instance = font_from_descriptor(&descriptor, None).unwrap().unwrap();
                let instance = NativeFace::from_font(instance, Some(&descriptor), None)
                    .unwrap()
                    .expect("the variable STIX instance is supported");
                assert!(instance.covers_cluster("A".into()));
                assert_eq!(instance.path.as_deref(), Some(path.as_path()));
                assert_eq!(instance.post_script_name, expected);
                // A descriptor copied from an installed font may omit a no-op axis change.
                // The effective coordinate then comes from the physical face's default.
                assert_eq!(
                    instance
                        .variations
                        .iter()
                        .find(|coord| coord.tag == *b"wght")
                        .map_or(axis.def_value, |coord| coord.value),
                    value
                );
                assert!(
                    instance
                        .matches_data(&bytes, 0, &OperationControl::new())
                        .unwrap()
                );
            }
        }
    }

    #[test]
    fn ambiguous_table_weights_follow_native_traits() {
        let candidates =
            [("regular", 0.0), ("bold", 0.4), ("black", 0.62)].map(|(name, native_weight)| {
                let stretch = Stretch::Normal;
                let style = Style::Normal;
                let table = Some((Weight::NORMAL, stretch, style));
                let native = (Some(css_weight(native_weight)), stretch, style);
                (
                    name,
                    FaceAttributes {
                        table,
                        native,
                        axes: MatchingAxes::default(),
                    },
                )
            });

        for (weight, expected) in [
            (400, "regular"),
            (500, "regular"),
            (700, "bold"),
            (900, "black"),
        ] {
            assert_eq!(
                rank_family(
                    candidates.to_vec(),
                    (Weight(weight), Stretch::Normal, Style::Normal),
                    |candidate| Ok(Some(candidate)),
                )
                .unwrap(),
                Some(expected),
                "CSS weight {weight}"
            );
        }

        let mut tried = Vec::new();
        rank_family(
            candidates.to_vec(),
            (Weight::BOLD, Stretch::Normal, Style::Normal),
            |candidate| {
                tried.push(candidate);
                Ok(None::<()>)
            },
        )
        .unwrap();
        assert_eq!(tried, ["bold", "black", "regular"]);
    }

    #[test]
    fn family_ranks_attainable_axes_without_overriding_style_or_named_instances() {
        let raw = ttf_parser::RawFace::parse(
            include_bytes!("../../../../tests/fonts/MermanVariableWeight.ttf"),
            0,
        )
        .unwrap();
        let weight_table = raw.table(ttf_parser::Tag::from_bytes(b"fvar")).unwrap();
        let mut width_table = weight_table.to_vec();
        let offset = usize::from(u16::from_be_bytes(width_table[4..6].try_into().unwrap()));
        width_table[offset..offset + 4].copy_from_slice(b"wdth");
        let regular = (Weight(100), Stretch::Normal, Style::Normal);
        let bold = (Weight(700), Stretch::Normal, Style::Normal);
        let select =
            |table: &[u8], pinned, variable: CssStyle, fixed: CssStyle, request: CssStyle| {
                let (weight, stretch, _) = request;
                let axes = MatchingAxes::from_fvar(
                    ttf_parser::fvar::Table::parse(table).unwrap(),
                    weight.0.into(),
                    css_width(stretch),
                    |_| pinned,
                );
                let attributes = |(weight, stretch, style), axes| FaceAttributes {
                    table: Some((weight, stretch, style)),
                    native: (Some(weight), stretch, style),
                    axes,
                };
                rank_family(
                    vec![
                        ("variable", attributes(variable, axes)),
                        ("static", attributes(fixed, MatchingAxes::default())),
                    ],
                    request,
                    |face| Ok(Some(face)),
                )
                .unwrap()
                .unwrap()
            };
        let semibold = (Weight(600), Stretch::Normal, Style::Normal);
        assert_eq!(
            select(weight_table, None, regular, bold, semibold),
            "variable"
        );
        assert_eq!(select(weight_table, None, regular, bold, bold), "static");
        assert_eq!(
            select(weight_table, Some(200.0), regular, bold, semibold),
            "static"
        );
        let italic = (Weight(700), Stretch::Normal, Style::Italic);
        assert_eq!(
            select(
                weight_table,
                None,
                regular,
                italic,
                (Weight(600), Stretch::Normal, Style::Italic)
            ),
            "static"
        );
        let expanded = (Weight(700), Stretch::ExtraExpanded, Style::Normal);
        assert_eq!(
            select(
                &width_table,
                None,
                regular,
                expanded,
                (Weight(700), Stretch::Expanded, Style::Normal)
            ),
            "variable"
        );
        // The width fixture starts at 100%; it cannot match a condensed request.
        let condensed = (Weight(700), Stretch::Condensed, Style::Normal);
        let semi_condensed = (Weight(700), Stretch::SemiCondensed, Style::Normal);
        assert_eq!(
            select(&width_table, None, regular, semi_condensed, condensed),
            "static"
        );
    }

    #[test]
    #[ignore = "optional qualification with the registered Sukhumvit Set collection"]
    fn condensed_native_face_is_ranked_by_width_before_weight() {
        let Some(descriptors) = family_descriptors("Sukhumvit Set") else {
            return;
        };
        if !descriptors
            .iter()
            .filter_map(|desc| font_from_descriptor(&desc, None).unwrap())
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
