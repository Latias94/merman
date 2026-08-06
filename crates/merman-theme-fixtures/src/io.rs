use crate::CatalogError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub(crate) struct VerifiedFile {
    path: PathBuf,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn canonical_root(root: &Path) -> Result<PathBuf, CatalogError> {
    let canonical = fs::canonicalize(root).map_err(|source| CatalogError::InvalidRoot {
        path: root.to_path_buf(),
        source,
    })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(CatalogError::RootNotDirectory(canonical))
    }
}

pub(crate) fn validate_id(id: &str) -> Result<(), CatalogError> {
    let mut bytes = id.bytes();
    let first_valid = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    if first_valid
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.ends_with('-')
        && !id.contains("--")
    {
        Ok(())
    } else {
        Err(CatalogError::InvalidId(id.to_string()))
    }
}

pub(crate) fn validate_revision(record: &str, revision: &str) -> Result<(), CatalogError> {
    if revision.len() == 40
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(CatalogError::InvalidRevision {
            record: record.to_string(),
            revision: revision.to_string(),
        })
    }
}

pub(crate) fn validate_relative_path(
    path: &str,
    kind: &'static str,
) -> Result<PathBuf, CatalogError> {
    if path.is_empty() || path.contains('\\') {
        return Err(CatalogError::InvalidPath {
            path: path.to_string(),
            kind,
        });
    }
    let value = Path::new(path);
    if value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CatalogError::InvalidPath {
            path: path.to_string(),
            kind,
        });
    }
    let normalized = value
        .components()
        .map(|component| match component {
            Component::Normal(segment) => segment.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"));
    if normalized.as_deref() != Some(path) {
        return Err(CatalogError::InvalidPath {
            path: path.to_string(),
            kind,
        });
    }
    Ok(value.to_path_buf())
}

fn resolve_file(root: &Path, relative: &str, kind: &'static str) -> Result<PathBuf, CatalogError> {
    let candidate = root.join(validate_relative_path(relative, kind)?);
    let canonical = fs::canonicalize(&candidate).map_err(|source| CatalogError::ReadFile {
        path: candidate,
        source,
    })?;
    if !canonical.starts_with(root) {
        return Err(CatalogError::EscapingPath(relative.to_string()));
    }
    if canonical.is_file() {
        Ok(canonical)
    } else {
        Err(CatalogError::NotAFile(canonical))
    }
}

pub(crate) fn validate_sha256(record: &str, value: &str) -> Result<(), CatalogError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(CatalogError::InvalidSha256 {
            record: record.to_string(),
            value: value.to_string(),
        })
    }
}

pub(crate) fn validate_unicode_range(record: &str, value: &str) -> Result<(), CatalogError> {
    if value.is_empty()
        || value
            .split(',')
            .any(|entry| parse_unicode_range_entry(entry).is_none())
    {
        Err(CatalogError::InvalidUnicodeRange(record.to_string()))
    } else {
        Ok(())
    }
}

pub(crate) fn unicode_range_contains(value: &str, codepoint: u32) -> Result<bool, ()> {
    let mut valid = false;
    for entry in value.split(',') {
        let (start, end) = parse_unicode_range_entry(entry).ok_or(())?;
        valid = true;
        if (start..=end).contains(&codepoint) {
            return Ok(true);
        }
    }
    if valid { Ok(false) } else { Err(()) }
}

pub(crate) fn parse_unicode_range_entry(entry: &str) -> Option<(u32, u32)> {
    let entry = entry.trim();
    let range = entry
        .strip_prefix("U+")
        .or_else(|| entry.strip_prefix("u+"))?;
    let (start, end) = match range.split_once('-') {
        Some((start, end)) => (
            u32::from_str_radix(start, 16).ok()?,
            u32::from_str_radix(end, 16).ok()?,
        ),
        None => {
            let codepoint = u32::from_str_radix(range, 16).ok()?;
            (codepoint, codepoint)
        }
    };
    (start <= end && end <= 0x10ffff).then_some((start, end))
}

pub(crate) fn parse_unicode_codepoint(value: &str) -> Option<u32> {
    let (start, end) = parse_unicode_range_entry(value)?;
    (start == end).then_some(start)
}

pub(crate) fn validate_hashed_file(
    root: &Path,
    record: &str,
    relative: &str,
    expected: &str,
) -> Result<VerifiedFile, CatalogError> {
    validate_sha256(record, expected)?;
    let path = resolve_file(root, relative, "hashed fixture file")?;
    let contents = fs::read(&path).map_err(|source| CatalogError::ReadFile {
        path: path.clone(),
        source,
    })?;
    if contents.is_empty() {
        return Err(CatalogError::EmptyEvidenceFile {
            record: record.to_string(),
            path: relative.to_string(),
        });
    }
    let actual = format!("{:x}", Sha256::digest(&contents));
    if actual == expected {
        Ok(VerifiedFile {
            path,
            bytes: contents,
        })
    } else {
        Err(CatalogError::HashMismatch {
            record: record.to_string(),
            path: relative.to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

pub(crate) fn verified_text(file: VerifiedFile) -> Result<String, CatalogError> {
    String::from_utf8(file.bytes).map_err(|source| CatalogError::InvalidUtf8File {
        path: file.path,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_unicode_range_entry, unicode_range_contains};

    #[test]
    fn unicode_range_parser_accepts_singletons_and_closed_ranges() {
        assert_eq!(parse_unicode_range_entry("U+6d4b"), Some((0x6d4b, 0x6d4b)));
        assert_eq!(parse_unicode_range_entry("U+20-7E"), Some((0x20, 0x7e)));
        assert_eq!(parse_unicode_range_entry("U+7E-20"), None);
        assert_eq!(parse_unicode_range_entry("U+110000"), None);
    }

    #[test]
    fn unicode_range_coverage_rejects_unlisted_fixture_text() {
        let ranges = "U+20-7e,U+6d4b,U+8bd5";
        assert!(unicode_range_contains(ranges, u32::from('测')).expect("valid range"));
        assert!(unicode_range_contains(ranges, u32::from('试')).expect("valid range"));
        assert!(!unicode_range_contains(ranges, u32::from('主')).expect("valid range"));
    }
}
