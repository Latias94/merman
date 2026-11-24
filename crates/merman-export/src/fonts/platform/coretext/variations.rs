use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFType};
use objc2_core_text::{
    CTFont, CTFontDescriptor, kCTFontOpticalSizeAttribute, kCTFontVariationAttribute,
};
use ttf_parser::fvar;
use usvg::FontVariation;

use super::{NativeFontError, OperationControl, checkpoint, read_table, table_bytes};

const OPSZ: [u8; 4] = *b"opsz";
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
        weight: f32,
        width: f32,
        control: Option<&OperationControl>,
    ) -> Result<Self, NativeFontError> {
        let table = read_table(font, u32::from_be_bytes(*b"fvar"), control)?;
        let Some(fvar) = table
            .as_deref()
            .map(table_bytes)
            .and_then(fvar::Table::parse)
        else {
            return Ok(Self::default());
        };
        // SAFETY: The retained font supplies its effective design coordinates.
        let selected = unsafe { font.variation() }.map(typed_axis_values);
        Ok(Self::from_fvar(fvar, weight, width, |tag| {
            let key = CFNumber::new_i32(i32::from_be_bytes(tag));
            selected
                .as_deref()?
                .get(&key)
                .as_deref()
                .and_then(axis_value)
        }))
    }

    pub(super) fn from_fvar(
        table: fvar::Table<'_>,
        weight: f32,
        width: f32,
        mut selected: impl FnMut([u8; 4]) -> Option<f32>,
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
            // An installed non-default instance keeps its own position. A base
            // face can realize the authored CSS coordinate during usvg layout.
            *position = Some(
                selected(tag)
                    .filter(|value| *value != axis.def_value)
                    .unwrap_or_else(|| requested.clamp(axis.min_value, axis.max_value)),
            );
        }
        axes
    }
}

/// Translate Core Text design coordinates using the selected physical face's fvar.
pub(super) fn read(
    font: &CTFont,
    selection_descriptor: Option<&CTFontDescriptor>,
    control: Option<&OperationControl>,
) -> Result<Option<Vec<FontVariation>>, NativeFontError> {
    checkpoint(control)?;
    // SAFETY: The font is live and Core Text returns an immutable variation dictionary.
    let selected = unsafe { font.variation() }.map(typed_axis_values);
    let requested = descriptor_axis_values(selection_descriptor);
    let has_selected = selected.as_ref().is_some_and(|axes| !axes.is_empty());
    if !has_selected && requested.as_ref().is_none_or(|axes| axes.is_empty()) {
        return Ok(Some(Vec::new()));
    }
    let table = read_table(font, u32::from_be_bytes(*b"fvar"), control)?;
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
        point_size,
        explicit_optical_size,
    ))
}

fn physical_variations(
    fvar: fvar::Table<'_>,
    selected: Option<&AxisValues>,
    requested: Option<&AxisValues>,
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
        let requested = requested
            .and_then(|axes| axes.get(&key))
            .as_deref()
            .and_then(axis_value);
        let value = if let Some(value) = selected.and_then(|axes| axes.get(&key)) {
            matched += 1;
            axis_value(&value).filter(|value| (axis.min_value..=axis.max_value).contains(value))?
        } else if requested == Some(axis.def_value) {
            // CTFontCopyVariation omits defaults. Keep the coordinate if the descriptor
            // used to select the font asked for it explicitly.
            axis.def_value
        } else {
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

    fn fixture_fvar() -> &'static [u8] {
        let raw = ttf_parser::RawFace::parse(
            include_bytes!("../../../../tests/fonts/MermanVariableWeight.ttf"),
            0,
        )
        .unwrap();
        raw.table(ttf_parser::Tag::from_bytes(b"fvar")).unwrap()
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
            physical_variations(table, None, None, 12.0, false)
                .unwrap()
                .is_empty()
        );
        // SAFETY: The live descriptor and axis identifier describe the requested instance.
        let explicit = unsafe { descriptor.copy_with_variation(&weight, 100.0) };
        let requested = descriptor_axis_values(Some(&explicit)).unwrap();
        assert_eq!(
            physical_variations(table, None, Some(&requested), 12.0, false).unwrap(),
            [FontVariation::new(*b"wght", 100.0)]
        );
        assert_eq!(
            physical_variations(table, Some(&requested), None, 12.0, false).unwrap(),
            [FontVariation::new(*b"wght", 100.0)]
        );
        let selected = values(*b"wght", 700.0);
        assert_eq!(
            physical_variations(table, Some(&selected), Some(&requested), 12.0, false).unwrap(),
            [FontVariation::new(*b"wght", 700.0)]
        );
        assert!(
            physical_variations(table, None, Some(&selected), 12.0, false)
                .unwrap()
                .is_empty()
        );
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
            physical_variations(table, Some(&optical_size), None, 150.0, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            physical_variations(table, Some(&optical_size), None, 150.0, true).unwrap(),
            expected
        );
        assert_eq!(
            physical_variations(
                table,
                Some(&optical_size),
                Some(&optical_size),
                150.0,
                false
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            physical_variations(table, Some(&optical_size), None, 200.0, false).unwrap(),
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
                physical_variations(table, Some(&selected), None, 12.0, false).unwrap(),
                [FontVariation::new(*b"wght", 700.0)]
            );
        }
        fvar[8..10].copy_from_slice(&(MAX_VARIATION_AXES + 1).to_be_bytes());
        let table = fvar::Table::parse(&fvar).unwrap();
        assert!(physical_variations(table, Some(&selected), None, 12.0, false).is_none());
    }
}
