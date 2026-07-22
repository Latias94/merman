use crate::XtaskError;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod baseline;
mod cypress;
mod docs;
mod examples;
mod fixture_files;
mod html;
mod pkg_tests;

pub(crate) use baseline::{
    acquire_imported_fixture_family_locks, acquire_imported_fixture_transaction_locks,
    acquire_imported_fixture_workspace_lock, candidate_snapshot_failure,
    candidate_svg_compare_failure, candidate_upstream_svg_failure,
    defer_imported_fixture_transaction, load_existing_imported_fixtures,
    reject_imported_fixture_transaction, rollback_imported_fixture_snapshots,
    should_revalidate_deferred_fixture, validate_exact_import_candidate_filter,
};
pub(crate) use cypress::{cypress_corpus_source_alignment_failures, import_upstream_cypress};
pub(crate) use docs::import_upstream_docs;
pub(crate) use examples::import_upstream_examples;
pub(crate) use fixture_files::{
    ImportedFixtureSnapshot, cleanup_deferred_fixture_files, cleanup_fixture_files,
    defer_fixture_files_with_replace_existing, imported_fixture_config_look,
    write_imported_fixture,
};
pub(crate) use html::import_upstream_html;
pub(crate) use pkg_tests::import_upstream_pkg_tests;

fn normalize_imported_diagram_dir(detected: &str) -> Option<&'static str> {
    merman_core::diagram_type_metadata_id(detected)
}

#[cfg(test)]
mod tests {
    use super::normalize_imported_diagram_dir;
    use merman_core::baseline::BaselineRegistryProfile;

    #[test]
    fn imported_fixture_routing_is_owned_by_the_family_catalog() {
        for capability in
            merman_core::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Full)
        {
            let expected = capability.metadata_id;
            assert_eq!(
                normalize_imported_diagram_dir(capability.diagram_type),
                expected,
                "diagram type {}",
                capability.diagram_type
            );
        }
    }
}
