use crate::XtaskError;
use crate::cmd;
use std::process::Command;

const STRICT_WEB_SCRIPTS: [&str; 3] = ["build", "test", "smoke"];
const STRICT_PLAYGROUND_SCRIPTS: [&str; 8] = [
    "verify:dependencies",
    "verify:security",
    "prepare:browser-runtime",
    "test:prepared",
    "lint",
    "build:prepared",
    "test:browser:smoke:non-chromium:built",
    "test:browser:chromium:desktop:built",
];

#[derive(Debug, Default)]
struct VerifyOptions {
    clippy: bool,
    all_features: bool,
    feature_matrix: bool,
    root_parity: bool,
    strict: bool,
}

pub(crate) fn verify(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_verify_options(args)?;

    fn parse_verify_options(args: Vec<String>) -> Result<VerifyOptions, XtaskError> {
        let mut options = VerifyOptions::default();

        for arg in args {
            match arg.as_str() {
                "--clippy" => options.clippy = true,
                "--all-features" => options.all_features = true,
                "--feature-matrix" => options.feature_matrix = true,
                "--root-parity" => options.root_parity = true,
                "--strict" => {
                    options.strict = true;
                    options.clippy = true;
                    options.all_features = true;
                    options.feature_matrix = true;
                    options.root_parity = true;
                }
                "--help" | "-h" => {
                    print_verify_usage();
                    return Err(XtaskError::Usage);
                }
                _ => return Err(XtaskError::Usage),
            }
        }

        Ok(options)
    }

    fn print_verify_usage() {
        println!(
            "usage: xtask verify [--clippy] [--all-features] [--feature-matrix] [--root-parity] [--strict]"
        );
        println!();
        println!("Default gates:");
        println!("  cargo fmt --check");
        println!("  cargo nextest run --workspace");
        println!("  cargo test -p merman-render --doc");
        println!("  cargo test -p merman --doc --features svg");
        println!("  compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3");
        println!("  compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3");
        println!();
        println!("Optional gates:");
        println!("  --clippy        run cargo clippy --workspace --all-targets -- -D warnings");
        println!("  --all-features  run cargo check --workspace --all-features");
        println!("                  also applies --all-features to clippy when combined");
        println!("  --feature-matrix");
        println!(
            "                  validate public Cargo capability closures and build critical combinations"
        );
        println!("  --root-parity   run full SVG root parity after normal DOM parity");
        println!("  --strict        run every optional gate plus materialized release, generated,");
        println!("                  Web, Playground browser, and VS Code evidence");
        println!("                  and cargo test --workspace --doc");
    }

    let workspace_root = crate::cmd::workspace_root();

    fn run_checked(what: &str, cmd: &mut Command) -> Result<(), XtaskError> {
        let status = cmd.status().map_err(|e| {
            XtaskError::VerifyFailed(format!("{what}: failed to spawn process: {e}"))
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(XtaskError::VerifyFailed(format!(
                "{what}: exited with {status}"
            )))
        }
    }

    println!("\n== cargo fmt ==");
    let mut fmt_cmd = Command::new("cargo");
    fmt_cmd
        .arg("fmt")
        .arg("--check")
        .current_dir(&workspace_root);
    run_checked("cargo fmt --check", &mut fmt_cmd)?;

    if !options.strict {
        println!("\n== editor language contract ==");
        cmd::verify_editor_language_contract(Vec::new())?;
    }

    if options.all_features {
        println!("\n== cargo check --workspace --all-features ==");
        let mut check_cmd = Command::new("cargo");
        check_cmd
            .arg("check")
            .arg("--workspace")
            .arg("--all-features")
            .current_dir(&workspace_root);
        run_checked("cargo check --workspace --all-features", &mut check_cmd)?;
    }

    if options.clippy {
        println!(
            "\n== cargo clippy --workspace --all-targets{} ==",
            if options.all_features {
                " --all-features"
            } else {
                ""
            }
        );
        let mut clippy_cmd = Command::new("cargo");
        clippy_cmd
            .arg("clippy")
            .arg("--workspace")
            .arg("--all-targets");
        if options.all_features {
            clippy_cmd.arg("--all-features");
        }
        clippy_cmd
            .arg("--")
            .arg("-D")
            .arg("warnings")
            .current_dir(&workspace_root);
        let what = if options.all_features {
            "cargo clippy --workspace --all-targets --all-features -- -D warnings"
        } else {
            "cargo clippy --workspace --all-targets -- -D warnings"
        };
        run_checked(what, &mut clippy_cmd)?;
    }

    if options.feature_matrix {
        println!("\n== feature matrix ==");
        let args = options
            .strict
            .then(|| "--strict".to_string())
            .into_iter()
            .collect();
        cmd::verify_feature_matrix(args)?;
    }

    if options.strict {
        println!("\n== materialized Mermaid reference bundle ==");
        cmd::verify_mermaid_reference(vec!["--materialized".to_string()])?;

        println!("\n== all generated contracts ==");
        cmd::verify_generated(Vec::new())?;

        println!("\n== alignment evidence ==");
        cmd::check_alignment(Vec::new())?;

        println!("\n== open-source release materials ==");
        for (what, script, argument) in [
            (
                "governed RustSec exceptions",
                "scripts/verify_rustsec_exceptions.py",
                None,
            ),
            (
                "target-scoped artifact dependency closures",
                "scripts/verify_artifact_dependency_closures.py",
                None,
            ),
            (
                "third-party source and license contract",
                "scripts/verify-third-party-licenses.py",
                None,
            ),
            (
                "Rust dependency license report",
                "scripts/generate-rust-license-report.py",
                Some("--check"),
            ),
            (
                "release legal material projections",
                "scripts/sync-release-legal-materials.py",
                Some("--check"),
            ),
            (
                "Cargo package legal materials",
                "scripts/verify_crate_package_legal_materials.py",
                None,
            ),
        ] {
            let mut legal_cmd = Command::new("python3");
            legal_cmd.arg(script).current_dir(&workspace_root);
            if let Some(argument) = argument {
                legal_cmd.arg(argument);
            }
            run_checked(what, &mut legal_cmd)?;
        }
        for package_dir in ["playground", "tools/vscode-extension"] {
            run_npm_script(
                &workspace_root,
                package_dir,
                "licenses:check",
                &mut run_checked,
            )?;
        }
    }

    println!("\n== cargo nextest ==");
    let mut nextest_cmd = Command::new("cargo");
    nextest_cmd
        .arg("nextest")
        .arg("run")
        .arg("--workspace")
        .current_dir(&workspace_root);
    run_checked("cargo nextest run --workspace", &mut nextest_cmd)?;

    println!("\n== architecture compile-fail contracts ==");
    for (what, package, features) in [
        ("cargo test -p merman-render --doc", "merman-render", None),
        (
            "cargo test -p merman --doc --features svg",
            "merman",
            Some("svg"),
        ),
    ] {
        let mut doctest_cmd = Command::new("cargo");
        doctest_cmd.args(["test", "-p", package, "--doc"]);
        if let Some(features) = features {
            doctest_cmd.args(["--features", features]);
        }
        doctest_cmd.current_dir(&workspace_root);
        run_checked(what, &mut doctest_cmd)?;
    }
    if options.strict {
        let mut workspace_doctest_cmd = Command::new("cargo");
        workspace_doctest_cmd
            .args(["test", "--workspace", "--doc"])
            .current_dir(&workspace_root);
        run_checked("cargo test --workspace --doc", &mut workspace_doctest_cmd)?;
    }

    println!("\n== svg dom structure ==");
    cmd::compare_all_svgs(vec![
        "--check-dom".to_string(),
        "--dom-mode".to_string(),
        "structure".to_string(),
        "--dom-decimals".to_string(),
        "3".to_string(),
    ])?;

    println!("\n== svg dom parity ==");
    cmd::compare_all_svgs(vec![
        "--check-dom".to_string(),
        "--dom-mode".to_string(),
        "parity".to_string(),
        "--dom-decimals".to_string(),
        "3".to_string(),
    ])?;

    if options.root_parity {
        println!("\n== svg root parity ==");
        cmd::compare_all_svgs(vec![
            "--check-dom".to_string(),
            "--dom-mode".to_string(),
            "parity-root".to_string(),
            "--dom-decimals".to_string(),
            "3".to_string(),
        ])?;
    }

    if options.strict {
        println!("\n== Web package ==");
        for script in STRICT_WEB_SCRIPTS {
            run_npm_script(&workspace_root, "platforms/web", script, &mut run_checked)?;
        }

        println!("\n== Playground package and browsers ==");
        for script in STRICT_PLAYGROUND_SCRIPTS {
            run_npm_script(&workspace_root, "playground", script, &mut run_checked)?;
        }

        println!("\n== VS Code extension ==");
        run_npm_script(
            &workspace_root,
            "tools/vscode-extension",
            "test",
            &mut run_checked,
        )?;
    }

    Ok(())
}

fn run_npm_script(
    workspace_root: &std::path::Path,
    package_dir: &str,
    script: &str,
    run_checked: &mut impl FnMut(&str, &mut Command) -> Result<(), XtaskError>,
) -> Result<(), XtaskError> {
    let what = format!("npm run {script} ({package_dir})");
    println!("{what}");
    let mut command = Command::new("npm");
    command
        .args(["run", script])
        .current_dir(workspace_root.join(package_dir));
    run_checked(&what, &mut command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_web_scripts_exist_in_workspace_manifest() {
        assert_manifest_scripts("platforms/web/package.json", &STRICT_WEB_SCRIPTS);
    }

    #[test]
    fn strict_playground_scripts_exist_in_workspace_manifest() {
        assert_manifest_scripts("playground/package.json", &STRICT_PLAYGROUND_SCRIPTS);
    }

    fn assert_manifest_scripts(manifest: &str, expected: &[&str]) {
        let manifest_path = crate::cmd::workspace_root().join(manifest);
        let manifest_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&manifest_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", manifest_path.display())),
        )
        .unwrap_or_else(|error| panic!("parse {}: {error}", manifest_path.display()));
        let scripts = manifest_json["scripts"]
            .as_object()
            .unwrap_or_else(|| panic!("{manifest} must define scripts"));

        for script in expected {
            assert!(
                scripts
                    .get(*script)
                    .is_some_and(serde_json::Value::is_string),
                "strict verification script `{script}` is missing from {manifest}"
            );
        }
    }
}
