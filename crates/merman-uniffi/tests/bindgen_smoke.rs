#![cfg(feature = "bindgen-smoke")]

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn generates_python_binding_from_cdylib_metadata() {
    let workspace_root = workspace_root();
    let cdylib = build_cdylib(&workspace_root);
    let out_dir = tempfile::tempdir().expect("create bindgen smoke tempdir");

    uniffi::generate(uniffi::GenerateOptions {
        languages: vec![uniffi::TargetLanguage::Python],
        source: utf8_path(&cdylib).into(),
        out_dir: utf8_path(out_dir.path()).into(),
        config_override: None,
        format: false,
        crate_filter: Some("merman_uniffi".to_string()),
        metadata_no_deps: false,
    })
    .expect("generate Python bindings from merman-uniffi cdylib metadata");

    let python_files = generated_files_with_extension(out_dir.path(), "py");
    assert_eq!(
        python_files.len(),
        1,
        "expected exactly one generated Python binding file in {}",
        out_dir.path().display()
    );

    let generated = fs::read_to_string(&python_files[0]).expect("read generated Python binding");
    assert!(
        generated.contains("class MermanEngine"),
        "generated binding should expose MermanEngine"
    );
    assert!(
        generated.contains("def render_svg"),
        "generated binding should expose render_svg"
    );
    assert!(
        generated.contains("class MermanError"),
        "generated binding should expose structured MermanError"
    );
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("merman-uniffi should live under workspace/crates")
        .to_path_buf()
}

fn build_cdylib(workspace_root: &Path) -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(&cargo)
        .current_dir(workspace_root)
        .args([
            "build",
            "-p",
            "merman-uniffi",
            "--features",
            "bindgen-smoke",
        ])
        .status()
        .expect("run cargo build for merman-uniffi cdylib");
    assert!(status.success(), "cargo build -p merman-uniffi failed");

    let target_dir = cargo_target_dir(workspace_root);
    let cdylib = target_dir.join("debug").join(cdylib_filename());
    assert!(
        cdylib.exists(),
        "expected merman-uniffi cdylib at {}",
        cdylib.display()
    );
    cdylib
}

fn cargo_target_dir(workspace_root: &Path) -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .current_dir(workspace_root)
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: Value =
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata JSON");
    let target_directory = metadata
        .get("target_directory")
        .and_then(Value::as_str)
        .expect("cargo metadata target_directory");
    PathBuf::from(target_directory)
}

fn cdylib_filename() -> &'static str {
    if cfg!(windows) {
        "merman_uniffi.dll"
    } else if cfg!(target_os = "macos") {
        "libmerman_uniffi.dylib"
    } else {
        "libmerman_uniffi.so"
    }
}

fn generated_files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_with_extension(dir, extension, &mut files);
    files.sort();
    files
}

fn collect_files_with_extension(dir: &Path, extension: &str, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read generated binding directory") {
        let entry = entry.expect("read generated binding entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path);
        }
    }
}

fn utf8_path(path: &Path) -> String {
    path.to_str()
        .unwrap_or_else(|| panic!("path is not valid UTF-8: {}", path.display()))
        .to_string()
}
