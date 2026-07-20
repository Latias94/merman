use super::wasm_module_surface::{
    LoadedWasmModule, WasmExport, WasmImport, WasmModuleLoadError, WasmSurfaceProfile,
};
use crate::XtaskError;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    PureWasm,
    Typst,
}

impl Profile {
    fn parse(raw: &str) -> Result<Self, XtaskError> {
        match raw {
            "pure" | "pure-wasm" => Ok(Self::PureWasm),
            "typst" | "typst-wasm" => Ok(Self::Typst),
            _ => Err(XtaskError::Usage),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::PureWasm => "pure-wasm",
            Self::Typst => "typst-wasm",
        }
    }

    const fn surface_profile(self) -> WasmSurfaceProfile {
        match self {
            Self::PureWasm => WasmSurfaceProfile::PureWasm,
            Self::Typst => WasmSurfaceProfile::Typst,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckKind {
    Deps,
    Imports,
    Exports,
    Wasm,
}

#[derive(Debug, Default)]
struct ProfileBudgetOptions {
    check: Option<CheckKind>,
    profile: Option<Profile>,
    wasm_file: Option<PathBuf>,
    tree_file: Option<PathBuf>,
    package: Option<String>,
    target: Option<String>,
    no_default_features: bool,
    features: Option<String>,
    depth: Option<usize>,
    extra_forbidden: Vec<String>,
}

pub(crate) fn profile_budget(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let check = options.check.ok_or(XtaskError::Usage)?;
    let profile = options.profile.ok_or(XtaskError::Usage)?;

    let mut failures = Vec::new();
    let wasm = if matches!(
        check,
        CheckKind::Imports | CheckKind::Exports | CheckKind::Wasm
    ) {
        let wasm_file = options.wasm_file.as_deref().ok_or(XtaskError::Usage)?;
        Some(load_wasm_module(wasm_file)?)
    } else {
        None
    };

    if matches!(check, CheckKind::Deps) {
        let tree = load_cargo_tree(&options)?;
        let dep_failures = check_deps(profile, &tree, &options.extra_forbidden);
        print_dep_report(profile, &dep_failures);
        failures.extend(dep_failures);
    }

    if matches!(check, CheckKind::Imports | CheckKind::Wasm) {
        let surface = wasm.as_ref().expect("WASM checks load a module").surface();
        let import_failures = surface.validate_imports(profile.surface_profile());
        print_import_report(profile, surface.imports(), &import_failures);
        failures.extend(import_failures);
    }

    if matches!(check, CheckKind::Exports | CheckKind::Wasm) {
        let surface = wasm.as_ref().expect("WASM checks load a module").surface();
        let export_failures = surface.validate_exports(profile.surface_profile());
        print_export_report(profile, surface.exports(), &export_failures);
        failures.extend(export_failures);
    }

    if let Some(wasm_file) = options.wasm_file.as_deref() {
        print_size_report(wasm_file)?;
    }

    if failures.is_empty() {
        println!("profile-budget OK profile={}", profile.label());
        Ok(())
    } else {
        Err(XtaskError::ProfileBudgetFailed(failures.join("\n")))
    }
}

fn parse_options(args: Vec<String>) -> Result<ProfileBudgetOptions, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return Err(XtaskError::Usage);
    }

    let mut options = ProfileBudgetOptions::default();
    let mut iter = args.into_iter();
    let Some(action) = iter.next() else {
        print_usage();
        return Err(XtaskError::Usage);
    };
    options.check = Some(match action.as_str() {
        "check-imports" => CheckKind::Imports,
        "check-exports" => CheckKind::Exports,
        "check-wasm" => CheckKind::Wasm,
        "check-deps" => CheckKind::Deps,
        _ => {
            print_usage();
            return Err(XtaskError::Usage);
        }
    });

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" => {
                let raw = iter.next().ok_or(XtaskError::Usage)?;
                options.profile = Some(Profile::parse(&raw)?);
            }
            "--wasm" => {
                let path = iter.next().ok_or(XtaskError::Usage)?;
                options.wasm_file = Some(PathBuf::from(path));
            }
            "--tree-file" => {
                let path = iter.next().ok_or(XtaskError::Usage)?;
                options.tree_file = Some(PathBuf::from(path));
            }
            "--package" | "-p" => {
                options.package = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--target" => {
                options.target = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--no-default-features" => {
                options.no_default_features = true;
            }
            "--features" => {
                options.features = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--depth" => {
                let raw = iter.next().ok_or(XtaskError::Usage)?;
                options.depth = Some(raw.parse().map_err(|_| XtaskError::Usage)?);
            }
            "--forbid" => {
                options
                    .extra_forbidden
                    .push(iter.next().ok_or(XtaskError::Usage)?);
            }
            _ => {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    match options.check {
        Some(CheckKind::Deps) => {
            if options.tree_file.is_some() == options.package.is_some() {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
        Some(CheckKind::Imports | CheckKind::Exports | CheckKind::Wasm) => {
            if options.wasm_file.is_none() {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
        None => return Err(XtaskError::Usage),
    }

    if options.tree_file.is_some() && options.package.is_some() {
        print_usage();
        return Err(XtaskError::Usage);
    }

    Ok(options)
}

fn print_usage() {
    println!("usage: xtask profile-budget <check> --profile <profile> <input>");
    println!();
    println!("Checks:");
    println!("  check-imports    check WASM import allowlist");
    println!("  check-exports    check WASM exports required by the profile");
    println!("  check-wasm       check imports, exports, and print size when --wasm is used");
    println!("  check-deps       check cargo tree dependency allowlist");
    println!();
    println!("Profiles:");
    println!("  pure-wasm        no imports are allowed");
    println!("  typst-wasm       only wasm-minimal-protocol typst_env imports are allowed");
    println!();
    println!("Dependency input:");
    println!("  --tree-file <path>");
    println!(
        "  --package <name> [--target <triple>] [--no-default-features] [--features <features>] [--depth <n>]"
    );
    println!();
    println!("WebAssembly input:");
    println!("  --wasm <module.wasm>");
}

fn load_cargo_tree(options: &ProfileBudgetOptions) -> Result<String, XtaskError> {
    if let Some(path) = options.tree_file.as_deref() {
        return crate::util::read_text(path);
    }

    cargo_tree(options)
}

fn load_wasm_module(path: &Path) -> Result<LoadedWasmModule, XtaskError> {
    LoadedWasmModule::from_file(path).map_err(|error| match error {
        WasmModuleLoadError::Read { path, source } => XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        },
        WasmModuleLoadError::Compile { path, message } => XtaskError::ProfileBudgetFailed(format!(
            "failed to load WebAssembly module {}: {message}",
            path.display()
        )),
    })
}

fn cargo_tree(options: &ProfileBudgetOptions) -> Result<String, XtaskError> {
    let package = options.package.as_deref().ok_or(XtaskError::Usage)?;
    let mut command = Command::new("cargo");
    command
        .arg("tree")
        .arg("-p")
        .arg(package)
        .arg("-e")
        .arg("normal");
    if let Some(target) = options.target.as_deref() {
        command.arg("--target").arg(target);
    }
    if options.no_default_features {
        command.arg("--no-default-features");
    }
    if let Some(features) = options.features.as_deref() {
        command.arg("--features").arg(features);
    }
    if let Some(depth) = options.depth {
        command.arg("--depth").arg(depth.to_string());
    }

    let output = command
        .current_dir(crate::cmd::workspace_root())
        .output()
        .map_err(|source| {
            XtaskError::ProfileBudgetFailed(format!("failed to spawn cargo tree: {source}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(XtaskError::ProfileBudgetFailed(format!(
            "cargo tree for package {package} exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    String::from_utf8(output.stdout).map_err(|source| {
        XtaskError::ProfileBudgetFailed(format!("cargo tree output was not UTF-8: {source}"))
    })
}

fn check_deps(profile: Profile, tree: &str, extra_forbidden: &[String]) -> Vec<String> {
    let mut forbidden = forbidden_crates(profile)
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    forbidden.extend(extra_forbidden.iter().cloned());
    forbidden.sort();
    forbidden.dedup();

    forbidden
        .into_iter()
        .filter(|krate| cargo_tree_contains_crate(tree, krate))
        .map(|krate| {
            format!(
                "{} profile forbids dependency `{krate}` in cargo tree",
                profile.label()
            )
        })
        .collect()
}

fn forbidden_crates(profile: Profile) -> &'static [&'static str] {
    match profile {
        Profile::PureWasm | Profile::Typst => &[
            "console_error_panic_hook",
            "getrandom",
            "json5",
            "js-sys",
            "lol_html",
            "pest",
            "serde-wasm-bindgen",
            "serde_yaml",
            "unsafe-libyaml",
            "url",
            "wasm-bindgen",
            "wasm-bindgen-futures",
            "web-time",
        ],
    }
}

fn cargo_tree_contains_crate(tree: &str, krate: &str) -> bool {
    tree.lines()
        .map(cargo_tree_line_payload)
        .any(|line| line == krate || line.starts_with(&format!("{krate} ")))
}

fn cargo_tree_line_payload(line: &str) -> &str {
    line.trim_start_matches(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '│' | '├' | '└' | '─' | '┬' | '┼' | '┌' | '┐' | '┘' | '┴' | '┤' | '╰'
            )
    })
}

fn print_dep_report(profile: Profile, failures: &[String]) {
    println!(
        "profile-budget deps profile={} failures={}",
        profile.label(),
        failures.len()
    );
    for failure in failures {
        println!("  {failure}");
    }
}

fn print_import_report(profile: Profile, imports: &[WasmImport], failures: &[String]) {
    println!(
        "profile-budget imports profile={} imports={} failures={}",
        profile.label(),
        imports.len(),
        failures.len()
    );
    for import in imports {
        println!("  import {}::{}", import.module(), import.name());
    }
}

fn print_export_report(profile: Profile, exports: &[WasmExport], failures: &[String]) {
    println!(
        "profile-budget exports profile={} exports={} failures={}",
        profile.label(),
        exports.len(),
        failures.len()
    );
    for export in exports {
        println!("  export {:?} {}", export.ty(), export.name());
    }
}

fn print_size_report(wasm_file: &Path) -> Result<(), XtaskError> {
    let size = std::fs::metadata(wasm_file)
        .map_err(|source| XtaskError::ReadFile {
            path: wasm_file.display().to_string(),
            source,
        })?
        .len();
    println!(
        "profile-budget size path={} bytes={size}",
        wasm_file.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_profile_rejects_forbidden_dependencies() {
        let tree = r#"
merman-core v0.7.0
├── chrono v0.4.43
├── js-sys v0.3.85
├── serde_yaml v0.9.34+deprecated
├── lol_html v2.7.1
└── web-time v1.1.0
"#;

        let failures = check_deps(Profile::PureWasm, tree, &[]);

        assert_eq!(failures.len(), 4);
        assert!(failures.iter().any(|failure| failure.contains("`js-sys`")));
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("`lol_html`"))
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("`serde_yaml`"))
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("`web-time`"))
        );
    }

    #[test]
    fn dependency_gate_accepts_profile_safe_tree() {
        let tree = r#"
merman-core v0.7.0
├── chrono v0.4.43
└── serde_json v1.0.149
"#;

        assert!(check_deps(Profile::PureWasm, tree, &[]).is_empty());
    }
}
