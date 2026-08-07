/// Portable grammar for stable runtime-catalog identifiers.
///
/// The pattern intentionally uses only regular-expression syntax shared by Rust generators,
/// JavaScript, and Python. Canonical capability, operation, output, metadata, payload, provider,
/// service, profile, and transport IDs use this form.
pub const RUNTIME_CATALOG_IDENTIFIER_PATTERN: &str = r"^[a-z0-9][a-z0-9-]*$";

/// Portable grammar for schema field and resource-limit identifiers.
///
/// Existing option and resource IDs use underscores, while additive future discovery IDs may use
/// either underscores or hyphens. Host SDKs must therefore accept both without weakening the
/// leading-character or lowercase ASCII contract.
pub const RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN: &str = r"^[a-z][a-z0-9_-]*$";

/// Largest integer that every schema-1 JSON host can represent without precision loss.
///
/// Runtime-catalog producers must keep numeric discovery values within this boundary. Binary
/// operation payloads and native ABI records retain their wider typed integer contracts.
pub const RUNTIME_CATALOG_MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;
