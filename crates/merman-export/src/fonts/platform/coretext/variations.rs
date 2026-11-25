use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFType};
use objc2_core_text::{
    CTFont, CTFontDescriptor, kCTFontOpticalSizeAttribute, kCTFontVariationAttribute,
};
use ttf_parser::fvar;
use usvg::FontVariation;

use super::{FVAR, NativeFontError, OperationControl, checkpoint, read_table, table_bytes};

const OPSZ: [u8; 4] = *b"opsz";
const WDTH: [u8; 4] = *b"wdth";
// Skrifa supports heap-backed locations. Bound the native dictionary probes and
// the renderer's linear coordinate lookup for each physical axis.
const MAX_VARIATION_AXES: u16 = 256;

type AxisValues = CFDictionary<CFNumber, CFType>;

#[derive(Clone, Copy, Default)]
pub(super) struct MatchingAxes {
    pub(super) weight: Option<f32>,
    pub(super) width: Option<f32>,
}

impl MatchingAxes {
    pub(super) fn for_font(
        font: &CTFont,
        descriptor: &CTFontDescriptor,
        weight: f32,
        width: f32,
        control: Option<&OperationControl>,
    ) -> Result<Self, NativeFontError> {
        let table = read_table(font, FVAR, control)?;
        let Some(fvar) = table
            .as_deref()
            .map(table_bytes)
            .and_then(fvar::Table::parse)
        else {
            return Ok(Self::default());
        };
        // SAFETY: The retained font supplies its effective design coordinates.
        let selected = unsafe { font.variation() }.map(typed_axis_values);
        let requested = descriptor_axis_values(Some(descriptor));
        Ok(Self::from_fvar(fvar, weight, width, |tag| {
            let key = CFNumber::new_i32(i32::from_be_bytes(tag));
            (
                dictionary_axis_value(selected.as_deref(), &key),
                dictionary_axis_value(requested.as_deref(), &key),
            )
        }))
    }

    pub(super) fn from_fvar(
        table: fvar::Table<'_>,
        weight: f32,
        width: f32,
        mut native: impl FnMut([u8; 4]) -> (Option<f32>, Option<f32>),
    ) -> Self {
        let mut axes = Self::default();
        if table.axes.len() > MAX_VARIATION_AXES {
            return axes;
        }
        for axis in table.axes {
            let tag = axis.tag.0.to_be_bytes();
            let (position, requested) = match &tag {
                b"wght" => (&mut axes.weight, weight),
                b"wdth" => (&mut axes.width, width),
                _ => continue,
            };
            let (selected, descriptor) = native(tag);
            *position = Some(css_axis_position(axis, selected, descriptor, requested));
        }
        axes
    }
}

/// Translate Core Text design coordinates using the selected physical face's fvar.
pub(super) fn read(
    font: &CTFont,
    selection_descriptor: Option<&CTFontDescriptor>,
    css_width: Option<f32>,
    control: Option<&OperationControl>,
) -> Result<Option<Vec<FontVariation>>, NativeFontError> {
    checkpoint(control)?;
    // SAFETY: The font is live and Core Text returns an immutable variation dictionary.
    let selected = unsafe { font.variation() }.map(typed_axis_values);
    let requested = descriptor_axis_values(selection_descriptor);
    let has_selected = selected.as_ref().is_some_and(|axes| !axes.is_empty());
    if css_width.is_none() && !has_selected && requested.as_ref().is_none_or(|axes| axes.is_empty())
    {
        return Ok(Some(Vec::new()));
    }
    let table = read_table(font, FVAR, control)?;
    let Some(fvar) = table
        .as_deref()
        .map(table_bytes)
        .and_then(fvar::Table::parse)
    else {
        // A descriptor axis can be only a matching preference for a static face.
        return Ok((!has_selected).then(Vec::new));
    };
    // SAFETY: These accessors read immutable properties of the retained font.
    let (point_size, optical_size) = unsafe {
        (
            font.size() as f32,
            font.attribute(kCTFontOpticalSizeAttribute),
        )
    };
    let explicit_optical_size =
        optical_size.is_some_and(|value| value.downcast::<CFNumber>().is_ok());
    checkpoint(control)?;
    Ok(physical_variations(
        fvar,
        selected.as_deref(),
        requested.as_deref(),
        css_width,
        point_size,
        explicit_optical_size,
    ))
}

