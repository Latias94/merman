use crate::XtaskError;
use crate::cmd;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Default)]
struct VerifyOptions {
    clippy: bool,
    all_features: bool,
}

pub(crate) fn verify(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_verify_options(args)?;

    fn parse_verify_options(args: Vec<String>) -> Result<VerifyOptions, XtaskError> {
        let mut options = VerifyOptions::default();

        for arg in args {
            match arg.as_str() {
                "--clippy" => options.clippy = true,
                "--all-features" => options.all_features = true,
                "--strict" => {
                    options.clippy = true;
                    options.all_features = true;
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
        println!("usage: xtask verify [--clippy] [--all-features] [--strict]");
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
        println!("  --strict        shorthand for --clippy --all-features");
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

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

    Ok(())
}
