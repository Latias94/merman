use crate::XtaskError;
use crate::cmd;
use std::process::Command;

#[derive(Debug, Default)]
struct VerifyOptions {
    clippy: bool,
    all_features: bool,
    check_overrides: bool,
    feature_matrix: bool,
    root_parity: bool,
}

pub(crate) fn verify(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_verify_options(args)?;

    fn parse_verify_options(args: Vec<String>) -> Result<VerifyOptions, XtaskError> {
        let mut options = VerifyOptions::default();

        for arg in args {
            match arg.as_str() {
                "--clippy" => options.clippy = true,
                "--all-features" => options.all_features = true,
                "--check-overrides" => options.check_overrides = true,
                "--feature-matrix" => options.feature_matrix = true,
                "--root-parity" => options.root_parity = true,
                "--strict" => {
                    options.clippy = true;
                    options.all_features = true;
                    options.check_overrides = true;
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
            "usage: xtask verify [--clippy] [--all-features] [--check-overrides] [--feature-matrix] [--root-parity] [--strict]"
        );
        println!();
        println!("Default gates:");
        println!("  cargo fmt --check");
        println!("  cargo nextest run");
        println!("  compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3");
        println!();
        println!("Optional gates:");
        println!("  --clippy        run cargo clippy --workspace --all-targets -- -D warnings");
        println!("  --all-features  run cargo check --workspace --all-features");
        println!("                  also applies --all-features to clippy when combined");
        println!("  --check-overrides");
        println!("                  fail if generated/manual override counts grow beyond budget");
        println!("  --feature-matrix");
        println!("                  check public no-default/render/raster feature combinations");
        println!("  --root-parity   run full SVG root parity after normal DOM parity");
        println!(
            "  --strict        shorthand for --clippy --all-features --check-overrides --feature-matrix --root-parity"
        );
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

    if options.check_overrides {
        println!("\n== override growth budget ==");
        cmd::report_overrides(vec!["--check-no-growth".to_string()])?;
    }

    if options.feature_matrix {
        println!("\n== feature matrix ==");
        run_feature_matrix(&workspace_root, &mut run_checked)?;
    }

    println!("\n== cargo nextest ==");
    let mut nextest_cmd = Command::new("cargo");
    nextest_cmd
        .arg("nextest")
        .arg("run")
        .current_dir(&workspace_root);
    run_checked("cargo nextest run", &mut nextest_cmd)?;

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

    Ok(())
}

fn run_feature_matrix(
    workspace_root: &std::path::Path,
    run_checked: &mut impl FnMut(&str, &mut Command) -> Result<(), XtaskError>,
) -> Result<(), XtaskError> {
    let checks: &[(&str, &[&str])] = &[
        (
            "cargo check -p merman --no-default-features",
            &["check", "-p", "merman", "--no-default-features"],
        ),
        (
            "cargo check -p merman --no-default-features --features render",
            &[
                "check",
                "-p",
                "merman",
                "--no-default-features",
                "--features",
                "render",
            ],
        ),
        (
            "cargo check -p merman --no-default-features --features raster",
            &[
                "check",
                "-p",
                "merman",
                "--no-default-features",
                "--features",
                "raster",
            ],
        ),
        (
            "cargo check -p merman-core --no-default-features",
            &["check", "-p", "merman-core", "--no-default-features"],
        ),
        (
            "cargo nextest run -p merman-lsp --no-default-features --lib",
            &[
                "nextest",
                "run",
                "-p",
                "merman-lsp",
                "--no-default-features",
                "--lib",
            ],
        ),
        (
            "cargo check -p merman-lsp --no-default-features --lib",
            &[
                "check",
                "-p",
                "merman-lsp",
                "--no-default-features",
                "--lib",
            ],
        ),
        (
            "cargo check -p merman-lsp --no-default-features --features core-full-registry --lib",
            &[
                "check",
                "-p",
                "merman-lsp",
                "--no-default-features",
                "--features",
                "core-full-registry",
                "--lib",
            ],
        ),
        (
            "cargo check -p merman-lsp --no-default-features --features core-full-config --lib",
            &[
                "check",
                "-p",
                "merman-lsp",
                "--no-default-features",
                "--features",
                "core-full-config",
                "--lib",
            ],
        ),
        (
            "cargo check -p merman-lsp --no-default-features --features core-full-sanitization --lib",
            &[
                "check",
                "-p",
                "merman-lsp",
                "--no-default-features",
                "--features",
                "core-full-sanitization",
                "--lib",
            ],
        ),
        (
            "cargo check -p merman-lsp --no-default-features --features core-host --lib",
            &[
                "check",
                "-p",
                "merman-lsp",
                "--no-default-features",
                "--features",
                "core-host",
                "--lib",
            ],
        ),
        (
            "cargo check -p merman-lsp --no-default-features --features stdio --bin merman-lsp",
            &[
                "check",
                "-p",
                "merman-lsp",
                "--no-default-features",
                "--features",
                "stdio",
                "--bin",
                "merman-lsp",
            ],
        ),
    ];

    for (what, args) in checks {
        println!("{what}");
        let mut cmd = Command::new("cargo");
        cmd.args(*args).current_dir(workspace_root);
        run_checked(what, &mut cmd)?;
    }

    Ok(())
}
