//! Machine-readable execution summary; the outer clean-build runner owns source provenance.

#[cfg(not(merman_internal_theme_acceptance))]
fn main() {
    eprintln!("Use scripts/qualify_theme_presets.py to run workspace-only acceptance");
    std::process::exit(2);
}

#[cfg(merman_internal_theme_acceptance)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use merman::svg::ThemePreset;
    use merman_theme_acceptance::{preset_qualification_config, run_preset_qualification};
    use serde_json::json;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    let compiler = merman::svg::DiagramThemeCompiler::new().with_resource_policy(
        merman::svg::ThemeResourcePolicy::for_profile(merman::svg::CLI_DEFAULT_RESOURCE_PROFILE),
    );
    let catalog = json!({
        "schema_version": merman::svg::THEME_PRESET_CATALOG_SCHEMA_VERSION_V1,
        "presets": merman::svg::describe_theme_presets(&compiler),
    });
    let mut presets = Vec::new();
    for preset in [
        ThemePreset::Brutalist,
        ThemePreset::Spotless,
        ThemePreset::Cyberpunk,
    ] {
        let receipt = run_preset_qualification(preset)?;
        let report = receipt.report();
        let cells = report.observations().iter().map(|observation| {
            let target = observation.target_receipt();
            json!({
                "family": observation.spec().family_id().as_str(),
                "source_id": observation.spec().source_id(),
                "source": observation.spec().source(),
                "png_scale": observation.spec().png_scale(),
                "source_digest": hex(observation.source_digest()),
                "output": target.artifact_kind().id(),
                "admission": target.status().id(),
                "reasons": target.reasons().iter().map(|reason| reason.id()).collect::<Vec<_>>(),
                "font_source": target.font_source().id(),
                "resource_fingerprint": hex(target.resource_fingerprint().as_bytes()),
                "font_catalog_fingerprint": hex(target.font_catalog_fingerprint().as_bytes()),
                "document_digest": hex(&target.document_digest()),
                "artifact_digest": hex(&target.artifact_digest()),
                "target_evidence_digest": hex(&target.target_evidence_digest()),
                "target_receipt_digest": hex(&target.receipt_digest()),
                "residuals": {
                    "theme": observation.theme_residual_count(),
                    "source": observation.source_residual_count(),
                    "bridge": observation.bridge_residual_count(),
                    "mermaid": observation.mermaid_residual_count(),
                },
            })
        }).collect::<Vec<_>>();
        presets.push(json!({
            "preset": preset.id(),
            "catalog_entry": receipt.catalog_entry()?,
            "profile": receipt.profile_id(),
            "qualification_schema_revision": receipt.schema_revision(),
            "recipe_fingerprint": hex(report.recipe_fingerprint().as_bytes()),
            "resource_fingerprint": hex(report.resource_fingerprint()),
            "cells": cells,
        }));
    }
    println!(
        "{}",
        serde_json::to_string(&json!({
            "catalog": catalog,
            "presets": presets,
            "render_config": preset_qualification_config().as_value(),
        }))?
    );
    Ok(())
}
