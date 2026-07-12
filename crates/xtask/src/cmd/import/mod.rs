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
    validate_exact_import_candidate_filter,
};
pub(crate) use cypress::import_upstream_cypress;
pub(crate) use docs::import_upstream_docs;
pub(crate) use examples::import_upstream_examples;
pub(crate) use fixture_files::{
    ImportedFixtureSnapshot, cleanup_deferred_fixture_files, cleanup_fixture_files,
    defer_fixture_files_with_replace_existing, imported_fixture_config_look,
    write_imported_fixture,
};
pub(crate) use html::import_upstream_html;
pub(crate) use pkg_tests::import_upstream_pkg_tests;
