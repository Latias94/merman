use crate::{XtaskError, cmd::paths};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypstBuildProfile {
    Minimal,
    Full,
}

impl TypstBuildProfile {
    fn parse(raw: &str) -> Result<Self, XtaskError> {
        match raw {
            "minimal" | "default" => Ok(Self::Minimal),
            "full" | "core-full" => Ok(Self::Full),
            _ => Err(XtaskError::Usage),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Full => "full",
        }
    }
}

#[derive(Debug)]
struct Options {
    profile: TypstBuildProfile,
    out_dir: PathBuf,
    skip_wasm_build: bool,
}

#[derive(Debug, serde::Deserialize)]
struct TypstManifest {
    package: TypstManifestPackage,
}

#[derive(Debug, serde::Deserialize)]
struct TypstManifestPackage {
    version: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            profile: TypstBuildProfile::Minimal,
            out_dir: paths::workspace_root().join("dist").join("typst"),
            skip_wasm_build: false,
        }
    }
}

pub(crate) fn build_typst_package(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let root = paths::workspace_root();
    let package_source = root.join("packages").join("typst").join("merman");
    let manifest_path = package_source.join("typst.toml");
    let package_version = read_typst_package_version(&manifest_path)?;
    let wasm_path = root
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("wasm-size")
        .join("merman_typst_plugin.wasm");

    if !options.skip_wasm_build {
        build_wasm(options.profile, &wasm_path)?;
    }

    if !wasm_path.exists() {
        return Err(XtaskError::TypstPackageFailed(format!(
            "missing wasm artifact: {}\nrun without --skip-wasm-build first",
            wasm_path.display()
        )));
    }

    let package_dir = options.out_dir.join("merman").join(&package_version);
    fs::create_dir_all(&package_dir).map_err(|source| XtaskError::WriteFile {
        path: package_dir.display().to_string(),
        source,
    })?;

    copy_file(&manifest_path, &package_dir.join("typst.toml"))?;
    copy_file(
        &package_source.join("lib.typ"),
        &package_dir.join("lib.typ"),
    )?;
    copy_file(
        &package_source.join("README.md"),
        &package_dir.join("README.md"),
    )?;
    copy_file(&wasm_path, &package_dir.join("merman_typst_plugin.wasm"))?;

    for license in ["LICENSE", "LICENSE-MIT", "LICENSE-APACHE"] {
        let source = root.join(license);
        if source.exists() {
            copy_file(&source, &package_dir.join(license))?;
        }
    }

    let examples_source = package_source.join("examples");
    if examples_source.exists() {
        copy_dir_recursive(&examples_source, &package_dir.join("examples"))?;
    }

    let display_package_dir = package_dir
        .canonicalize()
        .unwrap_or_else(|_| package_dir.clone());

    println!(
        "Typst package built profile={} path={}",
        options.profile.label(),
        display_package_dir.display()
    );
    println!(
        "Local install target: <typst package path>/local/merman/{}",
        package_version
    );
    println!(
        "Preview smoke target: <typst package path>/preview/merman/{}",
        package_version
    );
    println!("Tip: run `typst info` to find your Typst package path.");

    Ok(())
}

fn read_typst_package_version(manifest_path: &Path) -> Result<String, XtaskError> {
    let manifest_text =
        fs::read_to_string(manifest_path).map_err(|source| XtaskError::ReadFile {
            path: manifest_path.display().to_string(),
            source,
        })?;
    let manifest: TypstManifest = toml::from_str(&manifest_text).map_err(|source| {
        XtaskError::TypstPackageFailed(format!(
            "failed to parse {}: {source}",
            manifest_path.display()
        ))
    })?;
    let version = manifest.package.version.trim();
    if !is_typst_package_version(version) {
        return Err(XtaskError::TypstPackageFailed(format!(
            "{} has unsupported Typst package version `{}`; Typst imports require an x.y.z numeric version",
            manifest_path.display(),
            manifest.package.version
        )));
    }
    Ok(version.to_string())
}

fn is_typst_package_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }

    [major, minor, patch]
        .into_iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parse_options(args: Vec<String>) -> Result<Options, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return Err(XtaskError::Usage);
    }

    let mut options = Options::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" => {
                let raw = iter.next().ok_or(XtaskError::Usage)?;
                options.profile = TypstBuildProfile::parse(&raw)?;
            }
            "--out" => {
                options.out_dir = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--skip-wasm-build" => {
                options.skip_wasm_build = true;
            }
            _ => {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    Ok(options)
}

fn print_usage() {
    println!(
        "usage: xtask build-typst-package [--profile minimal|full] [--out <dir>] [--skip-wasm-build]"
    );
}

fn build_wasm(profile: TypstBuildProfile, wasm_path: &Path) -> Result<(), XtaskError> {
    let mut command = Command::new("cargo");
    command.args([
        "build",
        "-p",
        "merman-typst-plugin",
        "--profile",
        "wasm-size",
        "--target",
        "wasm32-unknown-unknown",
    ]);
    if matches!(profile, TypstBuildProfile::Full) {
        command.args(["--features", "core-full"]);
    }

    let status = command.status().map_err(|source| XtaskError::ReadFile {
        path: "cargo".to_string(),
        source,
    })?;
    if !status.success() {
        return Err(XtaskError::TypstPackageFailed(format!(
            "cargo build failed with status {status}"
        )));
    }

    strip_wasm(wasm_path)?;
    Ok(())
}

fn strip_wasm(wasm_path: &Path) -> Result<(), XtaskError> {
    let stripped_path = wasm_path.with_file_name("merman_typst_plugin.stripped.wasm");
    let status = Command::new("wasm-tools")
        .args(["strip", "--all"])
        .arg(wasm_path)
        .arg("-o")
        .arg(&stripped_path)
        .status()
        .map_err(|source| XtaskError::ReadFile {
            path: "wasm-tools".to_string(),
            source,
        })?;

    if !status.success() {
        return Err(XtaskError::TypstPackageFailed(format!(
            "wasm-tools strip failed with status {status}"
        )));
    }

    fs::rename(&stripped_path, wasm_path).map_err(|source| XtaskError::WriteFile {
        path: wasm_path.display().to_string(),
        source,
    })?;
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), XtaskError> {
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|source_err| XtaskError::WriteFile {
            path: destination.display().to_string(),
            source: source_err,
        })
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), XtaskError> {
    fs::create_dir_all(destination).map_err(|source_err| XtaskError::WriteFile {
        path: destination.display().to_string(),
        source: source_err,
    })?;

    for entry in fs::read_dir(source).map_err(|source_err| XtaskError::ReadFile {
        path: source.display().to_string(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| XtaskError::ReadFile {
            path: source.display().to_string(),
            source: source_err,
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            copy_file(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_typst_package_version;

    #[test]
    fn typst_package_version_accepts_numeric_triplets() {
        assert!(is_typst_package_version("0.8.0"));
        assert!(is_typst_package_version("10.20.30"));
    }

    #[test]
    fn typst_package_version_rejects_prerelease_forms() {
        assert!(!is_typst_package_version("0.8.0-alpha.1"));
        assert!(!is_typst_package_version("0.8.0a1"));
        assert!(!is_typst_package_version("0.8"));
        assert!(!is_typst_package_version("0.8.0.1"));
    }
}
