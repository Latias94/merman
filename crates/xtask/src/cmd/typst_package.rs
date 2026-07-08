use crate::{XtaskError, cmd::paths};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypstBuildProfile {
    Minimal,
    Full,
    FullElk,
}

impl TypstBuildProfile {
    fn parse(raw: &str) -> Result<Self, XtaskError> {
        match raw {
            "minimal" | "default" => Ok(Self::Minimal),
            "full" | "core-full" => Ok(Self::Full),
            "full-elk" | "elk" | "publish" | "default-publish" => Ok(Self::FullElk),
            _ => Err(XtaskError::Usage),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Full => "full",
            Self::FullElk => "full-elk",
        }
    }
}

#[derive(Debug)]
struct Options {
    profile: TypstBuildProfile,
    out_dir: PathBuf,
    skip_wasm_build: bool,
}

#[derive(Debug)]
struct SmokeOptions {
    build: Options,
    compile_examples: bool,
    compile_tests: bool,
    keep_artifacts: bool,
    typst: Option<PathBuf>,
}

#[derive(Debug)]
struct TypstFixture {
    input: PathBuf,
    output: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
struct TypstManifest {
    package: TypstManifestPackage,
}

#[derive(Debug, serde::Deserialize)]
struct TypstManifestPackage {
    version: String,
}

#[derive(Debug, serde::Deserialize)]
struct CargoManifest {
    workspace: CargoWorkspace,
}

#[derive(Debug, serde::Deserialize)]
struct CargoWorkspace {
    package: CargoWorkspacePackage,
}

#[derive(Debug, serde::Deserialize)]
struct CargoWorkspacePackage {
    version: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            profile: TypstBuildProfile::FullElk,
            out_dir: paths::workspace_root().join("dist").join("typst"),
            skip_wasm_build: false,
        }
    }
}

pub(crate) fn build_typst_package(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let package_dir = build_typst_package_with_options(&options)?;

    println_package_build_summary(options.profile, &package_dir);
    Ok(())
}

pub(crate) fn typst_package_smoke(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_smoke_options(args)?;
    let package_dir = build_typst_package_with_options(&options.build)?;
    println_package_build_summary(options.build.profile, &package_dir);

    let typst = find_typst_command(options.typst.as_deref())?;
    let root = paths::workspace_root();
    let manifest_path = root
        .join("packages")
        .join("typst")
        .join("merman")
        .join("typst.toml");
    let package_version = read_typst_package_version(&manifest_path)?;
    let smoke_root = paths::target_root().join("typst-package-smoke");
    let package_path = smoke_root.join("packages");
    let preview_dir = package_path
        .join("preview")
        .join("merman")
        .join(&package_version);
    let output_dir = smoke_root.join("out");

    if smoke_root.exists() {
        fs::remove_dir_all(&smoke_root).map_err(|source| XtaskError::WriteFile {
            path: smoke_root.display().to_string(),
            source,
        })?;
    }
    copy_dir_recursive(&package_dir, &preview_dir)?;
    fs::create_dir_all(&output_dir).map_err(|source| XtaskError::WriteFile {
        path: output_dir.display().to_string(),
        source,
    })?;

    let mut fixtures = Vec::new();
    if options.compile_examples {
        collect_typst_fixtures(
            &package_dir.join("examples"),
            &output_dir.join("examples"),
            &mut fixtures,
        )?;
    }
    if options.compile_tests {
        collect_typst_fixtures(
            &root
                .join("packages")
                .join("typst")
                .join("merman")
                .join("tests"),
            &output_dir.join("tests"),
            &mut fixtures,
        )?;
    }
    fixtures.sort_by(|left, right| left.input.cmp(&right.input));

    if fixtures.is_empty() {
        return Err(XtaskError::TypstPackageSmokeFailed(
            "no Typst examples or tests were found to compile".to_string(),
        ));
    }

    let mut compiled = 0usize;
    for fixture in fixtures {
        if let Err(error) = compile_typst_fixture(&typst, &package_path, &fixture) {
            println!(
                "typst-package-smoke artifacts kept at {} after failure",
                smoke_root.display()
            );
            return Err(error);
        }
        compiled += 1;
    }

    println!(
        "typst-package-smoke OK package={} compiled={compiled} package_path={}",
        package_dir.display(),
        package_path.display()
    );

    if !options.keep_artifacts {
        fs::remove_dir_all(&smoke_root).map_err(|source| XtaskError::WriteFile {
            path: smoke_root.display().to_string(),
            source,
        })?;
    }

    Ok(())
}

