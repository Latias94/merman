//! Combined generation and verification for editor-language artifacts.

use crate::XtaskError;
use crate::cmd::editor_analysis_config::{
    EDITOR_LANGUAGE_CONTRACT_COMMAND, VSCODE_ANALYSIS_CONFIG_BASELINE_PATH, VSCODE_MANIFEST_PATH,
    baseline_artifact, baseline_drift, project_vscode_analysis_settings,
};
use crate::cmd::editor_token_descriptor::{
    DESCRIPTOR_PATH, TokenDescriptor, apply_vscode_token_projection, read_descriptor,
    token_artifact_drift, token_artifacts,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn language_contract_error(message: impl Into<String>) -> XtaskError {
    XtaskError::VerifyFailed(format!(
        "editor language contract is invalid: {}",
        message.into()
    ))
}

fn write_generated_artifact(path: &Path, contents: &str) -> Result<(), XtaskError> {
    let parent = path.parent().ok_or_else(|| {
        language_contract_error(format!("generated path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;
    fs::write(path, contents).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn manifest_projection(root: &Path, descriptor: &TokenDescriptor) -> Result<String, XtaskError> {
    let path = root.join(VSCODE_MANIFEST_PATH);
    let text = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let mut manifest: Value = serde_json::from_str(&text).map_err(|error| {
        language_contract_error(format!(
            "failed to parse VS Code manifest {}: {error}",
            path.display()
        ))
    })?;
    let contributes = manifest
        .as_object_mut()
        .and_then(|manifest| manifest.get_mut("contributes"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| language_contract_error("VS Code manifest contributes must be an object"))?;

    apply_vscode_token_projection(contributes, descriptor)?;
    project_vscode_analysis_settings(
        contributes,
        &crate::cmd::editor_analysis_config::client_projection(),
    )?;

    let mut output = serde_json::to_string_pretty(&manifest)?;
    output.push('\n');
    Ok(output)
}

fn manifest_drift(root: &Path, descriptor: &TokenDescriptor) -> Result<bool, XtaskError> {
    let expected = manifest_projection(root, descriptor)?;
    let path = root.join(VSCODE_MANIFEST_PATH);
    let actual = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    Ok(actual.replace("\r\n", "\n") != expected)
}

pub(crate) fn gen_editor_language_contract(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    let mut artifacts = token_artifacts(&root, &descriptor)?;
    artifacts.push((
        PathBuf::from(VSCODE_ANALYSIS_CONFIG_BASELINE_PATH),
        baseline_artifact()?,
    ));
    artifacts.push((
        PathBuf::from(VSCODE_MANIFEST_PATH),
        manifest_projection(&root, &descriptor)?,
    ));
    for (relative_path, contents) in artifacts {
        write_generated_artifact(&root.join(relative_path), &contents)?;
    }
    Ok(())
}

pub(crate) fn verify_editor_language_contract_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let descriptor = read_descriptor(&root.join(DESCRIPTOR_PATH))?;
    let mut drift = token_artifact_drift(&root, &descriptor)?;
    if baseline_drift(&root)? {
        drift.push(PathBuf::from(VSCODE_ANALYSIS_CONFIG_BASELINE_PATH));
    }
    if manifest_drift(&root, &descriptor)? {
        drift.push(PathBuf::from(VSCODE_MANIFEST_PATH));
    }
    if drift.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "editor language generated contract drifted: {}; regenerate with `{EDITOR_LANGUAGE_CONTRACT_COMMAND}`",
            drift
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }
}

pub(crate) fn verify_editor_language_contract(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    match verify_editor_language_contract_artifacts()? {
        Some(message) => Err(XtaskError::VerifyFailed(message)),
        None => Ok(()),
    }
}
