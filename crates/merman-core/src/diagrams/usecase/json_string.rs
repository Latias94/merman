//! Source token scanning for validated Usecase JSON.

pub(super) use merman_model::families::usecase::json_string::canonical;
use merman_model::families::usecase::json_string::has_unpaired_surrogates;

pub(super) fn token_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut end = start + 1;
    while end < bytes.len() {
        match bytes[end] {
            b'"' => return end + 1,
            b'\\' => end += 2,
            _ => end += 1,
        }
    }
    source.len()
}

/// Called only after serde_json has validated the complete JSON syntax.
pub(super) fn contains_unpaired(source: &str) -> Result<bool, serde_json::Error> {
    let mut offset = 0;
    while offset < source.len() {
        if source.as_bytes()[offset] == b'"' {
            let end = token_end(source, offset);
            if has_unpaired_surrogates(&source[offset..end])? {
                return Ok(true);
            }
            offset = end;
        } else {
            offset += 1;
        }
    }
    Ok(false)
}
