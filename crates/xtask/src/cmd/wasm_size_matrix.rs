use crate::{XtaskError, cmd::paths};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    Browser,
    Typst,
}

impl Surface {
    fn parse(raw: &str) -> Result<Self, XtaskError> {
        match raw {
            "browser" | "web" | "wasm" => Ok(Self::Browser),
            "typst" => Ok(Self::Typst),
            _ => Err(XtaskError::Usage),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Typst => "typst",
        }
    }
}

#[derive(Debug, Default)]
struct Options {
    surface: Option<Surface>,
    preset: Option<String>,
    no_strip: bool,
}

#[derive(Debug, Clone, Copy)]
struct WasmPreset {
    name: &'static str,
    surface: Surface,
    package: &'static str,
    artifact_name: &'static str,
    no_default_features: bool,
    features: &'static [&'static str],
}

const PRESETS: &[WasmPreset] = &[
    WasmPreset {
        name: "browser-core",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: true,
        features: &[],
    },
    WasmPreset {
        name: "browser-render",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: true,
        features: &["render"],
    },
    WasmPreset {
        name: "browser-ascii",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: true,
        features: &["ascii"],
    },
    WasmPreset {
        name: "browser-full",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: false,
        features: &[],
    },
    WasmPreset {
        name: "browser-ratex-math",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: false,
        features: &["ratex-math"],
    },
    WasmPreset {
        name: "typst-bridge",
        surface: Surface::Typst,
        package: "merman-typst-plugin",
        artifact_name: "merman_typst_plugin.wasm",
        no_default_features: true,
        features: &[],
    },
    WasmPreset {
        name: "typst-render",
        surface: Surface::Typst,
        package: "merman-typst-plugin",
        artifact_name: "merman_typst_plugin.wasm",
        no_default_features: false,
        features: &[],
    },
    WasmPreset {
        name: "typst-core-full",
        surface: Surface::Typst,
        package: "merman-typst-plugin",
        artifact_name: "merman_typst_plugin.wasm",
        no_default_features: false,
        features: &["core-full"],
    },
    WasmPreset {
        name: "typst-ratex-math",
        surface: Surface::Typst,
        package: "merman-typst-plugin",
        artifact_name: "merman_typst_plugin.wasm",
        no_default_features: false,
        features: &["ratex-math"],
    },
];

pub(crate) fn wasm_size_matrix(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let presets = selected_presets(&options)?;
    let strip_dir = paths::target_root().join("wasm-size-matrix");
    if !options.no_strip {
        fs::create_dir_all(&strip_dir).map_err(|source| XtaskError::WriteFile {
            path: strip_dir.display().to_string(),
            source,
        })?;
    }

    println!(
        "wasm-size-matrix columns=surface,preset,package,default_features,features,raw_bytes,stripped_bytes,artifact"
    );

    for preset in presets {
        build_preset(preset)?;
        let artifact_path = artifact_path(preset);
        let raw_bytes = file_size(&artifact_path)?;
        let stripped_bytes = if options.no_strip {
            None
        } else {
            Some(strip_copy(preset, &artifact_path, &strip_dir)?)
        };
        let display_artifact = artifact_path
            .canonicalize()
            .unwrap_or_else(|_| artifact_path.clone());

        println!(
            "wasm-size-matrix surface={} preset={} package={} default_features={} features={} raw_bytes={} stripped_bytes={} artifact={}",
            preset.surface.label(),
            preset.name,
            preset.package,
            !preset.no_default_features,
            feature_label(preset.features),
            raw_bytes,
            stripped_bytes
                .map(|bytes| bytes.to_string())
                .unwrap_or_else(|| "skipped".to_string()),
            display_artifact.display()
        );
    }

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
            "--surface" => {
                let raw = iter.next().ok_or(XtaskError::Usage)?;
                options.surface = if raw == "all" {
                    None
                } else {
                    Some(Surface::parse(&raw)?)
                };
            }
            "--preset" => {
                options.preset = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--no-strip" => {
                options.no_strip = true;
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
        "usage: xtask wasm-size-matrix [--surface browser|typst|all] [--preset <name>] [--no-strip]"
    );
    println!();
    println!("Presets:");
    for preset in PRESETS {
        println!(
            "  {:<18} surface={} package={} default_features={} features={}",
            preset.name,
            preset.surface.label(),
            preset.package,
            !preset.no_default_features,
            feature_label(preset.features)
        );
    }
}

