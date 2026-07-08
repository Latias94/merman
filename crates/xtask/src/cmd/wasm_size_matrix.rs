use crate::{XtaskError, cmd::paths};
use flate2::{Compression, write::GzEncoder};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
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
    budget_file: Option<PathBuf>,
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

#[derive(Debug)]
struct WasmMeasurement {
    raw_bytes: u64,
    stripped_bytes: Option<u64>,
    gzip_bytes: u64,
    brotli_bytes: u64,
    artifact_path: PathBuf,
    compressed_source: CompressionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionSource {
    Raw,
    Stripped,
}

impl CompressionSource {
    const fn label(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Stripped => "stripped",
        }
    }
}

#[derive(Debug, Deserialize)]
struct WasmSizeBudgets {
    presets: BTreeMap<String, WasmPresetBudget>,
}

#[derive(Debug, Default, Deserialize)]
struct WasmPresetBudget {
    max_raw_bytes: Option<u64>,
    max_stripped_bytes: Option<u64>,
    max_gzip_bytes: Option<u64>,
    max_brotli_bytes: Option<u64>,
}

const PRESETS: &[WasmPreset] = &[
    WasmPreset {
        name: "browser-core",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: true,
        features: &["analysis"],
    },
    WasmPreset {
        name: "browser-render",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: true,
        features: &["render", "analysis"],
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
        name: "browser-full-no-elk",
        surface: Surface::Browser,
        package: "merman-wasm",
        artifact_name: "merman_wasm.wasm",
        no_default_features: true,
        features: &[
            "core-full",
            "core-host",
            "render",
            "analysis",
            "ascii",
            "editor-language",
        ],
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
        name: "typst-render-no-elk",
        surface: Surface::Typst,
        package: "merman-typst-plugin",
        artifact_name: "merman_typst_plugin.wasm",
        no_default_features: true,
        features: &["render", "analysis"],
    },
    WasmPreset {
        name: "typst-core-full-no-elk",
        surface: Surface::Typst,
        package: "merman-typst-plugin",
        artifact_name: "merman_typst_plugin.wasm",
        no_default_features: true,
        features: &["render", "analysis", "core-full"],
    },
    WasmPreset {
        name: "typst-full-elk",
        surface: Surface::Typst,
        package: "merman-typst-plugin",
        artifact_name: "merman_typst_plugin.wasm",
        no_default_features: false,
        features: &[],
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
    let budgets = options
        .budget_file
        .as_deref()
        .map(load_budget_file)
        .transpose()?;
    let strip_dir = paths::target_root().join("wasm-size-matrix");
    if !options.no_strip {
        fs::create_dir_all(&strip_dir).map_err(|source| XtaskError::WriteFile {
            path: strip_dir.display().to_string(),
            source,
        })?;
    }

    println!(
        "wasm-size-matrix columns=surface,preset,package,default_features,features,raw_bytes,stripped_bytes,gzip_bytes,brotli_bytes,compressed_source,artifact"
    );

    let mut budget_failures = Vec::new();

    for preset in presets {
        let measurement = measure_preset(preset, &strip_dir, options.no_strip)?;
        let display_artifact = measurement
            .artifact_path
            .canonicalize()
            .unwrap_or_else(|_| measurement.artifact_path.clone());

        println!(
            "wasm-size-matrix surface={} preset={} package={} default_features={} features={} raw_bytes={} stripped_bytes={} gzip_bytes={} brotli_bytes={} compressed_source={} artifact={}",
            preset.surface.label(),
            preset.name,
            preset.package,
            !preset.no_default_features,
            feature_label(preset.features),
            measurement.raw_bytes,
            measurement
                .stripped_bytes
                .map(|bytes| bytes.to_string())
                .unwrap_or_else(|| "skipped".to_string()),
            measurement.gzip_bytes,
            measurement.brotli_bytes,
            measurement.compressed_source.label(),
            display_artifact.display()
        );

        if let Some(budgets) = budgets.as_ref() {
            budget_failures.extend(check_budget(preset, &measurement, budgets));
        }
    }

    if budget_failures.is_empty() {
        if let Some(path) = options.budget_file.as_deref() {
            println!(
                "wasm-size-matrix budget_file={} result=ok",
                resolve_repo_path(path).display()
            );
        }
        Ok(())
    } else {
        Err(XtaskError::WasmSizeMatrixFailed(budget_failures.join("\n")))
    }
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
            "--budget-file" => {
                options.budget_file = Some(PathBuf::from(iter.next().ok_or(XtaskError::Usage)?));
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
        "usage: xtask wasm-size-matrix [--surface browser|typst|all] [--preset <name>] [--no-strip] [--budget-file <path>]"
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

fn measure_preset(
    preset: &WasmPreset,
    strip_dir: &Path,
    no_strip: bool,
) -> Result<WasmMeasurement, XtaskError> {
    build_preset(preset)?;
    let artifact_path = artifact_path(preset);
    let raw_bytes = file_size(&artifact_path)?;

    let (compressed_path, stripped_bytes, compressed_source) = if no_strip {
        (artifact_path.clone(), None, CompressionSource::Raw)
    } else {
        let stripped_path = strip_copy(preset, &artifact_path, strip_dir)?;
        let bytes = file_size(&stripped_path)?;
        (stripped_path, Some(bytes), CompressionSource::Stripped)
    };

    let gzip_bytes = gzip_size(&compressed_path)?;
    let brotli_bytes = brotli_size(&compressed_path)?;

    Ok(WasmMeasurement {
        raw_bytes,
        stripped_bytes,
        gzip_bytes,
        brotli_bytes,
        artifact_path,
        compressed_source,
    })
}

fn load_budget_file(path: &Path) -> Result<WasmSizeBudgets, XtaskError> {
    let path = resolve_repo_path(path);
    let text = crate::util::read_text(&path)?;
    serde_json::from_str(&text).map_err(XtaskError::Json)
}

fn resolve_repo_path(path: &Path) -> PathBuf {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        paths::workspace_root().join(path)
    };
    resolved.canonicalize().unwrap_or(resolved)
}

fn check_budget(
    preset: &WasmPreset,
    measurement: &WasmMeasurement,
    budgets: &WasmSizeBudgets,
) -> Vec<String> {
    let Some(budget) = budgets.presets.get(preset.name) else {
        return vec![format!(
            "missing wasm size budget for preset {}",
            preset.name
        )];
    };

    let mut failures = Vec::new();
    check_metric(
        &mut failures,
        preset.name,
        "raw_bytes",
        measurement.raw_bytes,
        budget.max_raw_bytes,
    );
    if let Some(max) = budget.max_stripped_bytes {
        if let Some(stripped_bytes) = measurement.stripped_bytes {
            check_metric(
                &mut failures,
                preset.name,
                "stripped_bytes",
                stripped_bytes,
                Some(max),
            );
        } else {
            failures.push(format!(
                "preset {} skipped stripped_bytes but budget requires max_stripped_bytes={max}",
                preset.name
            ));
        }
    }
    check_metric(
        &mut failures,
        preset.name,
        "gzip_bytes",
        measurement.gzip_bytes,
        budget.max_gzip_bytes,
    );
    check_metric(
        &mut failures,
        preset.name,
        "brotli_bytes",
        measurement.brotli_bytes,
        budget.max_brotli_bytes,
    );

    failures
}

fn check_metric(
    failures: &mut Vec<String>,
    preset_name: &str,
    metric: &str,
    actual: u64,
    max: Option<u64>,
) {
    if let Some(max) = max
        && actual > max
    {
        failures.push(format!(
            "preset {preset_name} exceeds {metric}: actual={actual} max={max}"
        ));
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

fn strip_copy(
    preset: &WasmPreset,
    wasm_path: &Path,
    strip_dir: &Path,
) -> Result<PathBuf, XtaskError> {
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

    Ok(stripped_path)
}

fn file_size(path: &Path) -> Result<u64, XtaskError> {
    fs::metadata(path)
        .map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })
        .map(|metadata| metadata.len())
}

fn gzip_size(path: &Path) -> Result<u64, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&bytes)
        .map_err(|source| XtaskError::CompressFile {
            path: path.display().to_string(),
            source,
        })?;
    let compressed = encoder
        .finish()
        .map_err(|source| XtaskError::CompressFile {
            path: path.display().to_string(),
            source,
        })?;
    Ok(compressed.len() as u64)
}

