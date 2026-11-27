mod cmd;
mod generated;
mod svgdom;
mod util;

#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("usage: xtask <command> ...")]
    Usage,
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("failed to read file {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to compress file {path}: {source}")]
    CompressFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML schema: {0}")]
    ParseYaml(#[from] serde_saphyr::Error),
    #[error("failed to process JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to parse dompurify dist file: {0}")]
    ParseDompurify(String),
    #[error("failed to project Mermaid default config: {0}")]
    DefaultConfigProjection(String),
    #[error("failed to project Mermaid theme behavior: {0}")]
    ThemeSnapshotProjection(String),
    #[error("text-measurement protocol descriptor is invalid: {0}")]
    TextMeasurementProtocol(String),
    #[error("native ABI descriptor is invalid: {0}")]
    NativeAbi(String),
    #[cfg(feature = "render-parity")]
    #[error("resource contract projection is invalid: {0}")]
    ResourceContract(String),
    #[error("capability surface descriptor is invalid: {0}")]
    CapabilitySurface(String),
    #[error("artifact profile descriptor is invalid: {0}")]
    ArtifactProfiles(String),
    #[error("feature matrix is invalid: {0}")]
    FeatureMatrix(String),
    #[error("Mermaid reference bundle is invalid:\n{0}")]
    MermaidReference(String),
    #[error("missing reference checkout: {0}")]
    MissingReference(String),
    #[error("verification failed:\n{0}")]
    VerifyFailed(String),
    #[cfg(not(feature = "render-parity"))]
    #[error("{0}")]
    RenderParityRequired(String),
    #[cfg(feature = "wasm-host")]
    #[error("profile budget check failed:\n{0}")]
    ProfileBudgetFailed(String),
    #[cfg(not(feature = "wasm-host"))]
    #[error("{0}")]
    WasmHostRequired(String),
    #[error("WASM size matrix failed:\n{0}")]
    WasmSizeMatrixFailed(String),
    #[error("workspace WASM build lock failed: {0}")]
    WasmBuildLockFailed(String),
    #[error("Typst package build failed:\n{0}")]
    TypstPackageFailed(String),
    #[cfg(feature = "wasm-host")]
    #[error("Typst package smoke failed:\n{0}")]
    TypstPackageSmokeFailed(String),
    #[cfg(feature = "wasm-host")]
    #[error("Typst plugin smoke failed:\n{0}")]
    TypstPluginSmokeFailed(String),
    #[error("snapshot update failed: {0}")]
    SnapshotUpdateFailed(String),
    #[cfg(feature = "render-parity")]
    #[error("layout snapshot update failed: {0}")]
    LayoutSnapshotUpdateFailed(String),
    #[error("alignment check failed:\n{0}")]
    AlignmentCheckFailed(String),
    #[error("debug svg generation failed:\n{0}")]
    DebugSvgFailed(String),
    #[error("upstream svg generation failed:\n{0}")]
    UpstreamSvgFailed(String),
    #[error("parser differential failed:\n{0}")]
    ParserDifferential(String),
    #[error("svg compare failed:\n{0}")]
    SvgCompareFailed(String),
}

macro_rules! render_parity_command {
    ($name:expr, $run:expr) => {{
        #[cfg(feature = "render-parity")]
        {
            $run
        }
        #[cfg(not(feature = "render-parity"))]
        {
            let name = $name;
            Err(XtaskError::RenderParityRequired(format!(
                "`{name}` requires the `render-parity` feature. Run `cargo run -p xtask --features render-parity -- {name} ...`."
            )))
        }
    }};
}