fn physical_variations(
    fvar: fvar::Table<'_>,
    selected: Option<&AxisValues>,
    requested: Option<&AxisValues>,
    css_width: Option<f32>,
    point_size: f32,
    explicit_optical_size: bool,
) -> Option<Vec<FontVariation>> {
    let selected_count = selected.map_or(0, CFDictionary::len);
    if fvar.axes.len() > MAX_VARIATION_AXES || selected_count > usize::from(fvar.axes.len()) {
        return None;
    }
    let mut axes: Vec<_> = fvar.axes.into_iter().collect();
    axes.sort_unstable_by_key(|axis| axis.tag);
    if axes.windows(2).any(|pair| pair[0].tag == pair[1].tag) {
        return None;
    }

    let mut variations = Vec::with_capacity(selected_count);
    let mut matched = 0;
    for axis in axes {
        let tag = axis.tag.0.to_be_bytes();
        let key = CFNumber::new_i32(i32::from_be_bytes(tag));
        let requested = dictionary_axis_value(requested, &key);
        let selected = if let Some(value) = selected.and_then(|axes| axes.get(&key)) {
            matched += 1;
            Some(
                axis_value(&value)
                    .filter(|value| (axis.min_value..=axis.max_value).contains(value))?,
            )
        } else {
            None
        };
        // Family matching ranks movable widths at their reachable CSS position.
        // Keep that same position when the SVG does not carry a width axis of its own.
        let css = css_width
            .filter(|_| tag == WDTH)
            .map(|width| css_axis_position(axis, selected, requested, width));
        let Some(value) = css
            .filter(|value| *value != axis.def_value)
            .or(selected)
            .or_else(|| {
                // CTFontCopyVariation omits defaults. Keep the coordinate if the descriptor
                // used to select the font asked for it explicitly.
                requested.filter(|value| *value == axis.def_value)
            })
        else {
            continue;
        };
        if tag == OPSZ
            && value == point_size.clamp(axis.min_value, axis.max_value)
            && !explicit_optical_size
            && requested.is_none()
        {
            // usvg computes automatic optical sizing from the final glyph size.
            continue;
        }
        variations.push(FontVariation::new(tag, value + 0.0));
    }
    // Every selected axis must be portable to the physical face.
    (matched == selected_count).then_some(variations)
}

fn css_axis_position(
    axis: fvar::VariationAxis,
    selected: Option<f32>,
    requested: Option<f32>,
    css: f32,
) -> f32 {
    // Preserve the native instance and default coordinates explicitly pinned by its descriptor.
    selected
        .filter(|value| {
            *value != axis.def_value && (axis.min_value..=axis.max_value).contains(value)
        })
        .or_else(|| requested.filter(|value| *value == axis.def_value))
        .unwrap_or_else(|| css.clamp(axis.min_value, axis.max_value))
}

fn dictionary_axis_value(axes: Option<&AxisValues>, key: &CFNumber) -> Option<f32> {
    axes?.get(key).as_deref().and_then(axis_value)
}

fn descriptor_axis_values(descriptor: Option<&CTFontDescriptor>) -> Option<CFRetained<AxisValues>> {
    // SAFETY: The live descriptor uses an immutable Core Text attribute key.
    descriptor
        .and_then(|descriptor| unsafe { descriptor.attribute(kCTFontVariationAttribute) })
        .and_then(|value| value.downcast::<CFDictionary>().ok())
        .map(typed_axis_values)
}

fn typed_axis_values(values: CFRetained<CFDictionary>) -> CFRetained<AxisValues> {
    // SAFETY: Core Text variation dictionaries have CFNumber keys and CF object values.
    unsafe { CFRetained::cast_unchecked(values) }
}