fn brotli_size(path: &Path) -> Result<u64, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let mut compressed = Vec::new();
    let mut reader = brotli::CompressorReader::new(&bytes[..], 4096, 11, 22);
    reader
        .read_to_end(&mut compressed)
        .map_err(|source| XtaskError::CompressFile {
            path: path.display().to_string(),
            source,
        })?;
    Ok(compressed.len() as u64)
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
        assert!(presets.iter().any(|preset| preset.name == "typst-full-elk"));
    }

    #[test]
    fn surface_filter_selects_only_that_surface() {
        let options = Options {
            surface: Some(Surface::Typst),
            preset: None,
            no_strip: false,
            budget_file: None,
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
            budget_file: None,
        };
        let presets = selected_presets(&options).unwrap();

        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "browser-render");
        assert_eq!(presets[0].features, &["render", "analysis"]);
        assert!(presets[0].no_default_features);
    }

    #[test]
    fn unmatched_filters_are_errors() {
        let options = Options {
            surface: Some(Surface::Typst),
            preset: Some("browser-render".to_string()),
            no_strip: false,
            budget_file: None,
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
            "--budget-file".to_string(),
            "docs/release/WASM_SIZE_BUDGETS.json".to_string(),
        ])
        .unwrap();

        assert_eq!(options.surface, Some(Surface::Browser));
        assert_eq!(options.preset.as_deref(), Some("browser-core"));
        assert!(options.no_strip);
        assert_eq!(
            options.budget_file.as_deref(),
            Some(Path::new("docs/release/WASM_SIZE_BUDGETS.json"))
        );
    }

    #[test]
    fn budget_check_reports_missing_preset_budget() {
        let preset = PRESETS
            .iter()
            .find(|preset| preset.name == "browser-core")
            .unwrap();
        let budgets = WasmSizeBudgets {
            presets: BTreeMap::new(),
        };
        let measurement = measurement_for_test();

        let failures = check_budget(preset, &measurement, &budgets);

        assert_eq!(
            failures,
            vec!["missing wasm size budget for preset browser-core"]
        );
    }

    #[test]
    fn budget_check_reports_only_exceeded_metrics() {
        let preset = PRESETS
            .iter()
            .find(|preset| preset.name == "browser-core")
            .unwrap();
        let mut budgets = WasmSizeBudgets {
            presets: BTreeMap::new(),
        };
        budgets.presets.insert(
            "browser-core".to_string(),
            WasmPresetBudget {
                max_raw_bytes: Some(9),
                max_stripped_bytes: Some(7),
                max_gzip_bytes: Some(5),
                max_brotli_bytes: Some(3),
            },
        );
        let measurement = measurement_for_test();

        let failures = check_budget(preset, &measurement, &budgets);

        assert_eq!(
            failures,
            vec![
                "preset browser-core exceeds raw_bytes: actual=10 max=9",
                "preset browser-core exceeds brotli_bytes: actual=4 max=3",
            ]
        );
    }

    #[test]
    fn feature_label_uses_none_for_empty_features() {
        assert_eq!(feature_label(&[]), "none");
        assert_eq!(
            feature_label(&["render", "ratex-math"]),
            "render+ratex-math"
        );
    }

    fn measurement_for_test() -> WasmMeasurement {
        WasmMeasurement {
            raw_bytes: 10,
            stripped_bytes: Some(7),
            gzip_bytes: 5,
            brotli_bytes: 4,
            artifact_path: PathBuf::from("target/test.wasm"),
            compressed_source: CompressionSource::Stripped,
        }
    }
}
