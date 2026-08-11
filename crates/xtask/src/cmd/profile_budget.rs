use super::wasm_module_surface::{
    LoadedWasmModule, WasmExport, WasmImport, WasmModuleLoadError, WasmSurfaceProfile,
};
use crate::XtaskError;
use std::path::{Path, PathBuf};

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
    Imports,
    Exports,
    Wasm,
}

#[derive(Debug, Default)]
struct ProfileBudgetOptions {
    check: Option<CheckKind>,
    profile: Option<Profile>,
    wasm_file: Option<PathBuf>,
}

pub(crate) fn profile_budget(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let check = options.check.ok_or(XtaskError::Usage)?;
    let profile = options.profile.ok_or(XtaskError::Usage)?;

    let mut failures = Vec::new();
    let wasm_file = options.wasm_file.as_deref().ok_or(XtaskError::Usage)?;
    let wasm = load_wasm_module(wasm_file)?;

    if matches!(check, CheckKind::Imports | CheckKind::Wasm) {
        let surface = wasm.surface();
        let import_failures = surface.validate_imports(profile.surface_profile());
        print_import_report(profile, surface.imports(), &import_failures);
        failures.extend(import_failures);
    }

    if matches!(check, CheckKind::Exports | CheckKind::Wasm) {
        let surface = wasm.surface();
        let export_failures = surface.validate_exports(profile.surface_profile());
        print_export_report(profile, surface.exports(), &export_failures);
        failures.extend(export_failures);
    }

    print_size_report(wasm_file)?;

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
            _ => {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    if options.check.is_none() || options.wasm_file.is_none() {
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
    println!("  check-wasm       check imports, exports, and print size");
    println!();
    println!("Profiles:");
    println!("  pure-wasm        no imports are allowed");
    println!("  typst-wasm       only wasm-minimal-protocol typst_env imports are allowed");
    println!();
    println!("WebAssembly input:");
    println!("  --wasm <module.wasm>");
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
    fn dependency_checks_are_owned_by_the_artifact_closure_verifier() {
        let error = parse_options(vec![
            "check-deps".to_string(),
            "--profile".to_string(),
            "typst-wasm".to_string(),
            "--artifact-profile".to_string(),
            "typst-wasm".to_string(),
        ])
        .unwrap_err();

        assert!(matches!(error, XtaskError::Usage));
    }

    #[test]
    fn wasm_surface_checks_keep_their_narrow_interface() {
        let options = parse_options(vec![
            "check-wasm".to_string(),
            "--profile".to_string(),
            "typst-wasm".to_string(),
            "--wasm".to_string(),
            "plugin.wasm".to_string(),
        ])
        .expect("WASM surface options");

        assert_eq!(options.check, Some(CheckKind::Wasm));
        assert_eq!(options.profile, Some(Profile::Typst));
        assert_eq!(options.wasm_file, Some(PathBuf::from("plugin.wasm")));
    }
}