fn axis_value(value: &CFType) -> Option<f32> {
    let value = value.downcast_ref::<CFNumber>()?.as_f64()? as f32;
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_core_foundation::CFString;

    const VARIABLE_FONT: &[u8] = include_bytes!("../../../../tests/fonts/MermanVariableWeight.ttf");

    fn fixture_fvar() -> &'static [u8] {
        ttf_parser::RawFace::parse(VARIABLE_FONT, 0)
            .unwrap()
            .table(ttf_parser::Tag(FVAR))
            .unwrap()
    }

    fn width_fixture_fvar() -> Vec<u8> {
        let mut data = fixture_fvar().to_vec();
        let offset = usize::from(u16::from_be_bytes(data[4..6].try_into().unwrap()));
        data[offset..offset + 4].copy_from_slice(&WDTH);
        for (bytes, value) in data[offset + 4..offset + 16]
            .chunks_exact_mut(4)
            .zip([50_i32, 50, 150])
        {
            bytes.copy_from_slice(&(value << 16).to_be_bytes());
        }
        data
    }

    fn values(tag: [u8; 4], value: f64) -> CFRetained<AxisValues> {
        let key = CFNumber::new_i32(i32::from_be_bytes(tag));
        let value = CFNumber::new_f64(value);
        AxisValues::from_slices(&[&key], &[&value])
    }

    #[test]
    fn coretext_keeps_explicit_default_weight_from_the_selection_descriptor() {
        let table = fvar::Table::parse(fixture_fvar()).unwrap();
        let name = CFString::from_str("MermanVariableWeight-Thin");
        let weight = CFNumber::new_i32(i32::from_be_bytes(*b"wght"));
        // SAFETY: These retained objects supply metadata for an immutable descriptor.
        let descriptor = unsafe { CTFontDescriptor::with_name_and_size(&name, 0.0) };
        assert!(
            physical_variations(table, None, None, None, 12.0, false)
                .unwrap()
                .is_empty()
        );
        // SAFETY: The live descriptor and axis identifier describe the requested instance.
        let explicit = unsafe { descriptor.copy_with_variation(&weight, 100.0) };
        let requested = descriptor_axis_values(Some(&explicit)).unwrap();
        assert_eq!(
            physical_variations(table, None, Some(&requested), None, 12.0, false).unwrap(),
            [FontVariation::new(*b"wght", 100.0)]
        );
        assert_eq!(
            physical_variations(table, Some(&requested), None, None, 12.0, false).unwrap(),
            [FontVariation::new(*b"wght", 100.0)]
        );
        let selected = values(*b"wght", 700.0);
        assert_eq!(
            physical_variations(table, Some(&selected), Some(&requested), None, 12.0, false)
                .unwrap(),
            [FontVariation::new(*b"wght", 700.0)]
        );
        assert!(
            physical_variations(table, None, Some(&selected), None, 12.0, false)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn coretext_transports_the_ranked_width_and_preserves_instance_coordinates() {
        let data = width_fixture_fvar();
        let table = fvar::Table::parse(&data).unwrap();
        for (native, descriptor, width, expected) in [
            (None, None, 100.0, 100.0),
            (Some(50.0), None, 100.0, 100.0),
            (Some(125.0), None, 100.0, 125.0),
            (None, Some(50.0), 100.0, 50.0),
            (None, None, 200.0, 150.0),
        ] {
            let ranked = MatchingAxes::from_fvar(table, 400.0, width, |_| (native, descriptor));
            assert_eq!(ranked.width, Some(expected));
            let native = native.map(|value| values(WDTH, f64::from(value)));
            let descriptor = descriptor.map(|value| values(WDTH, f64::from(value)));
            let selected = physical_variations(
                table,
                native.as_deref(),
                descriptor.as_deref(),
                Some(width),
                12.0,
                false,
            );
            assert_eq!(selected.unwrap(), [FontVariation::new(WDTH, expected)]);
        }
    }

    #[test]
    #[ignore = "requires the macOS Core Text font registry"]
    fn coretext_omitted_and_normal_stretch_share_variable_glyph_metrics() {
        use crate::fonts::{FontByteBudget, NativeFontOptions, sfnt};
        use objc2_core_foundation::{CFURL, CFURLPathStyle};
        use std::{ffi::c_void, fs, path::PathBuf, ptr};

        // Use process-local registration for the fixture without enabling more production APIs.
        unsafe extern "C-unwind" {
            fn CTFontManagerRegisterFontsForURL(
                url: &CFURL,
                scope: u32,
                error: *mut *mut c_void,
            ) -> bool;
            fn CTFontManagerUnregisterFontsForURL(
                url: &CFURL,
                scope: u32,
                error: *mut *mut c_void,
            ) -> bool;
        }
        const PROCESS_SCOPE: u32 = 1;
        struct RegisteredFont {
            url: CFRetained<CFURL>,
            path: PathBuf,
        }
        impl Drop for RegisteredFont {
            fn drop(&mut self) {
                // SAFETY: The URL remains live and null discards the optional error.
                unsafe {
                    CTFontManagerUnregisterFontsForURL(&self.url, PROCESS_SCOPE, ptr::null_mut())
                };
                let _ = fs::remove_file(&self.path);
            }
        }

        let raw = ttf_parser::RawFace::parse(VARIABLE_FONT, 0).unwrap();
        let fvar = width_fixture_fvar();
        let mut stat = raw
            .table(ttf_parser::Tag::from_bytes(b"STAT"))
            .unwrap()
            .to_vec();
        let axis_offset = u32::from_be_bytes(stat[8..12].try_into().unwrap()) as usize;
        stat[axis_offset..axis_offset + 4].copy_from_slice(&WDTH);
        let tables = raw.table_records.into_iter().map(|record| {
            let data = match &record.tag.to_bytes() {
                b"fvar" => fvar.as_slice(),
                b"STAT" => stat.as_slice(),
                _ => raw.table(record.tag).unwrap(),
            };
            (record.tag.0, data)
        });
        let path =
            std::env::temp_dir().join(format!("merman-coretext-width-{}.ttf", std::process::id()));
        let url = CFURL::with_file_system_path(
            None,
            Some(&CFString::from_str(path.to_str().unwrap())),
            CFURLPathStyle::CFURLPOSIXPathStyle,
            false,
        )
        .unwrap();
        let control = OperationControl::new();
        let bytes = sfnt::from_tables(
            tables,
            FontByteBudget::new("test-font-bytes", 0, 64 * 1024),
            &control,
            &path,
        )
        .unwrap();
        fs::write(&path, bytes).unwrap();
        let registered = RegisteredFont { url, path };
        // SAFETY: The live URL points to the fixture and null discards the optional error.
        assert!(unsafe {
            CTFontManagerRegisterFontsForURL(&registered.url, PROCESS_SCOPE, ptr::null_mut())
        });

        let context = NativeFontOptions::default()
            .with_system_fonts(true)
            .load_platform(&control)
            .unwrap();
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let render = |attributes| {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="220" height="100">
                <text id="label" y="80" font-family="Merman Variable Weight" font-size="100" {attributes}>AV</text>
            </svg>"#
            );
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            context.check().unwrap();
            let Some(usvg::Node::Text(label)) = tree.node_by_id("label") else {
                panic!("expected text")
            };
            let [span] = label.layouted() else {
                panic!("expected one font instance")
            };
            let [a, v] = span.positioned_glyphs.as_slice() else {
                panic!("expected both glyphs")
            };
            assert!(a.id.0 != 0 && v.id.0 != 0 && a.font == v.font);
            assert_eq!(
                tree.fontdb().face(a.font).unwrap().post_script_name,
                "MermanVariableWeight-Thin"
            );
            (span.variations.clone(), v.transform().tx - a.transform().tx)
        };
        let omitted = render("");
        let normal = render(r#"font-stretch="normal""#);
        assert_eq!(omitted, normal);
        assert_eq!(normal.0, [FontVariation::new(WDTH, 100.0)]);
        assert!((normal.1 - 63.0).abs() < 0.001, "{normal:?}");
        let condensed = render(r#"font-stretch="ultra-condensed""#);
        assert!((condensed.1 - 51.0).abs() < 0.001, "{condensed:?}");
        let explicit = render(r#"font-stretch="normal" font-variation-settings="'wdth' 125""#);
        assert_eq!(explicit.0, [FontVariation::new(WDTH, 125.0)]);
        assert!((explicit.1 - 69.0).abs() < 0.001, "{explicit:?}");
    }

    #[test]
    fn coretext_preserves_explicit_optical_size_and_omits_the_provisional_size() {
        let mut fvar = fixture_fvar().to_vec();
        let axis = usize::from(u16::from_be_bytes(fvar[4..6].try_into().unwrap()));
        fvar[axis..axis + 4].copy_from_slice(b"opsz");
        let table = fvar::Table::parse(&fvar).unwrap();
        let optical_size = values(*b"opsz", 150.0);
        let expected = [FontVariation::new(*b"opsz", 150.0)];
        assert!(
            physical_variations(table, Some(&optical_size), None, None, 150.0, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            physical_variations(table, Some(&optical_size), None, None, 150.0, true).unwrap(),
            expected
        );
        assert_eq!(
            physical_variations(
                table,
                Some(&optical_size),
                Some(&optical_size),
                None,
                150.0,
                false
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            physical_variations(table, Some(&optical_size), None, None, 200.0, false).unwrap(),
            expected
        );
    }

    #[test]
    fn coretext_axis_limit_allows_the_renderer_past_sixty_three_axes() {
        let source = fixture_fvar();
        let axis = usize::from(u16::from_be_bytes(source[4..6].try_into().unwrap()));
        let first = &source[axis..axis + 20];
        let mut fvar = source[..axis + 20].to_vec();
        for index in 1..=MAX_VARIATION_AXES {
            fvar.extend_from_slice(format!("X{index:03}").as_bytes());
            fvar.extend_from_slice(&first[4..]);
        }
        let selected = values(*b"wght", 700.0);
        for count in [64, MAX_VARIATION_AXES] {
            fvar[8..10].copy_from_slice(&count.to_be_bytes());
            let table = fvar::Table::parse(&fvar).unwrap();
            assert_eq!(
                physical_variations(table, Some(&selected), None, None, 12.0, false).unwrap(),
                [FontVariation::new(*b"wght", 700.0)]
            );
        }
        fvar[8..10].copy_from_slice(&(MAX_VARIATION_AXES + 1).to_be_bytes());
        let table = fvar::Table::parse(&fvar).unwrap();
        assert!(physical_variations(table, Some(&selected), None, None, 12.0, false).is_none());
    }
}
