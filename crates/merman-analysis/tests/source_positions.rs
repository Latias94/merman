use merman_analysis::{SourceMap, Utf16Position, markdown::is_markdown_path};
use std::path::Path;

#[test]
fn source_map_converts_utf16_positions_to_offsets() {
    let map = SourceMap::new("a\nb💡c\n");

    assert_eq!(
        map.byte_offset_for_utf16_position(Utf16Position {
            line: 1,
            character: 3,
        }),
        Some("a\nb💡".len())
    );
}

#[test]
fn source_map_tracks_final_empty_line_after_trailing_newline() {
    let map = SourceMap::new("a\n");

    assert_eq!(
        map.utf16_position(map.source_len()).unwrap(),
        Utf16Position {
            line: 1,
            character: 0,
        }
    );
    assert_eq!(
        map.byte_offset_for_utf16_position(Utf16Position {
            line: 1,
            character: 0,
        }),
        Some(map.source_len())
    );
    assert_eq!(
        map.line_bounds(1),
        Some((map.source_len(), map.source_len()))
    );
}

#[test]
fn markdown_path_detection_matches_expected_extensions() {
    assert!(is_markdown_path(Path::new("/tmp/example.md")));
    assert!(is_markdown_path(Path::new("/tmp/example.markdown")));
    assert!(is_markdown_path(Path::new("/tmp/example.mdx")));
    assert!(!is_markdown_path(Path::new("/tmp/example.mmd")));
}