#[cfg(not(feature = "render-parity"))]
fn delegate_verify(args: impl Iterator<Item = String>) -> Result<(), XtaskError> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = std::process::Command::new(cargo)
        .args([
            "run",
            "--locked",
            "-p",
            "xtask",
            "--features",
            "render-parity",
            "--",
            "verify",
        ])
        .args(args)
        .current_dir(cmd::workspace_root())
        .status()
        .map_err(|error| {
            XtaskError::VerifyFailed(format!("failed to launch render-parity verify: {error}"))
        })?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn print_help(topic: Option<&str>) {
    if let Some(topic) = topic.filter(|t| !t.trim().is_empty()) {
        println!("usage: xtask {topic} ...");
        println!();
        println!("This repository uses a lightweight custom CLI parser for xtask commands.");
        println!("Most subcommands accept `--help`/`-h` and will show a usage error.");
        println!();
        println!("See: `crates/xtask/src/main.rs` for the full argument grammar.");
        return;
    }

    println!("usage: xtask <command> ...");
    println!();
    println!("Common commands:");
    println!("  verify (starts the render-parity verifier when needed)");
    println!("  verify-generated (requires --features render-parity)");
    println!("  verify-default-config");
    println!("  verify-dompurify-defaults");
    println!("  verify-theme-snapshot");
    println!("  verify-editor-language-contract");
    println!("  verify-binding-contract (requires --features render-parity)");
    println!("  verify-capability-surface");
    println!("  verify-artifact-profiles");
    println!("  verify-feature-matrix");
    println!("  verify-typst-profile-constants");
    println!("  verify-playground-example-catalog (requires --features render-parity)");
    println!("  verify-playground-ascii-capabilities");
    println!("  verify-mermaid-reference");
    println!(
        "  parser-differential [--cases N --seed N --timeout-seconds N | --replay PATH | --minimize PATH]"
    );
    println!("  verify-web-diagram-catalog");
    println!("  verify-resource-contract (requires --features render-parity)");
    println!("  check-alignment");
    println!("  profile-budget (requires --features wasm-host)");
    println!("  wasm-size-matrix");
    println!("  build-typst-package (requires --features wasm-host)");
    println!("  typst-package-smoke (requires --features wasm-host)");
    println!("  audit-gaps");
    println!("  import-upstream-docs (requires --features render-parity)");
    println!("  import-upstream-examples (requires --features render-parity)");
    println!("  import-upstream-html (requires --features render-parity)");
    println!("  project-upstream-cypress-collection");
    println!("  update-snapshots");
    println!(
        "  update-layout-snapshots (alias: gen-layout-goldens, requires --features render-parity)"
    );
    println!("  gen-upstream-svgs");
    println!("  sync-upstream-mmd-corpus");
    println!("  adopt-upstream-svg-provenance");
    println!("  check-upstream-svgs");
    println!("  compare-all-svgs (requires --features render-parity)");
    println!("  compare-svg-xml (requires --features render-parity)");
    println!("  gen-debug-svgs (requires --features render-parity)");
    println!("  debug-svg-bbox (requires --features render-parity)");
    println!("  debug-svg-data-points");
    println!("  measure-text (requires --features render-parity)");
    println!("  gen-default-config [--reference-bundle PATH --out PATH --shape-out PATH]");
    println!("  gen-theme-snapshot [--reference-bundle PATH --out PATH --audit-out PATH]");
    println!("  gen-editor-language-contract");
    println!("  gen-binding-contract (requires --features render-parity)");
    println!("  gen-capability-surface");
    println!("  gen-typst-profile-constants");
    println!("  gen-text-measurement-protocol");
    println!("  gen-native-abi");
    println!("  gen-resource-contract (requires --features render-parity)");
    println!("  verify-text-measurement-protocol");
    println!("  verify-native-abi");
    println!("  gen-playground-example-catalog (requires --features render-parity)");
    println!("  gen-playground-ascii-capabilities");
    println!("  gen-mermaid-reference");
    println!("  gen-web-diagram-catalog");
    println!();
    println!("Per-diagram SVG compare commands (require --features render-parity):");
    for fact in cmd::DIAGRAM_VERIFICATION_FACTS {
        println!("  {}", fact.command);
    }
    println!("  check-flowchart-elk-parity (requires --features render-parity)");
    println!("  audit-flowchart-elk-parity-coverage");
    println!();
    println!("Tips:");
    println!("  - `cargo run --locked -p xtask -- verify`");
    println!("  - `cargo run --locked -p xtask -- verify --strict`");
    println!(
        "  - `cargo run -p xtask --features render-parity -- compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout`"
    );
    println!(
        "  - `cargo run -p xtask --features render-parity -- compare-all-svgs --report-root --report-root-all --dom-mode parity-root`"
    );
    println!(
        "  - `cargo run -p xtask --features wasm-host -- build-typst-package --profile publish --budget-file docs/release/WASM_SIZE_BUDGETS.json`"
    );
    println!(
        "  - `cargo run -p xtask --features wasm-host -- typst-package-smoke --skip-wasm-build --typst <path>`"
    );
    println!("  - `cargo run -p xtask -- gen-upstream-svgs --diagram <name>`");
    println!(
        "  - `cargo run -p xtask -- adopt-upstream-svg-provenance --diagram <name|all> [--check-only | --allow-downgrade]`"
    );
    println!();
    println!("Topics:");
    println!("  xtask help <command>");
}