fn build_typst_package_with_options(options: &Options) -> Result<PathBuf, XtaskError> {
    let root = paths::workspace_root();
    let package_source = root.join("packages").join("typst").join("merman");
    let manifest_path = package_source.join("typst.toml");
    let package_version = read_typst_package_version(&manifest_path)?;
    verify_typst_readme_version_mapping(&package_source.join("README.md"), &package_version)?;
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

    let src_source = package_source.join("src");
    if src_source.exists() {
        copy_dir_recursive(&src_source, &package_dir.join("src"))?;
    }

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

    Ok(package_dir
        .canonicalize()
        .unwrap_or_else(|_| package_dir.clone()))
}

fn println_package_build_summary(profile: TypstBuildProfile, package_dir: &Path) {
    println!(
        "Typst package built profile={} path={}",
        profile.label(),
        package_dir.display()
    );
    println!(
        "Local install target: <typst package path>/local/merman/{}",
        package_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<version>")
    );
    println!(
        "Preview smoke target: <typst package path>/preview/merman/{}",
        package_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<version>")
    );
    println!("Tip: run `typst info` to find your Typst package path.");
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

fn read_workspace_version(manifest_path: &Path) -> Result<String, XtaskError> {
    let manifest_text =
        fs::read_to_string(manifest_path).map_err(|source| XtaskError::ReadFile {
            path: manifest_path.display().to_string(),
            source,
        })?;
    let manifest: CargoManifest = toml::from_str(&manifest_text).map_err(|source| {
        XtaskError::TypstPackageFailed(format!(
            "failed to parse {}: {source}",
            manifest_path.display()
        ))
    })?;
    Ok(manifest.workspace.package.version)
}

fn read_typst_plugin_abi_version(source_path: &Path) -> Result<String, XtaskError> {
    let source = fs::read_to_string(source_path).map_err(|source| XtaskError::ReadFile {
        path: source_path.display().to_string(),
        source,
    })?;
    let marker = "pub const TYPST_PLUGIN_ABI_VERSION: &str = \"";
    for line in source.lines() {
        let Some(rest) = line.trim().strip_prefix(marker) else {
            continue;
        };
        let Some(version) = rest.strip_suffix("\";") else {
            continue;
        };
        if !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit()) {
            return Ok(version.to_string());
        }
    }

    Err(XtaskError::TypstPackageFailed(format!(
        "{} must define numeric pub const TYPST_PLUGIN_ABI_VERSION",
        source_path.display()
    )))
}

fn verify_typst_readme_version_mapping(
    readme_path: &Path,
    package_version: &str,
) -> Result<(), XtaskError> {
    let readme = fs::read_to_string(readme_path).map_err(|source| XtaskError::ReadFile {
        path: readme_path.display().to_string(),
        source,
    })?;
    let workspace_version = read_workspace_version(&paths::workspace_root().join("Cargo.toml"))?;
    let plugin_abi = read_typst_plugin_abi_version(
        &paths::workspace_root()
            .join("crates")
            .join("merman-typst-plugin")
            .join("src")
            .join("lib.rs"),
    )?;
    let expected_row = format!("| `{package_version}` | `{workspace_version}` | `{plugin_abi}` |");
    if readme.contains(&expected_row) {
        return Ok(());
    }

    Err(XtaskError::TypstPackageFailed(format!(
        "{} version mapping must include Typst package, merman source version, and plugin ABI row `{expected_row}`",
        readme_path.display()
    )))
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
        "usage: xtask build-typst-package [--profile minimal|full|full-elk] [--out <dir>] [--skip-wasm-build]"
    );
}

fn parse_smoke_options(args: Vec<String>) -> Result<SmokeOptions, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_smoke_usage();
        return Err(XtaskError::Usage);
    }

    let mut options = SmokeOptions {
        build: Options::default(),
        compile_examples: true,
        compile_tests: true,
        keep_artifacts: false,
        typst: None,
    };

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" => {
                let raw = iter.next().ok_or(XtaskError::Usage)?;
                options.build.profile = TypstBuildProfile::parse(&raw)?;
            }
            "--out" => {
                options.build.out_dir = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--skip-wasm-build" => {
                options.build.skip_wasm_build = true;
            }
            "--examples-only" => {
                options.compile_examples = true;
                options.compile_tests = false;
            }
            "--tests-only" => {
                options.compile_examples = false;
                options.compile_tests = true;
            }
            "--keep-artifacts" => {
                options.keep_artifacts = true;
            }
            "--typst" => {
                options.typst = Some(PathBuf::from(iter.next().ok_or(XtaskError::Usage)?));
            }
            _ => {
                print_smoke_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    Ok(options)
}

