mod support;

use assert_cmd::cargo::cargo_bin;
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use support::exit_code;

const MERMAID_SOURCE: &str = "flowchart LR\nA-->B\n";

fn run_in(cwd: &Path, args: &[&str]) -> Output {
    Command::new(cargo_bin!("merman-cli"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run CLI")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

fn assert_alias_rejected(output: &Output, output_name: &str, protected_name: &str) {
    assert_eq!(exit_code(output.status), 2, "stderr:\n{}", stderr(output));
    assert!(
        output.stdout.is_empty(),
        "preflight failures must not write a payload"
    );
    let message = stderr(output);
    assert!(
        message.contains(output_name),
        "error should identify the output path:\n{message}"
    );
    assert!(
        message.contains(protected_name),
        "error should identify the protected path:\n{message}"
    );
    let lower = message.to_ascii_lowercase();
    assert!(
        lower.contains("alias")
            || lower.contains("same file")
            || lower.contains("file identity")
            || lower.contains("protected input"),
        "error should explain the file identity collision:\n{message}"
    );
}

#[test]
fn lexical_output_alias_of_primary_input_is_rejected_before_replacement() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(tmp.path().join("walk")).expect("create lexical path component");
    fs::write(tmp.path().join("source"), MERMAID_SOURCE).expect("write source");

    let output = run_in(
        tmp.path(),
        &[
            "render",
            "source",
            "--output",
            "walk/../source",
            "--format",
            "svg",
        ],
    );

    assert_alias_rejected(&output, "source", "source");
    assert_eq!(
        fs::read_to_string(tmp.path().join("source")).expect("read source"),
        MERMAID_SOURCE
    );
}

#[cfg(unix)]
#[test]
fn canonical_output_alias_through_a_symlinked_parent_is_rejected() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().expect("tempdir");
    let real = tmp.path().join("real");
    fs::create_dir(&real).expect("create real directory");
    fs::write(real.join("source"), MERMAID_SOURCE).expect("write source");
    symlink(&real, tmp.path().join("view")).expect("create directory symlink");

    let output = run_in(
        tmp.path(),
        &[
            "render",
            "real/source",
            "--output",
            "view/source",
            "--format",
            "svg",
        ],
    );

    assert_alias_rejected(&output, "source", "source");
    assert_eq!(
        fs::read_to_string(real.join("source")).expect("read source"),
        MERMAID_SOURCE
    );
}

#[test]
fn hard_link_output_alias_of_primary_input_is_rejected_by_file_identity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("diagram.mmd");
    let output_path = tmp.path().join("out.svg");
    fs::write(&input, MERMAID_SOURCE).expect("write source");
    fs::hard_link(&input, &output_path).expect("create output hard link");

    let output = run_in(
        tmp.path(),
        &["render", "diagram.mmd", "--output", "out.svg"],
    );

    assert_alias_rejected(&output, "out.svg", "diagram.mmd");
    assert_eq!(
        fs::read_to_string(input).expect("read source"),
        MERMAID_SOURCE
    );
    assert_eq!(
        fs::read_to_string(output_path).expect("read output link"),
        MERMAID_SOURCE
    );
}

#[test]
fn hard_link_aliases_of_every_auxiliary_input_are_rejected() {
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            "configuration",
            "config.json",
            "{}",
            &[
                "render",
                "diagram.mmd",
                "--output",
                "out.svg",
                "--config-file",
                "config.json",
            ],
        ),
        (
            "CSS",
            "style.css",
            "svg { color: red; }",
            &[
                "render",
                "diagram.mmd",
                "--output",
                "out.svg",
                "--css-file",
                "style.css",
            ],
        ),
        (
            "Puppeteer configuration",
            "puppeteer.json",
            "{}",
            &[
                "mmdc",
                "-i",
                "diagram.mmd",
                "-o",
                "out.svg",
                "--puppeteerConfigFile",
                "puppeteer.json",
            ],
        ),
        (
            "local icon",
            "icons.json",
            r#"{"prefix":"fixture","icons":{"box":{"body":"<path d=\"M0 0\"/>"}}}"#,
            &[
                "render",
                "diagram.mmd",
                "--output",
                "out.svg",
                "--icon-pack-source",
                "fixture#icons.json",
            ],
        ),
    ];

    for (label, protected_name, contents, args) in cases {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("diagram.mmd"), MERMAID_SOURCE).expect("write source");
        let protected = tmp.path().join(protected_name);
        fs::write(&protected, contents).expect("write protected input");
        fs::hard_link(&protected, tmp.path().join("out.svg")).expect("create output hard link");

        let output = run_in(tmp.path(), args);

        assert_alias_rejected(&output, "out.svg", protected_name);
        assert_eq!(
            fs::read_to_string(&protected).expect("read protected input"),
            *contents,
            "{label} input changed"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("out.svg")).expect("read output hard link"),
            *contents,
            "{label} hard link changed"
        );
    }
}

