// Both direct callers of the C scanner must pass one flag per external token.
pub const TOKEN_COUNT: usize = external_token_count(include_bytes!("../../src/parser.c"));
pub const DIRECTIVE_BODY: u16 = 21;

const fn external_token_count(mut parser: &[u8]) -> usize {
    const PREFIX: &[u8] = b"#define EXTERNAL_TOKEN_COUNT ";
    while let Some((prefix, mut remainder)) = parser.split_at_checked(PREFIX.len()) {
        if let PREFIX = prefix {
            let mut count = 0;
            while let [digit @ b'0'..=b'9', rest @ ..] = remainder {
                count = count * 10 + (*digit - b'0') as usize;
                remainder = rest;
            }
            assert!(count > 0 && matches!(remainder, [b'\r' | b'\n', ..]));
            return count;
        }
        parser = parser.split_at(1).1;
    }
    panic!("generated parser must define EXTERNAL_TOKEN_COUNT");
}