fn selected_presets(options: &Options) -> Result<Vec<&'static WasmPreset>, XtaskError> {
    let presets = PRESETS
        .iter()
        .filter(|preset| {
            options
                .surface
                .is_none_or(|surface| preset.surface == surface)
        })
        .filter(|preset| {
            options
                .preset
                .as_deref()
                .is_none_or(|name| preset.name == name)
        })
        .collect::<Vec<_>>();

    if presets.is_empty() {
        return Err(XtaskError::WasmSizeMatrixFailed(
            "no wasm size presets matched the requested filters".to_string(),
        ));
    }

    Ok(presets)
}

fn build_preset(preset: &WasmPreset) -> Result<(), XtaskError> {
    let mut command = Command::new("cargo");
    command.args([
        "build",
        "-p",
        preset.package,
        "--profile",
        "wasm-size",
        "--target",
        "wasm32-unknown-unknown",
    ]);

    if preset.no_default_features {
        command.arg("--no-default-features");
    }

    let features = preset.features.join(",");
    if !features.is_empty() {
        command.arg("--features").arg(&features);
    }

    let status = command
        .current_dir(paths::workspace_root())
        .status()
        .map_err(|source| XtaskError::ReadFile {
            path: "cargo".to_string(),
            source,
        })?;

    if !status.success() {
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "cargo build failed for preset {} with status {status}",
            preset.name
        )));
    }

    Ok(())
}

fn artifact_path(preset: &WasmPreset) -> PathBuf {
    paths::target_root()
        .join("wasm32-unknown-unknown")
        .join("wasm-size")
        .join(preset.artifact_name)
}

fn strip_copy(preset: &WasmPreset, wasm_path: &Path, strip_dir: &Path) -> Result<u64, XtaskError> {
    let stripped_path = strip_dir.join(format!("{}.stripped.wasm", preset.name));
    let status = Command::new("wasm-tools")
        .args(["strip", "--all"])
        .arg(wasm_path)
        .arg("-o")
        .arg(&stripped_path)
        .current_dir(paths::workspace_root())
        .status()
        .map_err(|source| XtaskError::ReadFile {
            path: "wasm-tools".to_string(),
            source,
        })?;

    if !status.success() {
        return Err(XtaskError::WasmSizeMatrixFailed(format!(
            "wasm-tools strip failed for preset {} with status {status}",
            preset.name
        )));
    }

    file_size(&stripped_path)
}

fn file_size(path: &Path) -> Result<u64, XtaskError> {
    fs::metadata(path)
        .map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })
        .map(|metadata| metadata.len())
}

fn feature_label(features: &[&str]) -> String {
    if features.is_empty() {
        "none".to_string()
    } else {
        features.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection_includes_browser_and_typst_surfaces() {
        let options = Options::default();
        let presets = selected_presets(&options).unwrap();

        assert!(presets.iter().any(|preset| preset.name == "browser-full"));
        assert!(presets.iter().any(|preset| preset.name == "typst-render"));
    }

    #[test]
    fn surface_filter_selects_only_that_surface() {
        let options = Options {
            surface: Some(Surface::Typst),
            preset: None,
            no_strip: false,
        };
        let presets = selected_presets(&options).unwrap();

        assert!(!presets.is_empty());
        assert!(
            presets
                .iter()
                .all(|preset| preset.surface == Surface::Typst)
        );
    }

    #[test]
    fn preset_filter_selects_one_named_preset() {
        let options = Options {
            surface: None,
            preset: Some("browser-render".to_string()),
            no_strip: false,
        };
        let presets = selected_presets(&options).unwrap();

        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "browser-render");
        assert_eq!(presets[0].features, &["render"]);
        assert!(presets[0].no_default_features);
    }

    #[test]
    fn unmatched_filters_are_errors() {
        let options = Options {
            surface: Some(Surface::Typst),
            preset: Some("browser-render".to_string()),
            no_strip: false,
        };

        assert!(selected_presets(&options).is_err());
    }

    #[test]
    fn option_parser_accepts_surface_preset_and_no_strip() {
        let options = parse_options(vec![
            "--surface".to_string(),
            "browser".to_string(),
            "--preset".to_string(),
            "browser-core".to_string(),
            "--no-strip".to_string(),
        ])
        .unwrap();

        assert_eq!(options.surface, Some(Surface::Browser));
        assert_eq!(options.preset.as_deref(), Some("browser-core"));
        assert!(options.no_strip);
    }

    #[test]
    fn feature_label_uses_none_for_empty_features() {
        assert_eq!(feature_label(&[]), "none");
        assert_eq!(
            feature_label(&["render", "ratex-math"]),
            "render+ratex-math"
        );
    }
}