#[cfg(unix)]
#[test]
fn output_alias_of_a_symlinked_css_input_is_rejected() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), MERMAID_SOURCE).expect("write source");
    fs::write(tmp.path().join("out.svg"), "svg { color: red; }").expect("write CSS target");
    symlink("out.svg", tmp.path().join("style.css")).expect("create CSS symlink");

    let output = run_in(
        tmp.path(),
        &[
            "render",
            "diagram.mmd",
            "--output",
            "out.svg",
            "--css-file",
            "style.css",
        ],
    );

    assert_alias_rejected(&output, "out.svg", "style.css");
    assert!(
        fs::symlink_metadata(tmp.path().join("style.css"))
            .expect("CSS link metadata")
            .file_type()
            .is_symlink(),
        "preflight must not replace the protected input symlink"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("out.svg")).expect("read CSS target"),
        "svg { color: red; }"
    );
}

#[cfg(unix)]
#[test]
fn an_unrelated_render_target_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), MERMAID_SOURCE).expect("write source");
    fs::write(tmp.path().join("existing.svg"), "complete old output").expect("write target");
    symlink("existing.svg", tmp.path().join("out.svg")).expect("create output symlink");

    let output = run_in(
        tmp.path(),
        &["render", "diagram.mmd", "--output", "out.svg"],
    );

    assert_eq!(exit_code(output.status), 2, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    let message = stderr(&output);
    assert!(
        message.contains("out.svg") && message.to_ascii_lowercase().contains("symlink"),
        "error should name and classify the rejected output:\n{message}"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("existing.svg")).expect("read target"),
        "complete old output"
    );
    assert!(
        fs::symlink_metadata(tmp.path().join("out.svg"))
            .expect("output link metadata")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn missing_output_parent_is_rejected_without_creating_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), MERMAID_SOURCE).expect("write source");

    let output = run_in(
        tmp.path(),
        &["render", "diagram.mmd", "--output", "missing/out.svg"],
    );

    assert_eq!(exit_code(output.status), 2, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    let message = stderr(&output);
    assert!(
        message.contains("missing") && message.contains("out.svg"),
        "error should identify the unavailable output parent:\n{message}"
    );
    assert!(
        !tmp.path().join("missing").exists(),
        "local preflight must not create an output directory"
    );
}

#[test]
fn non_regular_render_input_is_an_operational_failure() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(tmp.path().join("diagram.mmd")).expect("create directory input");

    let output = run_in(
        tmp.path(),
        &["render", "diagram.mmd", "--output", "out.svg"],
    );

    assert_eq!(exit_code(output.status), 3, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(
        stderr(&output).contains("not a regular file"),
        "the error should classify the filesystem input failure"
    );
    assert!(!tmp.path().join("out.svg").exists());
}

