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
    let wasm_path = root
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("wasm-size")
        .join("merman_typst_plugin.wasm");

    if !options.skip_wasm_build {
        build_wasm(options.profile)?;
    }

    if !wasm_path.exists() {
        return Err(XtaskError::TypstPackageFailed(format!(
            "missing wasm artifact: {}\nrun without --skip-wasm-build first",
            wasm_path.display()
        )));
    }

    let package_dir = options
        .out_dir
        .join("merman")
        .join(env!("CARGO_PKG_VERSION"));
    fs::create_dir_all(&package_dir).map_err(|source| XtaskError::WriteFile {
        path: package_dir.display().to_string(),
        source,
    })?;

    copy_file(
        &package_source.join("typst.toml"),
        &package_dir.join("typst.toml"),
    )?;
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
        env!("CARGO_PKG_VERSION")
    );
    println!("Tip: run `typst info` to find your Typst package path.");

    Ok(())
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

fn build_wasm(profile: TypstBuildProfile) -> Result<(), XtaskError> {
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