fn print_smoke_usage() {
    println!(
        "usage: xtask typst-package-smoke [--profile minimal|full|full-elk] [--out <dir>] [--skip-wasm-build] [--examples-only|--tests-only] [--keep-artifacts] [--typst <path>]"
    );
}

fn find_typst_command(explicit: Option<&Path>) -> Result<PathBuf, XtaskError> {
    let typst = explicit.map_or_else(|| PathBuf::from("typst"), PathBuf::from);
    let status = Command::new(&typst)
        .arg("--version")
        .status()
        .map_err(|source| {
            XtaskError::TypstPackageSmokeFailed(format!(
                "failed to execute `{}` --version: {source}",
                typst.display()
            ))
        })?;
    if status.success() {
        Ok(typst)
    } else {
        Err(XtaskError::TypstPackageSmokeFailed(format!(
            "`{}` --version failed with status {status}",
            typst.display()
        )))
    }
}

fn collect_typst_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), XtaskError> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|source| XtaskError::ReadFile {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_typst_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("typ") {
            out.push(path);
        }
    }

    Ok(())
}

fn collect_typst_fixtures(
    input_root: &Path,
    output_root: &Path,
    out: &mut Vec<TypstFixture>,
) -> Result<(), XtaskError> {
    let mut inputs = Vec::new();
    collect_typst_files(input_root, &mut inputs)?;
    for input in inputs {
        let output = typst_fixture_output_path(input_root, output_root, &input)?;
        out.push(TypstFixture { input, output });
    }
    Ok(())
}

fn typst_fixture_output_path(
    input_root: &Path,
    output_root: &Path,
    input: &Path,
) -> Result<PathBuf, XtaskError> {
    let relative = input.strip_prefix(input_root).map_err(|_| {
        XtaskError::TypstPackageSmokeFailed(format!(
            "fixture {} is outside input root {}",
            input.display(),
            input_root.display()
        ))
    })?;
    Ok(output_root.join(relative).with_extension("pdf"))
}