fn main() -> Result<(), XtaskError> {
    let mut args = std::env::args().skip(1);
    let Some(cmd_name) = args.next() else {
        return Err(XtaskError::Usage);
    };

    if matches!(cmd_name.as_str(), "--help" | "-h") {
        print_help(None);
        return Ok(());
    }
    if cmd_name == "help" {
        print_help(args.next().as_deref());
        return Ok(());
    }

    if let Some(fact) = cmd::diagram_verification_fact_for_command(&cmd_name).copied() {
        return render_parity_command!(
            fact.command,
            cmd::compare_diagram_command(fact, args.collect())
        );
    }

    match cmd_name.as_str() {
        "gen-default-config" => cmd::gen_default_config(args.collect()),
        "gen-dompurify-defaults" => cmd::gen_dompurify_defaults(args.collect()),
        "gen-theme-snapshot" => cmd::gen_theme_snapshot(args.collect()),
        "gen-editor-language-contract" => cmd::gen_editor_language_contract(args.collect()),
        "gen-binding-contract" => {
            render_parity_command!(cmd_name, cmd::gen_binding_contract(args.collect()))
        }
        "gen-capability-surface" => cmd::gen_capability_surface(args.collect()),
        "gen-typst-profile-constants" => cmd::gen_typst_profile_constants(args.collect()),
        "gen-playground-example-catalog" => {
            render_parity_command!(
                cmd_name,
                cmd::gen_playground_example_catalog(args.collect())
            )
        }
        "gen-playground-ascii-capabilities" => {
            cmd::gen_playground_ascii_capabilities(args.collect())
        }
        "gen-mermaid-reference" => cmd::gen_mermaid_reference(args.collect()),
        "gen-web-diagram-catalog" => cmd::gen_web_diagram_catalog(args.collect()),
        "gen-text-measurement-protocol" => cmd::gen_text_measurement_protocol(args.collect()),
        "gen-native-abi" => cmd::gen_native_abi(args.collect()),
        "gen-resource-contract" => {
            render_parity_command!(cmd_name, cmd::gen_resource_contract(args.collect()))
        }
        #[cfg(feature = "render-parity")]
        "verify" => cmd::verify(args.collect()),
        #[cfg(not(feature = "render-parity"))]
        "verify" => delegate_verify(args),
        "verify-default-config" => cmd::verify_default_config(args.collect()),
        "verify-dompurify-defaults" => cmd::verify_dompurify_defaults(args.collect()),
        "verify-theme-snapshot" => cmd::verify_theme_snapshot(args.collect()),
        "verify-editor-language-contract" => cmd::verify_editor_language_contract(args.collect()),
        "verify-binding-contract" => {
            render_parity_command!(cmd_name, cmd::verify_binding_contract(args.collect()))
        }
        "verify-capability-surface" => cmd::verify_capability_surface(args.collect()),
        "verify-artifact-profiles" => {
            cmd::verify_artifact_profiles(args.collect()).map_err(XtaskError::ArtifactProfiles)
        }
        "verify-feature-matrix" => cmd::verify_feature_matrix(args.collect()),
        "verify-typst-profile-constants" => cmd::verify_typst_profile_constants(args.collect()),
        "verify-playground-example-catalog" => {
            render_parity_command!(
                cmd_name,
                cmd::verify_playground_example_catalog(args.collect())
            )
        }
        "verify-playground-ascii-capabilities" => {
            cmd::verify_playground_ascii_capabilities(args.collect())
        }
        "verify-mermaid-reference" => cmd::verify_mermaid_reference(args.collect()),
        "parser-differential" => cmd::parser_differential::run(args.collect()),
        "verify-web-diagram-catalog" => cmd::verify_web_diagram_catalog(args.collect()),
        "verify-text-measurement-protocol" => cmd::verify_text_measurement_protocol(args.collect()),
        "verify-native-abi" => cmd::verify_native_abi(args.collect()),
        "verify-resource-contract" => {
            render_parity_command!(cmd_name, cmd::verify_resource_contract(args.collect()))
        }
        "verify-generated" => {
            render_parity_command!(cmd_name, cmd::verify_generated(args.collect()))
        }
        "wasm-size-matrix" => cmd::wasm_size_matrix(args.collect()),
        #[cfg(feature = "wasm-host")]
        "profile-budget" => cmd::profile_budget::profile_budget(args.collect()),
        #[cfg(feature = "wasm-host")]
        "build-typst-package" => cmd::typst_package::build_typst_package(args.collect()),
        #[cfg(feature = "wasm-host")]
        "typst-package-smoke" => cmd::typst_package::typst_package_smoke(args.collect()),
        #[cfg(not(feature = "wasm-host"))]
        "profile-budget" | "build-typst-package" | "typst-package-smoke" => {
            Err(XtaskError::WasmHostRequired(format!(
                "`{cmd_name}` requires the `wasm-host` feature. Run `cargo run -p xtask --features wasm-host -- {cmd_name} ...`."
            )))
        }
        "import-upstream-docs" => {
            render_parity_command!(cmd_name, cmd::import_upstream_docs(args.collect()))
        }
        "import-upstream-examples" => {
            render_parity_command!(cmd_name, cmd::import_upstream_examples(args.collect()))
        }
        "import-upstream-html" => {
            render_parity_command!(cmd_name, cmd::import_upstream_html(args.collect()))
        }
        "project-upstream-cypress-collection" => {
            cmd::project_upstream_cypress_collection(args.collect())
        }
        "update-snapshots" => cmd::update_snapshots(args.collect()),
        "update-layout-snapshots" | "gen-layout-goldens" => {
            render_parity_command!(cmd_name, cmd::update_layout_snapshots(args.collect()))
        }
        "check-alignment" => cmd::check_alignment(args.collect()),
        "audit-gaps" => cmd::audit_gaps(args.collect()),
        "gen-debug-svgs" => render_parity_command!(cmd_name, cmd::gen_debug_svgs(args.collect())),
        "measure-text" => render_parity_command!(cmd_name, cmd::measure_text(args.collect())),
        "gen-upstream-svgs" => cmd::gen_upstream_svgs(args.collect()),
        "sync-upstream-mmd-corpus" => cmd::sync_upstream_mmd_corpus(args.collect()),
        "adopt-upstream-svg-provenance" => cmd::adopt_upstream_svg_provenance(args.collect()),
        "check-upstream-svgs" => cmd::check_upstream_svgs(args.collect()),
        "check-flowchart-elk-parity" => {
            render_parity_command!(cmd_name, cmd::check_flowchart_elk_parity(args.collect()))
        }
        "audit-flowchart-elk-parity-coverage" => {
            cmd::audit_flowchart_elk_parity_coverage(args.collect())
        }
        "debug-svg-bbox" => render_parity_command!(cmd_name, cmd::debug_svg_bbox(args.collect())),
        "debug-svg-data-points" => cmd::debug_svg_data_points(args.collect()),
        "compare-all-svgs" => {
            render_parity_command!(cmd_name, cmd::compare_all_svgs(args.collect()))
        }
        "compare-svg-xml" => render_parity_command!(cmd_name, cmd::compare_svg_xml(args.collect())),
        other => Err(XtaskError::UnknownCommand(other.to_string())),
    }
}
