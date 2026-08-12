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

pub(crate) use baseline::{
    acquire_imported_fixture_family_locks, acquire_imported_fixture_transaction_locks,
    acquire_imported_fixture_workspace_lock, candidate_snapshot_failure,
    candidate_upstream_svg_failure, defer_imported_fixture_transaction,
    load_existing_imported_fixtures, reject_imported_fixture_transaction,
    rollback_imported_fixture_snapshots, should_revalidate_deferred_fixture,
    validate_exact_import_candidate_filter,
};
pub(crate) use cypress::{CypressRenderHelper, materialize_cypress_fixture_source};
pub(crate) use docs::import_upstream_docs;
pub(crate) use examples::import_upstream_examples;
pub(crate) use fixture_files::{
    ImportedFixtureSnapshot, cleanup_deferred_fixture_files, cleanup_fixture_files,
    defer_fixture_files_with_replace_existing, imported_fixture_config_look,
    write_imported_fixture,
};
pub(crate) use html::import_upstream_html;

fn normalize_imported_diagram_dir(detected: &str) -> Option<&'static str> {
    merman_core::diagram_type_metadata_id(detected)
}

pub(crate) fn imported_fixture_content_id(body: &str) -> String {
    crate::cmd::cypress_corpus_mmd_sha256(body.as_bytes())[..16].to_string()
}

pub(crate) fn canonicalize_imported_config_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                canonicalize_imported_config_value(value);
            }
        }
        serde_json::Value::Object(map) => {
            let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (key, mut value) in entries {
                canonicalize_imported_config_value(&mut value);
                map.insert(key, value);
            }
        }
        serde_json::Value::Number(number)
            if number.as_i64().is_none() && number.as_u64().is_none() =>
        {
            let Some(float) = number.as_f64().filter(|value| value.is_finite()) else {
                return;
            };
            if float.fract() != 0.0 {
                return;
            }
            if float >= 0.0 && float < u64::MAX as f64 {
                let integer = float as u64;
                if integer as f64 == float {
                    *value = serde_json::Value::Number(integer.into());
                    return;
                }
            }
            if float < 0.0 && float >= i64::MIN as f64 {
                let integer = float as i64;
                if integer as f64 == float {
                    *value = serde_json::Value::Number(integer.into());
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonicalize_imported_config_value, imported_fixture_content_id,
        normalize_imported_diagram_dir,
    };

    #[test]
    fn imported_fixture_routing_is_owned_by_the_family_catalog() {
        for capability in merman_core::diagram_family_capabilities() {
            let expected = capability.metadata_id;
            assert_eq!(
                normalize_imported_diagram_dir(capability.diagram_type),
                expected,
                "diagram type {}",
                capability.diagram_type
            );
        }
    }

    #[test]
    fn imported_fixture_content_identity_is_stable_and_body_addressed() {
        assert_eq!(
            imported_fixture_content_id("flowchart LR\nA-->B\n"),
            "45a6b9293d23b178"
        );
        assert_ne!(
            imported_fixture_content_id("flowchart LR\nA-->B\n"),
            imported_fixture_content_id("flowchart LR\nA-->C\n")
        );
    }

    #[test]
    fn imported_config_identity_sorts_keys_and_normalizes_integral_numbers() {
        let mut config = serde_json::json!({
            "z": 0.0,
            "a": { "second": 2.5, "first": 1.0 },
        });

        canonicalize_imported_config_value(&mut config);

        assert_eq!(
            serde_json::to_string(&config).expect("serialize canonical config"),
            r#"{"a":{"first":1,"second":2.5},"z":0}"#
        );
    }

    #[test]
    fn imported_config_identity_does_not_fold_out_of_range_integral_floats() {
        let mut config = serde_json::json!({ "limit": 18_446_744_073_709_551_616.0 });
        let expected = config.clone();

        canonicalize_imported_config_value(&mut config);

        assert_eq!(config, expected);
    }
}