#[test]
fn markdown_numbered_output_namespace_may_not_alias_css_input() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = "before\n```mermaid\nflowchart LR\nA-->B\n```\nafter\n";
    fs::write(tmp.path().join("document.md"), source).expect("write Markdown source");
    let artefacts = tmp.path().join("artefacts");
    fs::create_dir(&artefacts).expect("create artefacts directory");
    let protected = artefacts.join("result-1.svg");
    fs::write(&protected, "svg { color: red; }").expect("write protected CSS");

    let output = run_in(
        tmp.path(),
        &[
            "mmdc",
            "-i",
            "document.md",
            "-o",
            "result.md",
            "-a",
            "artefacts",
            "--cssFile",
            "artefacts/result-1.svg",
        ],
    );

    assert_alias_rejected(&output, "artefacts/result-1.svg", "artefacts/result-1.svg");
    assert!(
        !tmp.path().join("result.md").exists(),
        "reserved artifact alias rejection must precede document publication"
    );
    assert_eq!(
        fs::read_to_string(&protected).expect("read protected CSS"),
        "svg { color: red; }"
    );
}

#[test]
fn markdown_numbered_output_namespace_uses_filesystem_case_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = "before\n```mermaid\nflowchart LR\nA-->B\n```\nafter\n";
    fs::write(tmp.path().join("document.md"), source).expect("write Markdown source");
    let artefacts = tmp.path().join("artefacts");
    fs::create_dir(&artefacts).expect("create artefacts directory");
    let protected = artefacts.join("RESULT-1.SVG");
    fs::write(&protected, "complete existing output").expect("write existing output");

    if fs::metadata(artefacts.join("result-1.svg")).is_err() {
        return;
    }

    let output = run_in(
        tmp.path(),
        &[
            "mmdc",
            "-i",
            "document.md",
            "-o",
            "result.md",
            "-a",
            "artefacts",
        ],
    );

    assert_alias_rejected(&output, "result-1.svg", "RESULT-1.SVG");
    assert!(
        !tmp.path().join("result.md").exists(),
        "filesystem alias rejection must precede document publication"
    );
    assert_eq!(
        fs::read_to_string(&protected).expect("read existing output"),
        "complete existing output"
    );
}

#[test]
fn existing_output_is_atomically_replaced_and_preserves_mode() {
    #[cfg(unix)]
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), MERMAID_SOURCE).expect("write source");
    let output_path = tmp.path().join("out.svg");
    fs::write(&output_path, "complete old output").expect("write old output");
    #[cfg(unix)]
    fs::set_permissions(&output_path, fs::Permissions::from_mode(0o640))
        .expect("set old output mode");

    let output = run_in(
        tmp.path(),
        &["render", "diagram.mmd", "--output", "out.svg"],
    );

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(
        fs::read_to_string(&output_path)
            .expect("read replacement output")
            .contains("<svg"),
        "a successful rerun should replace the prior output"
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&output_path).expect("output metadata").mode() & 0o777,
        0o640,
        "ordinary Unix permission bits should survive atomic replacement"
    );
}

#[cfg(unix)]
#[test]
fn atomic_staging_permission_failure_leaves_no_target_or_temp_file() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("diagram.mmd"), MERMAID_SOURCE).expect("write source");
    let locked = tmp.path().join("locked");
    fs::create_dir(&locked).expect("create output directory");
    let output_path = locked.join("out.svg");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555))
        .expect("make output directory read-only");

    let permission_probe = locked.join(".permission-probe");
    if fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&permission_probe)
        .is_ok()
    {
        fs::remove_file(permission_probe).expect("remove permission probe");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
            .expect("restore output directory permissions");
        return;
    }

    let output = run_in(
        tmp.path(),
        &["render", "diagram.mmd", "--output", "locked/out.svg"],
    );

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
        .expect("restore output directory permissions");
    assert_eq!(exit_code(output.status), 3, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    let message = stderr(&output);
    assert!(
        message.contains("locked/out.svg")
            && (message.to_ascii_lowercase().contains("staging")
                || message.to_ascii_lowercase().contains("atomic")),
        "operational failure should name the publication operation and target:\n{message}"
    );
    assert!(
        !output_path.exists(),
        "failed staging must not publish a target"
    );
    assert_eq!(
        fs::read_dir(&locked)
            .expect("read output directory")
            .count(),
        0,
        "failed publication should clean its temporary staging file"
    );
}

