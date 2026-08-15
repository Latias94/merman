//! Tree-sitter language support for Mermaid source.

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_mermaid() -> *const ();
}

/// The generated Tree-sitter Mermaid language function.
///
/// # Safety
///
/// The symbol is provided by the committed ABI-14 parser source and compiled by this crate's
/// build script. The function has Tree-sitter's stable language-constructor signature.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_mermaid) };

/// Independently versioned language package release.
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Tree-sitter language symbol used by generated bindings.
pub const LANGUAGE_SYMBOL: &str = "mermaid";
/// Tree-sitter generated language ABI selected by this package.
pub const LANGUAGE_ABI: u32 = 14;
/// Experimental public CST schema version.
pub const NODE_SCHEMA_VERSION: u32 = 1;
/// Experimental public query schema version.
pub const QUERY_SCHEMA_VERSION: u32 = 1;
/// Exact Rust Tree-sitter runtime line validated by the package.
pub const TREE_SITTER_RUST_RUNTIME_VERSION: &str = "0.26.12";
/// Generated public node schema.
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");
/// A package-owned editor query profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryProfile {
    /// Query ABI profile name.
    pub profile: &'static str,
    /// Query surface name.
    pub surface: &'static str,
    /// Package-relative query path.
    pub path: &'static str,
    /// Query source bound to the packaged crate.
    pub source: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/query_profiles.rs"));

/// Returns a packaged query by profile and surface.
#[must_use]
pub fn query_profile(profile: &str, surface: &str) -> Option<&'static QueryProfile> {
    QUERY_PROFILES
        .iter()
        .find(|query| query.profile == profile && query.surface == surface)
}

/// Package-relative path to the initial portable highlight query profile.
pub const PORTABLE_HIGHLIGHTS_QUERY_PATH: &str = "queries/portable/highlights.scm";
/// Portable highlight query profile.
pub const PORTABLE_HIGHLIGHTS_QUERY: &str = include_str!("../../queries/portable/highlights.scm");
/// Immutable identities and digests for this generated language release.
pub const ARTIFACT_RECEIPT: &str = include_str!("../../metadata/artifact-receipt.json");

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::{
        ARTIFACT_RECEIPT, LANGUAGE, LANGUAGE_ABI, PORTABLE_HIGHLIGHTS_QUERY,
        PORTABLE_HIGHLIGHTS_QUERY_PATH, QUERY_PROFILES, query_profile,
    };

    #[test]
    fn generated_language_loads_and_reports_the_pinned_abi() {
        let language: tree_sitter::Language = LANGUAGE.into();
        assert_eq!(
            u32::try_from(language.abi_version()).expect("language ABI fits in u32"),
            LANGUAGE_ABI
        );

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("ABI-14 Mermaid language must load in the pinned runtime");
        let tree = parser
            .parse("flowchart TD\n  A --> B\n", None)
            .expect("parser must produce a tree");
        assert_eq!(tree.root_node().kind(), "source_file");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn portable_highlights_compile_and_execute_against_the_generated_language() {
        use tree_sitter::StreamingIterator;

        let language: tree_sitter::Language = LANGUAGE.into();
        let query = tree_sitter::Query::new(&language, PORTABLE_HIGHLIGHTS_QUERY)
            .expect("portable highlight query must compile against the generated language");
        assert!(query.capture_names().contains(&"keyword"));
        assert!(query.capture_names().contains(&"comment"));

        let source = "flowchart TD\n  A --> B\n";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("generated Mermaid language must load");
        let tree = parser
            .parse(source, None)
            .expect("portable query fixture must parse");
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        assert!(captures.next().is_some());
    }

    #[test]
    fn portable_highlights_are_bound_to_the_receipt() {
        let receipt: serde_json::Value =
            serde_json::from_str(ARTIFACT_RECEIPT).expect("artifact receipt must be valid JSON");
        let query_profile = receipt["queryProfiles"]
            .as_array()
            .expect("artifact receipt query profiles must be an array")
            .iter()
            .find(|profile| {
                profile["profile"] == "portable"
                    && profile["surface"] == "highlights"
                    && profile["path"] == PORTABLE_HIGHLIGHTS_QUERY_PATH
            })
            .expect("artifact receipt must bind the portable highlight profile");
        let digest = format!("{:x}", Sha256::digest(PORTABLE_HIGHLIGHTS_QUERY.as_bytes()));
        assert_eq!(query_profile["sha256"], digest);
    }

    #[test]
    fn every_packaged_query_profile_is_bound_to_the_receipt_and_compiles() {
        let language: tree_sitter::Language = LANGUAGE.into();
        let receipt: serde_json::Value =
            serde_json::from_str(ARTIFACT_RECEIPT).expect("artifact receipt must be valid JSON");
        let receipt_profiles = receipt["queryProfiles"]
            .as_array()
            .expect("artifact receipt query profiles must be an array");
        assert_eq!(QUERY_PROFILES.len(), receipt_profiles.len());

        for profile in QUERY_PROFILES {
            assert_eq!(
                query_profile(profile.profile, profile.surface),
                Some(profile)
            );
            tree_sitter::Query::new(&language, profile.source).unwrap_or_else(|error| {
                panic!(
                    "{}/{} query does not compile: {error}",
                    profile.profile, profile.surface
                )
            });
            let receipt_profile = receipt_profiles
                .iter()
                .find(|candidate| {
                    candidate["profile"] == profile.profile
                        && candidate["surface"] == profile.surface
                        && candidate["path"] == profile.path
                })
                .unwrap_or_else(|| {
                    panic!(
                        "receipt lacks {}/{} query profile",
                        profile.profile, profile.surface
                    )
                });
            let digest = format!("{:x}", Sha256::digest(profile.source.as_bytes()));
            assert_eq!(receipt_profile["sha256"], digest);
        }
    }
}
