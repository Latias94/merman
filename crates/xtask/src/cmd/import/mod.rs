use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

mod cypress;
mod docs;
mod examples;
mod html;
mod pkg_tests;

pub(crate) use cypress::import_upstream_cypress;
pub(crate) use docs::import_upstream_docs;
pub(crate) use examples::import_upstream_examples;
pub(crate) use html::import_upstream_html;
pub(crate) use pkg_tests::import_upstream_pkg_tests;