#[test]
fn fix_output_may_not_alias_its_input() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source_path = tmp.path().join("diagram.mmd");
    let original = "flowchart\nA-->B\n";
    fs::write(&source_path, original).expect("write source");

    let output = run_in(
        tmp.path(),
        &[
            "fix",
            "diagram.mmd",
            "--output",
            "diagram.mmd",
            "--lint-profile",
            "recommended",
        ],
    );

    assert_alias_rejected(&output, "diagram.mmd", "diagram.mmd");
    assert_eq!(
        fs::read_to_string(source_path).expect("read source"),
        original
    );
}

#[cfg(unix)]
#[test]
fn fix_write_follows_input_symlink_and_preserves_the_link_and_mode() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

    let tmp = tempfile::tempdir().expect("tempdir");
    let target = tmp.path().join("target.mmd");
    let input_link = tmp.path().join("input.mmd");
    fs::write(&target, "flowchart\nA-->B\n").expect("write source target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640))
        .expect("set source target mode");
    symlink("target.mmd", &input_link).expect("create input symlink");

    let output = run_in(
        tmp.path(),
        &[
            "fix",
            "input.mmd",
            "--write",
            "--lint-profile",
            "recommended",
        ],
    );

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(
        fs::symlink_metadata(&input_link)
            .expect("input link metadata")
            .file_type()
            .is_symlink(),
        "fix --write must preserve the input symlink directory entry"
    );
    assert_eq!(
        fs::read_link(&input_link).expect("read input link"),
        Path::new("target.mmd")
    );
    let fixed = fs::read_to_string(&target).expect("read fixed target");
    assert!(
        fixed.starts_with("flowchart TB\n"),
        "fix should replace the canonical symlink target:\n{fixed}"
    );
    assert_eq!(
        fs::metadata(&target).expect("target metadata").mode() & 0o777,
        0o640,
        "fix replacement should preserve ordinary Unix permission bits"
    );
}

#[test]
fn fix_write_replaces_one_hard_link_entry_without_rewriting_its_sibling() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    let sibling = tmp.path().join("sibling.mmd");
    let original = "flowchart\nA-->B\n";
    fs::write(&input, original).expect("write source");
    fs::hard_link(&input, &sibling).expect("create sibling hard link");

    let output = run_in(
        tmp.path(),
        &[
            "fix",
            "input.mmd",
            "--write",
            "--lint-profile",
            "recommended",
        ],
    );

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(
        fs::read_to_string(&input)
            .expect("read replaced input")
            .starts_with("flowchart TB\n")
    );
    assert_eq!(
        fs::read_to_string(&sibling).expect("read sibling hard link"),
        original,
        "atomic replacement must not mutate another hard link to the old inode"
    );
}

#[test]
fn markdown_render_failure_publishes_no_serial_or_parallel_subset() {
    let source = concat!(
        "```mermaid\n",
        "flowchart LR\n",
        "A-->B\n",
        "```\n\n",
        "```mermaid\n",
        "definitely not a Mermaid diagram\n",
        "```\n",
    );

    for (name, jobs) in [("serial", "1"), ("parallel", "2")] {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("document.md"), source).expect("write Markdown source");
        let output_name = format!("{name}.md");
        let output = run_in(
            tmp.path(),
            &[
                "mmdc",
                "-i",
                "document.md",
                "-o",
                &output_name,
                "--jobs",
                jobs,
                "--quiet",
            ],
        );

        assert_eq!(exit_code(output.status), 1, "stderr:\n{}", stderr(&output));
        assert!(!tmp.path().join(&output_name).exists());
        assert!(!tmp.path().join(format!("{name}-1.svg")).exists());
        assert!(!tmp.path().join(format!("{name}-2.svg")).exists());
    }
}

