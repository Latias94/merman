use crate::XtaskError;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod cypress;
mod docs;
mod examples;
mod fixture_files;
mod html;
mod pkg_tests;

pub(crate) use cypress::import_upstream_cypress;
pub(crate) use docs::import_upstream_docs;
pub(crate) use examples::import_upstream_examples;
pub(crate) use fixture_files::{
    cleanup_deferred_fixture_files, cleanup_fixture_files,
    defer_fixture_files_with_replace_existing, imported_fixture_config_look,
    write_imported_fixture,
};
pub(crate) use html::import_upstream_html;
pub(crate) use pkg_tests::import_upstream_pkg_tests;