fn compile_typst_fixture(
    typst: &Path,
    package_path: &Path,
    fixture: &TypstFixture,
) -> Result<(), XtaskError> {
    if let Some(parent) = fixture.output.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let status = Command::new(typst)
        .args(["compile", "--package-path"])
        .arg(package_path)
        .arg(&fixture.input)
        .arg(&fixture.output)
        .status()
        .map_err(|source| {
            XtaskError::TypstPackageSmokeFailed(format!(
                "failed to compile {}: {source}",
                fixture.input.display()
            ))
        })?;

    if status.success() {
        println!(
            "compiled Typst fixture {} -> {}",
            fixture.input.display(),
            fixture.output.display()
        );
        return Ok(());
    }

    Err(XtaskError::TypstPackageSmokeFailed(format!(
        "typst compile failed for {} with status {status}",
        fixture.input.display()
    )))
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
    match profile {
        TypstBuildProfile::Minimal => {
            command
                .arg("--no-default-features")
                .args(["--features", "render,analysis"]);
        }
        TypstBuildProfile::Full => {
            command
                .arg("--no-default-features")
                .args(["--features", "render,analysis,core-full"]);
        }
        TypstBuildProfile::FullElk => {
            command
                .arg("--no-default-features")
                .args(["--features", "render,analysis,core-full,elk-layout"]);
        }
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
    use super::{
        collect_typst_files, collect_typst_fixtures, copy_dir_recursive, is_typst_package_version,
        parse_smoke_options, read_typst_plugin_abi_version, read_workspace_version,
        typst_fixture_output_path, verify_typst_readme_version_mapping,
    };
    use crate::cmd::paths;
    use std::fs;
    use std::path::{Path, PathBuf};

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

    #[test]
    fn collect_typst_files_ignores_missing_directories() {
        let mut out = Vec::new();
        collect_typst_files(Path::new("target/definitely-missing-typst-dir"), &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn smoke_options_accept_explicit_typst_binary() {
        let options = parse_smoke_options(vec![
            "--skip-wasm-build".to_string(),
            "--typst".to_string(),
            "target/typst-local/typst.exe".to_string(),
        ])
        .unwrap();

        assert_eq!(
            options.typst.as_deref(),
            Some(Path::new("target/typst-local/typst.exe"))
        );
        assert!(options.build.skip_wasm_build);
    }

    #[test]
    fn typst_fixture_output_path_preserves_relative_directories() {
        let root = Path::new("tests");
        let out = Path::new("out");

        let api = typst_fixture_output_path(root, out, Path::new("tests/api/test.typ")).unwrap();
        let context =
            typst_fixture_output_path(root, out, Path::new("tests/context/test.typ")).unwrap();

        assert_eq!(api, PathBuf::from("out/api/test.pdf"));
        assert_eq!(context, PathBuf::from("out/context/test.pdf"));
        assert_ne!(api, context);
    }

    #[test]
    fn collect_typst_fixtures_keeps_nested_outputs_distinct() {
        let root = unique_test_dir("typst-fixtures");
        let input_root = root.join("tests");
        let output_root = root.join("out");
        fs::create_dir_all(input_root.join("api")).unwrap();
        fs::create_dir_all(input_root.join("context")).unwrap();
        fs::write(input_root.join("api").join("test.typ"), "").unwrap();
        fs::write(input_root.join("context").join("test.typ"), "").unwrap();

        let mut fixtures = Vec::new();
        collect_typst_fixtures(&input_root, &output_root, &mut fixtures).unwrap();
        fixtures.sort_by(|left, right| left.output.cmp(&right.output));

        assert_eq!(fixtures.len(), 2);
        assert_eq!(fixtures[0].output, output_root.join("api").join("test.pdf"));
        assert_eq!(
            fixtures[1].output,
            output_root.join("context").join("test.pdf")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copy_dir_recursive_copies_source_modules() {
        let root = unique_test_dir("typst-copy-src");
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(source.join("src").join("nested")).unwrap();
        fs::write(source.join("src").join("exports.typ"), "#let ok = true").unwrap();
        fs::write(
            source.join("src").join("nested").join("module.typ"),
            "#let nested = true",
        )
        .unwrap();

        copy_dir_recursive(&source.join("src"), &destination.join("src")).unwrap();

        assert!(destination.join("src").join("exports.typ").exists());
        assert!(
            destination
                .join("src")
                .join("nested")
                .join("module.typ")
                .exists()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typst_readme_version_mapping_matches_package_and_workspace_versions() {
        let readme = paths::workspace_root()
            .join("packages")
            .join("typst")
            .join("merman")
            .join("README.md");
        verify_typst_readme_version_mapping(&readme, "0.1.0").unwrap();
    }

    #[test]
    fn typst_plugin_abi_version_reads_public_const() {
        let root = unique_test_dir("typst-plugin-abi");
        let source = root.join("lib.rs");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &source,
            "pub const TYPST_PLUGIN_ABI_VERSION: &str = \"7\";\n",
        )
        .unwrap();

        assert_eq!(read_typst_plugin_abi_version(&source).unwrap(), "7");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typst_readme_version_mapping_rejects_missing_package_version() {
        let root = unique_test_dir("typst-readme-version");
        let readme = root.join("README.md");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &readme,
            "| Typst package | merman source version | Notes |\n| --- | --- | --- |\n| `9.9.9` | `0.8.0-alpha.2` | stale |\n",
        )
        .unwrap();

        let error = verify_typst_readme_version_mapping(&readme, "0.1.0").unwrap_err();
        assert!(
            error.to_string().contains("version mapping must include"),
            "unexpected error: {error}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typst_readme_version_mapping_rejects_missing_plugin_abi() {
        let root = unique_test_dir("typst-readme-plugin-abi");
        let readme = root.join("README.md");
        let workspace_version =
            read_workspace_version(&paths::workspace_root().join("Cargo.toml")).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &readme,
            format!(
                "| Typst package | merman source version | Notes |\n| --- | --- | --- |\n| `0.1.0` | `{workspace_version}` | stale |\n"
            ),
        )
        .unwrap();

        let error = verify_typst_readme_version_mapping(&readme, "0.1.0").unwrap_err();
        assert!(
            error.to_string().contains("plugin ABI"),
            "unexpected error: {error}"
        );

        let _ = fs::remove_dir_all(root);
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("merman-{name}-{pid}-{nanos}"))
    }
}
