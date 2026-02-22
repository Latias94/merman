use crate::XtaskError;
use crate::cmd;
use std::path::PathBuf;
use std::process::Command;

pub(crate) fn verify(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        return Err(XtaskError::Usage);
    }
    if !args.is_empty() {
        return Err(XtaskError::Usage);
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
        "--dom-decimals".to_string(),
        "3".to_string(),
    ])?;

    Ok(())
}
