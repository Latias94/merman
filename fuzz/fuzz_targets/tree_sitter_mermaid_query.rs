#![no_main]

use libfuzzer_sys::fuzz_target;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};
use tree_sitter_mermaid::{LANGUAGE, QUERY_PROFILES};

const MAX_INPUT_BYTES: usize = 64 * 1024;
const MATCH_LIMIT: u32 = 16 * 1024;

fn parser(language: &Language) -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("generated Mermaid language must load");
    parser
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let language: Language = LANGUAGE.into();
    let mut parser = parser(&language);
    let tree = parser
        .parse(data, None)
        .expect("arbitrary input must parse");
    let root = tree.root_node();

    for profile in QUERY_PROFILES {
        let query = Query::new(&language, profile.source).unwrap_or_else(|error| {
            panic!(
                "{}/{} query must compile against the packaged language: {error}",
                profile.profile, profile.surface
            )
        });
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(MATCH_LIMIT);
        let mut captures = cursor.captures(&query, root, data);
        while let Some((matched, capture_index)) = captures.next() {
            let capture = matched.captures[*capture_index];
            assert!(capture.node.start_byte() <= capture.node.end_byte());
            assert!(capture.node.end_byte() <= data.len());
            assert!(capture.node.start_byte() >= root.start_byte());
            assert!(capture.node.end_byte() <= root.end_byte());
            assert!(
                usize::try_from(capture.index).expect("capture index fits usize")
                    < query.capture_names().len()
            );
        }
    }
});