#[test]
fn markdown_document_budget_failure_publishes_no_rendered_artifact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = "# Doc\n\n```mermaid\nflowchart LR\nA-->B\n```\n";
    fs::write(tmp.path().join("document.md"), source).expect("write Markdown source");
    let probe = tmp.path().join("probe");
    let failed = tmp.path().join("failed");
    fs::create_dir(&probe).expect("create probe directory");
    fs::create_dir(&failed).expect("create failed directory");

    let baseline = run_in(
        tmp.path(),
        &["mmdc", "-i", "document.md", "-o", "probe/out.md", "--quiet"],
    );
    assert!(
        baseline.status.success(),
        "baseline stderr:\n{}",
        stderr(&baseline)
    );
    let artifact_bytes = fs::metadata(probe.join("out-1.svg"))
        .expect("probe artifact metadata")
        .len();
    let limit = format!("max_staged_bytes={artifact_bytes}");

    let output = run_in(
        tmp.path(),
        &[
            "mmdc",
            "-i",
            "document.md",
            "-o",
            "failed/out.md",
            "--resource-limit",
            &limit,
            "--quiet",
        ],
    );

    assert_eq!(exit_code(output.status), 1, "stderr:\n{}", stderr(&output));
    assert!(
        stderr(&output).contains("max_staged_bytes"),
        "expected the rewritten document to exceed the aggregate staging budget"
    );
    assert!(!failed.join("out.md").exists());
    assert!(
        !failed.join("out-1.svg").exists(),
        "a later document-budget failure must not expose an earlier rendered artifact"
    );
}

#[test]
fn fix_write_with_stdin_is_rejected_before_waiting_for_eof() {
    let mut child = Command::new(cargo_bin!("merman-cli"))
        .args(["fix", "-", "--write"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if child.try_wait().expect("poll CLI").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill blocked CLI");
            child.wait().expect("reap blocked CLI");
            panic!("fix --write waited for stdin instead of rejecting the invocation");
        }
        thread::sleep(Duration::from_millis(10));
    }

    let output = child.wait_with_output().expect("collect CLI output");
    assert_eq!(exit_code(output.status), 2, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    let message = stderr(&output);
    assert!(
        message.contains("--write") && message.to_ascii_lowercase().contains("stdin"),
        "usage error should explain why stdin cannot be written in place:\n{message}"
    );
}

#[test]
fn alias_preflight_precedes_markdown_directory_and_network_side_effects() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = "before\n```mermaid\nflowchart LR\nA-->B\n```\nafter\n";
    fs::write(tmp.path().join("document.md"), source).expect("write Markdown source");
    let (url, request_seen, stop, server) = start_http_sentinel();
    let icon_source = format!("remote#{url}");

    let output = run_in(
        tmp.path(),
        &[
            "mmdc",
            "-i",
            "document.md",
            "-o",
            "document.md",
            "-a",
            "artefacts",
            "--iconPacksNamesAndUrls",
            &icon_source,
            "--allow-network",
            "--allow-private-network",
        ],
    );

    stop.store(true, Ordering::SeqCst);
    server.join().expect("join HTTP sentinel");
    assert_alias_rejected(&output, "document.md", "document.md");
    assert_eq!(
        fs::read_to_string(tmp.path().join("document.md")).expect("read Markdown source"),
        source
    );
    assert!(
        !tmp.path().join("artefacts").exists(),
        "alias preflight must precede artefacts directory creation"
    );
    assert!(
        !request_seen.load(Ordering::SeqCst),
        "alias preflight must precede remote icon acquisition"
    );
}

fn start_http_sentinel() -> (
    String,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind HTTP sentinel");
    listener
        .set_nonblocking(true)
        .expect("make HTTP sentinel non-blocking");
    let address = listener.local_addr().expect("HTTP sentinel address");
    let request_seen = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let thread_seen = Arc::clone(&request_seen);
    let thread_stop = Arc::clone(&stop);
    let server = thread::spawn(move || {
        let body = br#"{"prefix":"remote","icons":{"cloud":{"body":"<path d=\"M0 0\"/>"}}}"#;
        while !thread_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    thread_seen.store(true, Ordering::SeqCst);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(body);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("HTTP sentinel accept failed: {error}"),
            }
        }
    });
    (
        format!("http://{address}/icons.json?token=must-not-be-read"),
        request_seen,
        stop,
        server,
    )
}
