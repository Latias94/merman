use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFType};
use objc2_core_text::{
    CTFont, CTFontDescriptor, kCTFontOpticalSizeAttribute, kCTFontVariationAttribute,
};
use usvg::FontVariation;

use super::{NativeFontError, OperationControl, checkpoint, read_table, table_bytes};

const OPSZ: [u8; 4] = *b"opsz";

/// Translate Core Text design coordinates using the selected physical face's fvar.
pub(super) fn read(
    font: &CTFont,
    descriptor: Option<&CTFontDescriptor>,
    control: Option<&OperationControl>,
) -> Result<Option<Vec<FontVariation>>, NativeFontError> {
    checkpoint(control)?;
    // SAFETY: Core Text returns an immutable dictionary of CFNumber identifiers and values.
    let Some(values) = (unsafe { font.variation() }).filter(|values| !values.is_empty()) else {
        return Ok(Some(Vec::new()));
    };
    // SAFETY: The dictionary stores CFNumber keys and CF object values checked below.
    let values: CFRetained<CFDictionary<CFNumber, CFType>> =
        unsafe { CFRetained::cast_unchecked(values) };
    let table = read_table(font, u32::from_be_bytes(*b"fvar"), control)?;
    let Some(fvar) = table
        .as_deref()
        .map(table_bytes)
        .and_then(ttf_parser::fvar::Table::parse)
    else {
        return Ok(None);
    };
    // ttf-parser::Face::set_variation does not apply coordinates at this axis count.
    if fvar.axes.len() >= 64 || values.len() > usize::from(fvar.axes.len()) {
        return Ok(None);
    }

    let mut variations = Vec::with_capacity(values.len());
    let mut matched = 0;
    for axis in fvar.axes {
        let tag = axis.tag.0.to_be_bytes();
        let key = CFNumber::new_i32(i32::from_be_bytes(tag));
        let Some(value) = values.get(&key) else {
            continue;
        };
        matched += 1;
        let Some(value) = value
            .downcast::<CFNumber>()
            .ok()
            .and_then(|value| value.as_f64())
            .map(|value| value as f32)
            .filter(|value| (axis.min_value..=axis.max_value).contains(value))
        else {
            return Ok(None);
        };
        if value == axis.def_value {
            continue;
        }
        if tag == OPSZ {
            // SAFETY: The live font's construction point size is an immutable scalar.
            let point_size = unsafe { font.size() } as f32;
            if value == point_size.clamp(axis.min_value, axis.max_value)
                && !has_explicit_optical_size(font, descriptor, &key)
            {
                // usvg computes automatic optical sizing from the final glyph size.
                continue;
            }
        }
        let value = if value == 0.0 { 0.0 } else { value };
        variations.push(FontVariation::new(tag, value));
    }
    if matched != values.len() {
        // An unmapped identifier can be a native-only axis. Applying a subset can
        // produce geometry that differs from the Core Text selection.
        return Ok(None);
    }
    variations.sort_unstable_by_key(|variation| variation.tag);
    if variations.windows(2).any(|pair| pair[0].tag == pair[1].tag) {
        return Ok(None);
    }
    Ok(Some(variations))
}

fn has_explicit_optical_size(
    font: &CTFont,
    descriptor: Option<&CTFontDescriptor>,
    axis: &CFNumber,
) -> bool {
    // SAFETY: The key is an immutable Core Text constant and the font is live.
    if unsafe { font.attribute(kCTFontOpticalSizeAttribute) }
        .is_some_and(|value| value.downcast::<CFNumber>().is_ok())
    {
        return true;
    }
    // A matched descriptor can carry an explicit optical coordinate even when it
    // equals the construction point size. Read the descriptor used to create the font.
    // SAFETY: The descriptor and attribute key are live and the return value is retained.
    let Some(axes) = descriptor
        .and_then(|descriptor| unsafe { descriptor.attribute(kCTFontVariationAttribute) })
        .and_then(|value| value.downcast::<CFDictionary>().ok())
    else {
        return false;
    };
    // SAFETY: The variation attribute contains CFNumber keys and CF object values.
    let axes: CFRetained<CFDictionary<CFNumber, CFType>> =
        unsafe { CFRetained::cast_unchecked(axes) };
    axes.get(axis).is_some()
}
