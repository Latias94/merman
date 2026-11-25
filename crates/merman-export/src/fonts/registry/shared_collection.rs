use super::{OpenPlatformFont, read_bounded_file};
use crate::fonts::{
    FontByteBudget, NativeFontError, OperationControl, OperationPhase,
    platform::coretext::NativeFace,
};
use std::path::Path;
use ttf_parser::RawFace;

/// Keeps a verified collection when two copies of its shared outline would cost more.
pub(super) fn shared_collection(
    candidate: &NativeFace,
    path: &Path,
    budget: FontByteBudget,
    max_faces: usize,
    control: &OperationControl,
) -> Result<Option<(Vec<u8>, u32)>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    if max_faces < 2
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ttc") || ext.eq_ignore_ascii_case("otc"))
    {
        return Ok(None);
    }
    let mut source = match OpenPlatformFont::new(path, control) {
        Ok(source) => source,
        Err(NativeFontError::Source { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    if source.len > budget.remaining() as u64 {
        return Ok(None);
    }
    let Some((tag, length)) = candidate.largest_outline(control)? else {
        return Ok(None);
    };
    if length as u64 <= source.len / 2 {
        return Ok(None);
    }

    let mut data = Vec::new();
    read_bounded_file(
        path,
        &mut source.file,
        source.len,
        budget,
        control,
        OperationPhase::Layout,
        &mut data,
    )?;
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    let count = ttf_parser::fonts_in_collection(&data).unwrap_or(0);
    if count < 2 || count as usize > max_faces || length <= data.len() / 2 {
        return Ok(None);
    }

    let outline = |index| RawFace::parse(&data, index).ok()?.table(tag);
    let Some(first) = outline(0).filter(|table| table.len() == length) else {
        return Ok(None);
    };
    for index in 1..count {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        // Both slices borrow this TTC. Pointer and length equality prove the same file range.
        if !outline(index).is_some_and(|table| std::ptr::eq(first, table)) {
            return Ok(None);
        }
    }
    for index in 0..count {
        if candidate.matches_data(&data, index, control)? {
            return Ok(Some((data.into_boxed_slice().into_vec(), index)));
        }
    }
    Ok(None)
}
