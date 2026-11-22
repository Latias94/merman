//! Bounded SFNT assembly from the tables of one platform-selected face.

use super::{NativeFontError, OperationControl, OperationPhase, check_limit, source_error};
use std::collections::BTreeSet;
use std::path::Path;
use write_fonts::{FontBuilder, types::Tag};

const HEADER_LEN: usize = 12;
const TABLE_RECORD_LEN: usize = 16;
const DSIG: u32 = u32::from_be_bytes(*b"DSIG");

/// Repackages one face at index zero, preserving opaque tables without subsetting.
///
/// `limit` bounds the serialized bytes, including the directory and table padding.
/// The caller owns the input buffers and validates the result before registration;
/// neither this adapter nor `FontBuilder` validates table contents.
pub(super) fn from_tables<'a>(
    tables: impl IntoIterator<Item = (u32, &'a [u8])>,
    limit: usize,
    control: &OperationControl,
    path: &Path,
) -> Result<Vec<u8>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    let mut size = HEADER_LEN;
    check_limit("font-bytes", size, limit)?;
    let mut builder = FontBuilder::new();
    let mut seen = BTreeSet::new();
    let mut table_count = 0u16;

    for (tag, data) in tables {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        if !seen.insert(tag) {
            return Err(source_error(
                path,
                format!("duplicate SFNT table tag {tag:#010x}"),
            ));
        }
        if tag == DSIG {
            // Rebuilding changes the signed bytes, so the original signature is invalid.
            continue;
        }

        table_count = table_count
            .checked_add(1)
            .ok_or_else(|| source_error(path, "SFNT table count exceeds u16"))?;
        // write-fonts 0.48.1 also unwraps the u16 searchRange/rangeShift fields.
        // With 16-byte records, 4096 tables already overflow searchRange.
        let directory_len = usize::from(table_count)
            .checked_mul(TABLE_RECORD_LEN)
            .ok_or_else(|| source_error(path, "SFNT directory size overflow"))?;
        if directory_len > usize::from(u16::MAX) {
            return Err(source_error(
                path,
                "SFNT directory search fields exceed u16",
            ));
        }
        u32::try_from(data.len())
            .map_err(|_| source_error(path, "SFNT table length exceeds u32"))?;
        let padded_len = data
            .len()
            .checked_add(3)
            .map(|len| len & !3)
            .ok_or_else(|| source_error(path, "SFNT table alignment overflow"))?;
        size = size
            .checked_add(TABLE_RECORD_LEN)
            .and_then(|size| size.checked_add(padded_len))
            .ok_or_else(|| source_error(path, "SFNT size overflow"))?;
        // The total bounds every intermediate offset, regardless of table ordering.
        u32::try_from(size).map_err(|_| source_error(path, "SFNT size exceeds u32 offsets"))?;
        check_limit("font-bytes", size, limit)?;
        builder.add_raw(Tag::from_be_bytes(tag.to_be_bytes()), data);
    }

    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    // All size checks precede this synchronous build. FontBuilder writes the directory,
    // padding and checksums, including head.checkSumAdjustment, without changing our inputs.
    let bytes = builder.build();
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::{OperationCancelled, OperationLedgerError};
    use ttf_parser::{Face, RawFace};

    const COUSINE: &[u8] = include_bytes!("../../tests/fonts/Cousine-Regular.ttf");

    fn face_tables(bytes: &[u8], index: u32) -> Vec<(u32, &[u8])> {
        let face = RawFace::parse(bytes, index).unwrap();
        face.table_records
            .into_iter()
            .map(|record| (record.tag.0, face.table(record.tag).unwrap()))
            .collect()
    }

    #[test]
    fn selected_face_preserves_tables_and_metrics_at_index_zero() {
        for (source, index) in [
            (COUSINE, 0),
            (
                include_bytes!("../../tests/fonts/Cousine-test-collection.ttc").as_slice(),
                1,
            ),
            (
                include_bytes!("../../tests/fonts/MermanVariableWeight.ttf").as_slice(),
                0,
            ),
        ] {
            let original = Face::parse(source, index).unwrap();
            let tables = face_tables(source, index);
            let bytes = from_tables(
                tables.iter().copied(),
                usize::MAX,
                &OperationControl::new(),
                Path::new("selected-face"),
            )
            .unwrap();
            assert_eq!(ttf_parser::fonts_in_collection(&bytes), None);
            assert!(Face::parse(&bytes, 1).is_err());
            let rebuilt = Face::parse(&bytes, 0).unwrap();
            assert_eq!(rebuilt.number_of_glyphs(), original.number_of_glyphs());
            assert_eq!(rebuilt.units_per_em(), original.units_per_em());
            assert_eq!(rebuilt.ascender(), original.ascender());
            assert_eq!(rebuilt.descender(), original.descender());
            assert_eq!(rebuilt.is_italic(), original.is_italic());
            let glyph = original.glyph_index('A').unwrap();
            assert_eq!(rebuilt.glyph_index('A'), Some(glyph));
            assert_eq!(
                rebuilt.glyph_hor_advance(glyph),
                original.glyph_hor_advance(glyph)
            );
            assert_eq!(
                rebuilt.glyph_bounding_box(glyph),
                original.glyph_bounding_box(glyph)
            );

            assert_eq!(
                usize::from(rebuilt.raw_face().table_records.len()),
                tables.len()
            );
            for (tag, expected) in tables {
                let actual = rebuilt.raw_face().table(ttf_parser::Tag(tag)).unwrap();
                if tag == u32::from_be_bytes(*b"head") {
                    assert_eq!(&actual[..8], &expected[..8]);
                    assert_eq!(&actual[12..], &expected[12..]);
                } else {
                    assert_eq!(actual, expected, "table {tag:#010x}");
                }
            }
            assert_eq!(bytes.len() % 4, 0);
            let checksum = bytes.chunks_exact(4).fold(0u32, |sum, word| {
                sum.wrapping_add(u32::from_be_bytes(word.try_into().unwrap()))
            });
            assert_eq!(checksum, 0xB1B0_AFBA);
        }
    }

    #[test]
    fn exact_budget_includes_directory_and_alignment() {
        let table = [(u32::from_be_bytes(*b"TEST"), b"abc".as_slice())];
        let path = Path::new("selected-face");
        let bytes = from_tables(table, 32, &OperationControl::new(), path).unwrap();
        assert_eq!(bytes.len(), 32);
        assert!(matches!(
            from_tables(table, 31, &OperationControl::new(), path),
            Err(NativeFontError::Limit {
                resource: "font-bytes",
                actual: 32,
                limit: 31,
            })
        ));
    }

    #[test]
    fn duplicate_tags_are_errors_including_discarded_signatures() {
        let path = Path::new("selected-face");
        for tag in [u32::from_be_bytes(*b"TEST"), DSIG] {
            let result = from_tables(
                [(tag, b"first".as_slice()), (tag, b"second".as_slice())],
                usize::MAX,
                &OperationControl::new(),
                path,
            );
            assert!(
                matches!(result, Err(NativeFontError::Source { path: actual, message })
                if actual == path && message.contains("duplicate SFNT table tag"))
            );
        }
    }

    #[test]
    fn cancellation_is_observed_before_and_during_table_acquisition() {
        let path = Path::new("selected-face");
        let control = OperationControl::new();
        control.cancel();
        let tables = std::iter::from_fn(|| -> Option<(u32, &[u8])> {
            panic!("cancelled assembly must not acquire tables")
        });
        assert!(matches!(
            from_tables(tables, usize::MAX, &control, path),
            Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
                OperationCancelled {
                    phase: OperationPhase::Layout,
                    ..
                }
            )))
        ));

        let control = OperationControl::new();
        let mut acquired = 0;
        let tables = face_tables(COUSINE, 0).into_iter().inspect(|_| {
            acquired += 1;
            if acquired == 2 {
                control.cancel();
            }
        });
        assert!(matches!(
            from_tables(tables, usize::MAX, &control, path),
            Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
                OperationCancelled {
                    phase: OperationPhase::Layout,
                    ..
                }
            )))
        ));
        assert_eq!(acquired, 2);
    }

    #[test]
    fn drops_dsig_but_preserves_color_and_unknown_tables() {
        let mut tables = face_tables(COUSINE, 0);
        // Opaque payloads exercise preservation, not validation of optional table contents.
        let extras = [
            (u32::from_be_bytes(*b"COLR"), b"color".as_slice()),
            (u32::from_be_bytes(*b"CPAL"), b"palette".as_slice()),
            (u32::from_be_bytes(*b"SVG "), b"svg".as_slice()),
            (u32::from_be_bytes(*b"TEST"), b"unknown".as_slice()),
        ];
        tables.extend(extras);
        let control = OperationControl::new();
        let path = Path::new("selected-face");
        let unsigned = from_tables(tables.iter().copied(), usize::MAX, &control, path).unwrap();
        tables.push((DSIG, b"invalidated original signature"));
        let rebuilt = from_tables(tables, unsigned.len(), &control, path).unwrap();
        assert_eq!(rebuilt, unsigned);
        let face = RawFace::parse(&rebuilt, 0).unwrap();
        assert!(face.table(ttf_parser::Tag(DSIG)).is_none());
        for (tag, bytes) in extras {
            assert_eq!(face.table(ttf_parser::Tag(tag)), Some(bytes));
        }
    }

    #[test]
    fn rejects_directory_search_field_overflow_before_build() {
        let tables = (0..4096).map(|tag| (tag, b"".as_slice()));
        let result = from_tables(
            tables,
            usize::MAX,
            &OperationControl::new(),
            Path::new("selected-face"),
        );
        assert!(
            matches!(result, Err(NativeFontError::Source { message, .. })
            if message.contains("search fields exceed u16"))
        );
    }

    #[test]
    fn rejects_aggregate_offset_overflow_without_materializing_gigabytes() {
        let shared = vec![0; 1024 * 1024 + 1024];
        let tables = (0..4095).map(|tag| (tag, shared.as_slice()));
        let result = from_tables(
            tables,
            usize::MAX,
            &OperationControl::new(),
            Path::new("selected-face"),
        );
        assert!(matches!(result, Err(NativeFontError::Source { .. })));
    }
}
