#![no_main]

use libfuzzer_sys::fuzz_target;

#[path = "../src/tree_sitter_edits.rs"]
mod tree_sitter_edits;

fuzz_target!(|data: &[u8]| {
    tree_sitter_edits::assert_edit_sequence(data);
});
