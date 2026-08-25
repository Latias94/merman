use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
#[cfg(feature = "network-icons")]
use std::io::Read;
use std::io::{ErrorKind, Write};
#[cfg(feature = "network-icons")]
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
#[cfg(feature = "network-icons")]
use std::time::{Duration, Instant};

const CASE_ENV: &str = "MERMAN_CLI_PROFILE_CASE";
const SIMPLE_SOURCE: &str = "flowchart LR\nA[Start] --> B[Done]\n";
#[cfg(feature = "icons")]
const ICON_SOURCE: &str = "flowchart TD\nA@{ icon: \"test:rocket\", label: \"Rocket\" }\n";
#[cfg(feature = "icons")]
const ICON_BODY: &str = r#"{
  "prefix": "test",
  "width": 16,
  "height": 16,
  "icons": {
    "rocket": {
      "body": "<path data-profile-icon=\"rocket\" d=\"M1 1H15V15H1z\"/>"
    }
  }
}"#;

const ALL_OPTIONAL_COMMANDS: &[&str] = &[
    "batch",
    "completion",
    "fix",
    "layout",
    "lint",
    "lint-rules",
    "mmdc",
    "render",
    "rustdoc",
];

const DEFAULT_CAPABILITIES: &[&str] = &[
    "analysis",
    "ascii",
    "icons",
    "jpeg",
    "layout-cytoscape",
    "markdown",
    "math",
    "network-icons",
    "parallel-markdown",
    "pdf",
    "png",
    "rustdoc",
    "shell-completions",
    "svg",
    "system-clock",
    "system-random",
    "system-timezone",
    "system-timing",
];

const RELEASE_CAPABILITIES: &[&str] = &[
    "analysis",
    "ascii",
    "icons",
    "jpeg",
    "layout-cytoscape",
    "layout-elk",
    "markdown",
    "math",
    "network-icons",
    "parallel-markdown",
    "pdf",
    "png",
    "rustdoc",
    "shell-completions",
    "svg",
    "system-clock",
    "system-random",
    "system-timezone",
    "system-timing",
];

fn cli() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("merman-cli").to_path_buf()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("expected crates/<name> layout")
        .to_path_buf()
}

fn run(args: &[&str], stdin: &[u8], cwd: Option<&Path>) -> Output {
    run_with_environment(args, stdin, cwd, &[])
}

fn run_with_environment(
    args: &[&str],
    stdin: &[u8],
    cwd: Option<&Path>,
    environment: &[(&str, &str)],
) -> Output {
    let mut command = Command::new(cli());
    command
        .args(args)
        .envs(environment.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command.spawn().expect("spawn merman-cli");
    let write_result = child.stdin.take().expect("child stdin").write_all(stdin);
    if let Err(error) = write_result
        && error.kind() != ErrorKind::BrokenPipe
    {
        panic!("write merman-cli stdin: {error}");
    }
    child.wait_with_output().expect("wait for merman-cli")
}

fn run_without_stdin(args: &[&str], cwd: Option<&Path>) -> Output {
    let mut command = Command::new(cli());
    command.args(args).stdin(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.output().expect("run merman-cli")
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn selected_case() -> String {
    if let Ok(case) = std::env::var(CASE_ENV) {
        return case;
    }

    let compiled = compiled_capabilities_for_auto_detection();
    for case in [
        "base",
        "analysis",
        "svg",
        "ascii",
        "local-icons",
        "markdown",
        "parallel-markdown",
        "network-icons",
        "png",
        "jpeg",
        "pdf",
        "parallel-pdf",
        "cytoscape-layout",
        "elk-layout",
        "math",
        "rustdoc",
        "completions",
        "svg-completions",
        "system-clock",
        "system-timezone",
        "system-random",
        "system-timing",
        "default",
    ] {
        if expected_capabilities(case) == compiled {
            return case.to_string();
        }
    }
    "auto".to_string()
}

fn expected_capabilities(case: &str) -> Vec<&'static str> {
    match case {
        "base" => vec![],
        "analysis" => vec!["analysis"],
        "svg" => vec!["svg"],
        "ascii" => vec!["ascii"],
        "local-icons" => vec!["icons", "svg"],
        "markdown" => vec!["markdown", "svg"],
        "parallel-markdown" => vec!["markdown", "parallel-markdown", "svg"],
        "network-icons" => vec!["icons", "network-icons", "svg"],
        "png" => vec!["png", "svg"],
        "jpeg" => vec!["jpeg", "svg"],
        "pdf" => vec!["pdf", "svg"],
        "parallel-pdf" => vec!["markdown", "parallel-markdown", "pdf", "svg"],
        "cytoscape-layout" => vec!["layout-cytoscape", "svg"],
        "elk-layout" => vec!["layout-elk", "svg"],
        "math" => vec!["math", "svg"],
        "rustdoc" => vec!["layout-cytoscape", "markdown", "math", "rustdoc", "svg"],
        "completions" => vec!["shell-completions"],
        "svg-completions" => vec!["shell-completions", "svg"],
        "system-clock" => vec!["system-clock"],
        "system-timezone" => vec!["system-timezone"],
        "system-random" => vec!["system-random"],
        "system-timing" => vec!["system-timing"],
        "default" => DEFAULT_CAPABILITIES.to_vec(),
        "release" => RELEASE_CAPABILITIES.to_vec(),
        "auto" => compiled_capabilities_for_auto_detection(),
        other => panic!("unknown {CASE_ENV} value {other:?}"),
    }
}

fn compiled_capabilities_for_auto_detection() -> Vec<&'static str> {
    let mut capabilities = vec![
        #[cfg(feature = "analysis")]
        "analysis",
        #[cfg(feature = "ascii")]
        "ascii",
        #[cfg(feature = "icons")]
        "icons",
        #[cfg(feature = "jpeg")]
        "jpeg",
        #[cfg(feature = "layout-cytoscape")]
        "layout-cytoscape",
        #[cfg(feature = "layout-elk")]
        "layout-elk",
        #[cfg(feature = "markdown")]
        "markdown",
        #[cfg(feature = "math")]
        "math",
        #[cfg(feature = "network-icons")]
        "network-icons",
        #[cfg(feature = "parallel-markdown")]
        "parallel-markdown",
        #[cfg(feature = "pdf")]
        "pdf",
        #[cfg(feature = "png")]
        "png",
        #[cfg(feature = "rustdoc")]
        "rustdoc",
        #[cfg(feature = "shell-completions")]
        "shell-completions",
        #[cfg(feature = "svg")]
        "svg",
        #[cfg(feature = "system-clock")]
        "system-clock",
        #[cfg(feature = "system-random")]
        "system-random",
        #[cfg(feature = "system-timezone")]
        "system-timezone",
        #[cfg(feature = "system-timing")]
        "system-timing",
    ];
    capabilities.sort_unstable();
    capabilities
}

fn expected_commands(capabilities: &[&str]) -> Vec<String> {
    let capabilities = capabilities.iter().copied().collect::<BTreeSet<_>>();
    let mut commands = BTreeSet::from([
        "capabilities".to_string(),
        "detect".to_string(),
        "parse".to_string(),
    ]);
    if capabilities.contains("analysis") {
        commands.extend(["fix", "lint", "lint-rules"].map(str::to_string));
    }
    if capabilities.contains("svg") {
        commands.extend(["layout", "mmdc", "render"].map(str::to_string));
    } else if capabilities.contains("ascii") {
        commands.insert("render".to_string());
    }
    if capabilities.contains("markdown") {
        commands.insert("batch".to_string());
    }
    if capabilities.contains("shell-completions") {
        commands.insert("completion".to_string());
    }
    if capabilities.contains("rustdoc") {
        commands.insert("rustdoc".to_string());
    }
    commands.into_iter().collect()
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path.as_ref()).expect("read JSON contract"))
        .expect("parse JSON contract")
}

fn sorted_objects_by_id(values: impl Iterator<Item = Value>) -> Vec<Value> {
    let mut values = values.collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left["id"]
            .as_str()
            .expect("descriptor id")
            .cmp(right["id"].as_str().expect("descriptor id"))
    });
    values
}

fn assert_capability_document(case: &str, payload: &Value) {
    let root = repo_root();
    let surface = read_json(root.join("capabilities/feature-surface-v1.json"));
    let profiles = read_json(root.join("capabilities/artifact-profiles-v1.json"));
    let expected_ids = expected_capabilities(case);
    let expected_id_set = expected_ids.iter().copied().collect::<BTreeSet<_>>();
    let expected_commands = expected_commands(&expected_ids);

    assert_eq!(payload["schema_version"], 2);
    assert_eq!(payload["cli_contract_version"], 4);
    assert_eq!(payload["package"]["name"], "merman-cli");
    assert_eq!(payload["package"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        payload["compatibility"],
        json!({
            "mermaid": merman::baseline::PINNED_MERMAID_BASELINE_VERSION,
            "mmdc": merman::baseline::PINNED_MERMAID_CLI_VERSION,
        })
    );
    assert_eq!(
        payload["descriptor"],
        json!({
            "schema_version": profiles["capability_authority"]["schema_version"],
            "digest": profiles["capability_authority"]["digest"],
        }),
        "runtime provenance must match the artifact-profile authority"
    );
    assert_eq!(payload["commands"], json!(expected_commands));

    let expected_capability_objects = sorted_objects_by_id(
        surface["capabilities"]
            .as_array()
            .expect("capability descriptors")
            .iter()
            .filter(|entry| {
                entry["id"]
                    .as_str()
                    .is_some_and(|id| expected_id_set.contains(id))
            })
            .map(|entry| {
                json!({
                    "id": entry["id"],
                    "kind": entry["kind"],
                    "description": entry["description"],
                    "implications": entry["implications"],
                })
            }),
    );
    assert_eq!(
        payload["capabilities"],
        json!(expected_capability_objects),
        "compiled capability objects drifted for matrix case {case}"
    );

    let expected_outputs = sorted_objects_by_id(
        surface["outputs"]
            .as_array()
            .expect("output descriptors")
            .iter()
            .filter(|entry| {
                entry["capability"]
                    .as_str()
                    .is_some_and(|id| expected_id_set.contains(id))
            })
            .map(|entry| {
                let mut output = json!({
                    "id": entry["id"],
                    "description": entry["description"],
                    "media_type": entry["media_type"],
                    "system_fonts": null,
                    "embedded_images": null,
                });
                if matches!(entry["id"].as_str(), Some("jpeg" | "pdf" | "png")) {
                    output["system_fonts"] = json!({
                        "source_id": "host-system",
                        "discovery": "first-use",
                        "cache_scope": "process-global",
                        "host_dependent": true,
                        "caller_configurable": false,
                        "resource_bounded": false,
                    });
                    output["embedded_images"] = json!({
                        "source_ids": ["data-url"],
                        "filesystem_access": false,
                        "network_access": false,
                        "caller_configurable": true,
                        "limits": {
                            "max_bytes_per_image": 16 * 1024 * 1024,
                            "max_total_bytes": 32 * 1024 * 1024,
                            "max_pixels_per_image": 16 * 1024 * 1024,
                            "max_total_pixels": 32 * 1024 * 1024,
                        },
                    });
                }
                output
            }),
    );
    assert_eq!(payload["outputs"], json!(expected_outputs));

    if case == "release" {
        let release = profiles["profiles"]
            .as_array()
            .expect("artifact profiles")
            .iter()
            .find(|profile| profile["id"] == "cli-release")
            .expect("cli-release profile");
        assert_eq!(
            release["expected"]["capabilities"],
            json!(expected_ids),
            "the release feature matrix must follow cli-release"
        );
        assert_eq!(
            release["expected"]["outputs"],
            object_ids(&payload["outputs"])
        );
    }
}

fn object_ids(values: &Value) -> Value {
    Value::Array(
        values
            .as_array()
            .expect("JSON array")
            .iter()
            .map(|entry| entry["id"].clone())
            .collect(),
    )
}

fn assert_compiled_surface(case: &str) {
    let capabilities = run_without_stdin(&["capabilities", "--json"], None);
    assert_success(&capabilities, "capabilities --json");
    assert!(
        capabilities.stderr.is_empty(),
        "capability reporting must keep stderr empty"
    );
    let payload: Value =
        serde_json::from_slice(&capabilities.stdout).expect("capabilities JSON document");
    assert_capability_document(case, &payload);

    let help = run_without_stdin(&["--help"], None);
    assert_success(&help, "root help");
    let help = String::from_utf8(help.stdout).expect("root help UTF-8");
    let expected_capabilities = expected_capabilities(case);
    let commands = expected_commands(&expected_capabilities)
        .into_iter()
        .collect::<BTreeSet<_>>();
    for command in ALL_OPTIONAL_COMMANDS {
        let line = format!("\n  {command:<14} ");
        assert_eq!(
            help.contains(&line),
            commands.contains(*command),
            "root help command surface drifted for {command:?} in case {case}:\n{help}"
        );
    }

    let parse_help = run_without_stdin(&["parse", "--help"], None);
    assert_success(&parse_help, "parse help");
    let parse_help = String::from_utf8(parse_help.stdout).expect("parse help UTF-8");
    let capabilities = expected_capabilities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for (capability, flag) in [
        ("system-clock", "--system-clock"),
        ("system-timezone", "--system-timezone"),
        ("system-random", "--system-random"),
        ("system-timing", "--system-timing"),
    ] {
        assert_eq!(
            parse_help.contains(flag),
            capabilities.contains(capability),
            "{flag} help ownership drifted in case {case}"
        );
    }

    let native_available = ["system-clock", "system-timezone", "system-random"]
        .iter()
        .all(|capability| capabilities.contains(capability));
    let native = run(
        &["parse", "--runtime", "native", "-"],
        SIMPLE_SOURCE.as_bytes(),
        None,
    );
    assert_eq!(
        native.status.success(),
        native_available,
        "--runtime native surface drifted in case {case}: {}",
        String::from_utf8_lossy(&native.stderr)
    );

    assert_option_ownership(case, &capabilities, &commands);
}

fn assert_option_ownership(case: &str, capabilities: &BTreeSet<&str>, commands: &BTreeSet<String>) {
    if commands.contains("render") {
        let help = command_help("render");
        for (capability, flag) in [
            ("svg", "--svg-pipeline"),
            ("ascii", "--ascii-charset"),
            ("icons", "--icon-pack-source"),
            ("network-icons", "--allow-network"),
            ("pdf", "--pdf-filter-scale"),
        ] {
            assert_eq!(
                help.contains(flag),
                capabilities.contains(capability),
                "{flag} help ownership drifted in case {case}:\n{help}"
            );
        }
        assert_eq!(
            help.contains("--raster-max-width"),
            capabilities.contains("png") || capabilities.contains("jpeg"),
            "raster option ownership drifted in case {case}:\n{help}"
        );

        for (capability, format) in [
            ("svg", "svg"),
            ("ascii", "ascii"),
            ("png", "png"),
            ("jpeg", "jpg"),
            ("pdf", "pdf"),
        ] {
            if !capabilities.contains(capability) {
                assert_usage_error(
                    &["render", "--format", format, "-"],
                    &format!("{format} must be absent in case {case}"),
                );
            }
        }
    }

    if commands.contains("batch") {
        let help = command_help("batch");
        assert_eq!(
            help.contains("--jobs"),
            capabilities.contains("parallel-markdown"),
            "--jobs ownership drifted in case {case}:\n{help}"
        );
        assert_eq!(
            help.contains("--allow-network"),
            capabilities.contains("network-icons"),
            "batch network option ownership drifted in case {case}:\n{help}"
        );
    }

    if commands.contains("mmdc") {
        let help = command_help("mmdc");
        assert_eq!(
            help.contains("--artefacts"),
            capabilities.contains("markdown"),
            "mmdc Markdown option ownership drifted in case {case}:\n{help}"
        );
        assert_eq!(
            help.contains("--jobs"),
            capabilities.contains("parallel-markdown"),
            "mmdc parallel option ownership drifted in case {case}:\n{help}"
        );
    }
}

fn command_help(command: &str) -> String {
    let output = run_without_stdin(&[command, "--help"], None);
    assert_success(&output, &format!("{command} help"));
    String::from_utf8(output.stdout).expect("command help UTF-8")
}

fn assert_usage_error(args: &[&str], context: &str) {
    let output = run_without_stdin(args, None);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{context}: expected usage error, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn workflow_base() {
    let detect = run(&["detect", "-"], SIMPLE_SOURCE.as_bytes(), None);
    assert_success(&detect, "detect stdin");
    assert_eq!(detect.stdout, b"flowchart-v2\n");

    let parse = run(&["parse", "-"], SIMPLE_SOURCE.as_bytes(), None);
    assert_success(&parse, "parse stdin");
    let payload: Value = serde_json::from_slice(&parse.stdout).expect("parse JSON");
    assert!(payload.is_object());
}

fn workflow_analysis() {
    #[cfg(feature = "analysis")]
    {
        let lint = run(
            &["lint", "--format", "json", "-"],
            b"sequenceDiagram\nAlice->>Bob: Hello\n",
            None,
        );
        assert!(
            matches!(lint.status.code(), Some(0 | 1)),
            "lint should complete its analysis workflow: {}",
            String::from_utf8_lossy(&lint.stderr)
        );
        let payload: Value = serde_json::from_slice(&lint.stdout).expect("lint JSON");
        assert!(payload["diagnostics"].is_array());

        let fix = run(&["fix", "-"], b"flowchart\nA-->B\n", None);
        assert_success(&fix, "fix stdin");
        assert!(fix.stdout.starts_with(b"flowchart TB\n"));
    }
    #[cfg(not(feature = "analysis"))]
    panic!("analysis matrix case was built without feature analysis");
}

#[cfg(feature = "svg")]
fn assert_svg(bytes: &[u8]) {
    let text = std::str::from_utf8(bytes).expect("SVG UTF-8");
    assert!(text.trim_start().starts_with("<svg"), "not SVG:\n{text}");
    roxmltree::Document::parse(text).expect("valid SVG XML");
}

fn workflow_svg() {
    #[cfg(feature = "svg")]
    {
        let stdout = run(
            &["render", "--format", "svg", "-"],
            SIMPLE_SOURCE.as_bytes(),
            None,
        );
        assert_success(&stdout, "SVG stdout");
        assert_svg(&stdout.stdout);

        let temp = tempfile::tempdir().expect("tempdir");
        let output_path = temp.path().join("atomic.svg");
        let output = run(
            &[
                "render",
                "--format",
                "svg",
                "--output",
                output_path.to_string_lossy().as_ref(),
                "-",
            ],
            SIMPLE_SOURCE.as_bytes(),
            None,
        );
        assert_success(&output, "SVG file");
        assert!(output.stdout.is_empty());
        assert_svg(&fs::read(output_path).expect("read SVG file"));
    }
    #[cfg(not(feature = "svg"))]
    panic!("SVG matrix case was built without feature svg");
}

fn workflow_mmdc_native_format_guidance() {
    #[cfg(feature = "svg")]
    {
        for (format, feature, available) in [
            ("jpg", "jpeg", cfg!(feature = "jpeg")),
            ("ascii", "ascii", cfg!(feature = "ascii")),
            ("unicode", "ascii", cfg!(feature = "ascii")),
        ] {
            let output = run(
                &["mmdc", "-i", "-", "-o", "-", "-e", format],
                SIMPLE_SOURCE.as_bytes(),
                None,
            );
            assert_eq!(
                output.status.code(),
                Some(2),
                "mmdc native-only {format} should remain a usage error"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            if available {
                assert!(
                    stderr.contains(&format!("merman-cli render -f {format}")),
                    "compiled native output needs an executable replacement: {stderr}"
                );
            } else {
                assert!(
                    stderr.contains(&format!("without the `{feature}` feature"))
                        && !stderr.contains(&format!("merman-cli render -f {format}")),
                    "slim builds must not recommend unavailable output: {stderr}"
                );
            }
        }

        let temp = tempfile::tempdir().expect("create mmdc extension fixture");
        fs::write(temp.path().join("diagram.mmd"), SIMPLE_SOURCE).expect("write Mermaid input");
        let output = run_without_stdin(
            &["mmdc", "-i", "diagram.mmd", "-o", "out.jpg"],
            Some(temp.path()),
        );
        assert_eq!(
            output.status.code(),
            Some(2),
            "mmdc native-only output extension should remain a usage error"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        if cfg!(feature = "jpeg") {
            assert!(
                stderr.contains("merman-cli render -f jpg"),
                "compiled JPEG output needs an executable extension replacement: {stderr}"
            );
        } else {
            assert!(
                stderr.contains("without the `jpeg` feature")
                    && !stderr.contains("merman-cli render -f jpg"),
                "slim builds must not recommend unavailable extension output: {stderr}"
            );
        }
        assert!(
            !temp.path().join("out.jpg").exists(),
            "extension guidance must precede output effects"
        );
    }
}

fn workflow_ascii() {
    #[cfg(feature = "ascii")]
    {
        let output = run(
            &["render", "--format", "unicode", "-"],
            b"sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello\n",
            None,
        );
        assert_success(&output, "Unicode output");
        let text = String::from_utf8(output.stdout).expect("Unicode output UTF-8");
        assert!(text.contains('┌') && text.contains("Hello"), "{text}");
    }
    #[cfg(not(feature = "ascii"))]
    panic!("ASCII matrix case was built without feature ascii");
}

fn workflow_local_icons() {
    #[cfg(feature = "icons")]
    {
        let temp = tempfile::tempdir().expect("tempdir");
        let icon_path = temp.path().join("icons.json");
        fs::write(&icon_path, ICON_BODY).expect("write local icon fixture");
        let source = format!("test#{}", icon_path.display());
        let output = run(
            &[
                "render",
                "--format",
                "svg",
                "--icon-pack-source",
                &source,
                "-",
            ],
            ICON_SOURCE.as_bytes(),
            None,
        );
        assert_success(&output, "local icon render");
        assert!(String::from_utf8_lossy(&output.stdout).contains("data-profile-icon=\"rocket\""));
    }
    #[cfg(not(feature = "icons"))]
    panic!("icons matrix case was built without feature icons");
}

#[cfg(feature = "network-icons")]
fn serve_icon_body_once() -> (String, std::thread::JoinHandle<Result<(), String>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind icon fixture");
    let address = listener.local_addr().expect("icon fixture address");
    listener
        .set_nonblocking(true)
        .expect("make icon fixture listener nonblocking");
    let thread = std::thread::spawn(move || -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err("timed out waiting for icon request".to_owned());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(format!("accept icon request: {error}")),
            }
        };
        stream
            .set_nonblocking(false)
            .map_err(|error| format!("make icon fixture stream blocking: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| format!("set icon fixture timeout: {error}"))?;
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream
                .read(&mut buffer)
                .map_err(|error| format!("read icon request: {error}"))?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        if !request.starts_with(b"GET /icons.json") {
            return Err("icon fixture received an unexpected request target".to_owned());
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            ICON_BODY.len(),
            ICON_BODY
        );
        stream
            .write_all(response.as_bytes())
            .map_err(|error| format!("write icon response: {error}"))?;
        Ok(())
    });
    (format!("http://{address}/icons.json"), thread)
}

fn workflow_network_icons() {
    #[cfg(feature = "network-icons")]
    {
        let denied = run(
            &[
                "render",
                "--format",
                "svg",
                "--allow-network",
                "--icon-pack-source",
                "test#http://127.0.0.1:9/icons.json",
                "-",
            ],
            ICON_SOURCE.as_bytes(),
            None,
        );
        assert_eq!(denied.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&denied.stderr).contains("private network authorization"));

        let (url, server) = serve_icon_body_once();
        let source = format!("test#{url}");
        let output = run(
            &[
                "render",
                "--format",
                "svg",
                "--allow-network",
                "--allow-private-network",
                "--icon-pack-source",
                &source,
                "-",
            ],
            ICON_SOURCE.as_bytes(),
            None,
        );
        server
            .join()
            .expect("icon fixture thread")
            .expect("icon fixture completion");
        assert_success(&output, "network icon render");
        assert!(String::from_utf8_lossy(&output.stdout).contains("data-profile-icon=\"rocket\""));
    }
    #[cfg(not(feature = "network-icons"))]
    panic!("network matrix case was built without feature network-icons");
}

fn workflow_batch(format: &str, jobs: Option<&str>) {
    #[cfg(feature = "markdown")]
    {
        let temp = tempfile::tempdir().expect("tempdir");
        let input = temp.path().join("input.md");
        fs::write(
            &input,
            concat!(
                "# Profiles\n\n",
                "```mermaid\nflowchart LR\nA-->B\n```\n\n",
                "```mermaid\nsequenceDiagram\nA->>B: Hello\n```\n",
            ),
        )
        .expect("write Markdown");
        // Exercise recovery of an interrupted bootstrap transaction before the
        // process creates and publishes the new generation.
        let bootstrap_transaction = temp.path().join("generated/.merman.transaction");
        fs::create_dir_all(&bootstrap_transaction)
            .expect("write recoverable bootstrap transaction");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&bootstrap_transaction, fs::Permissions::from_mode(0o700))
                .expect("restrict bootstrap transaction permissions");
        }
        let mut args = vec![
            "batch",
            "input.md",
            "--output-dir",
            "generated",
            "--format",
            format,
            "--quiet",
        ];
        if let Some(jobs) = jobs {
            args.extend(["--jobs", jobs]);
        }
        let output = run(&args, b"", Some(temp.path()));
        assert_success(&output, "Markdown batch");
        let extension = if format == "jpg" { "jpg" } else { format };
        let first = fs::read(temp.path().join(format!("generated/input-1.{extension}")))
            .expect("read first Markdown artifact");
        let second = fs::read(temp.path().join(format!("generated/input-2.{extension}")))
            .expect("read second Markdown artifact");
        match format {
            "svg" => {
                assert_svg(&first);
                assert_svg(&second);
            }
            "pdf" => {
                assert!(first.starts_with(b"%PDF-"));
                assert!(second.starts_with(b"%PDF-"));
            }
            other => panic!("unsupported matrix batch format {other}"),
        }
        let document =
            fs::read_to_string(temp.path().join("generated/input.md")).expect("rewritten Markdown");
        assert!(document.contains(&format!("./input-1.{extension}")));
        assert!(document.contains(&format!("./input-2.{extension}")));
        assert!(temp.path().join("generated/.merman.lock").is_file());
        assert!(!temp.path().join("generated/.merman.transaction").exists());
    }
    #[cfg(not(feature = "markdown"))]
    {
        let _ = (format, jobs);
        panic!("Markdown matrix case was built without feature markdown");
    }
}

fn workflow_raster(format: &str, signature: &[u8]) {
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_path = temp.path().join(format!("out.{format}"));
        let output = run(
            &[
                "render",
                "--format",
                format,
                "--output",
                output_path.to_string_lossy().as_ref(),
                "-",
            ],
            SIMPLE_SOURCE.as_bytes(),
            None,
        );
        assert_success(&output, "raster/PDF output");
        let bytes = fs::read(output_path).expect("read encoded output");
        assert!(bytes.starts_with(signature), "wrong {format} signature");
    }
    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    {
        let _ = (format, signature);
        panic!("encoded output matrix case lacks an encoder feature");
    }
}

fn workflow_cytoscape() {
    #[cfg(feature = "layout-cytoscape")]
    {
        let output = run(
            &["render", "--format", "svg", "-"],
            b"architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R -- L:db\n",
            None,
        );
        assert_success(&output, "Cytoscape architecture render");
        assert_svg(&output.stdout);
    }
    #[cfg(not(feature = "layout-cytoscape"))]
    panic!("Cytoscape matrix case lacks layout-cytoscape");
}

fn workflow_elk() {
    #[cfg(feature = "layout-elk")]
    {
        let output = run(
            &["render", "--format", "svg", "-"],
            b"flowchart-elk LR\nA[Start] --> B[Done]\n",
            None,
        );
        assert_success(&output, "ELK render");
        assert_svg(&output.stdout);
    }
    #[cfg(not(feature = "layout-elk"))]
    panic!("ELK matrix case lacks layout-elk");
}

fn workflow_math() {
    #[cfg(feature = "math")]
    {
        let output = run(
            &["render", "--format", "svg", "--math-renderer", "ratex", "-"],
            b"flowchart LR\nA[\"$$x^2$$\"] --> B[Done]\n",
            None,
        );
        assert_success(&output, "RaTeX render");
        let svg = String::from_utf8(output.stdout).expect("RaTeX SVG UTF-8");
        assert!(svg.contains("<path") && !svg.contains("$$x^2$$"), "{svg}");
    }
    #[cfg(not(feature = "math"))]
    panic!("math matrix case lacks math");
}

fn workflow_rustdoc() {
    #[cfg(feature = "rustdoc")]
    {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("diagram.mmd"), SIMPLE_SOURCE).expect("write Rustdoc source");
        fs::write(
            temp.path().join("merman-rustdoc.toml"),
            "schema = 1\n[[fragments]]\nid = \"overview\"\nsource = \"diagram.mmd\"\n",
        )
        .expect("write Rustdoc config");

        let build = run_without_stdin(&["rustdoc", "build", "--quiet"], Some(temp.path()));
        assert_success(&build, "Rustdoc build");
        let fragment = fs::read_to_string(
            temp.path()
                .join("docs/generated/merman-rustdoc/overview.md"),
        )
        .expect("read generated Rustdoc fragment");
        assert!(fragment.contains("data-merman-rustdoc-theme=\"light\""));
        assert!(fragment.contains("data-merman-rustdoc-theme=\"dark\""));

        let check = run_without_stdin(&["rustdoc", "check", "--quiet"], Some(temp.path()));
        assert_success(&check, "Rustdoc check");
    }
    #[cfg(not(feature = "rustdoc"))]
    panic!("Rustdoc matrix case lacks feature rustdoc");
}

fn workflow_completions() {
    #[cfg(feature = "shell-completions")]
    {
        let output = run_without_stdin(&["completion", "bash"], None);
        assert_success(&output, "Bash completion");
        let script = String::from_utf8(output.stdout).expect("completion UTF-8");
        let capabilities = expected_capabilities(&selected_case());
        let commands = expected_commands(&capabilities)
            .into_iter()
            .collect::<BTreeSet<_>>();
        for command in ALL_OPTIONAL_COMMANDS {
            let token = format!("merman__cli,{command})");
            assert_eq!(
                script.contains(&token),
                commands.contains(*command),
                "completion command surface drifted for {command:?}"
            );
        }

        #[cfg(feature = "svg")]
        {
            let render = bash_completion_options(&script, "render");
            assert!(
                render.contains("-f") && !render.contains("-e"),
                "native completion must expose -f without deprecated -e: {render:?}"
            );
            let mmdc = bash_completion_options(&script, "mmdc");
            assert!(
                mmdc.contains("-e"),
                "mmdc completion must retain its permanent -e: {mmdc:?}"
            );

            let format_values = bash_completion_values(&script, "render", "--format");
            let expected_formats = [
                ("svg", cfg!(feature = "svg")),
                ("ascii", cfg!(feature = "ascii")),
                ("unicode", cfg!(feature = "ascii")),
                ("png", cfg!(feature = "png")),
                ("jpg", cfg!(feature = "jpeg")),
                ("pdf", cfg!(feature = "pdf")),
            ]
            .into_iter()
            .filter_map(|(format, enabled)| enabled.then_some(format.to_owned()))
            .collect::<BTreeSet<_>>();
            assert_eq!(format_values, expected_formats);

            let theme_values = bash_completion_values(&script, "render", "--theme");
            assert_eq!(
                theme_values,
                merman::supported_themes()
                    .iter()
                    .map(|theme| (*theme).to_owned())
                    .collect()
            );

            let presentation_profiles =
                bash_completion_values(&script, "render", "--presentation-profile");
            assert_eq!(
                presentation_profiles,
                merman::svg::presentation_profile_descriptors()
                    .iter()
                    .map(|descriptor| descriptor.id().to_owned())
                    .collect()
            );
        }
    }
    #[cfg(not(feature = "shell-completions"))]
    panic!("completion matrix case lacks shell-completions");
}

#[cfg(all(feature = "shell-completions", feature = "svg"))]
fn bash_completion_block<'a>(script: &'a str, command: &str) -> &'a str {
    let marker = format!("\n        merman__cli__subcmd__{command})");
    let start = script
        .find(&marker)
        .unwrap_or_else(|| panic!("Bash completion omits {command:?} state"));
    let rest = &script[start + marker.len()..];
    let end = rest
        .find("\n        merman__cli__subcmd__")
        .or_else(|| rest.find("\n    esac"))
        .unwrap_or(rest.len());
    &rest[..end]
}

#[cfg(all(feature = "shell-completions", feature = "svg"))]
fn bash_completion_options(script: &str, command: &str) -> BTreeSet<String> {
    bash_completion_block(script, command)
        .lines()
        .find_map(|line| line.trim().strip_prefix("opts=\""))
        .and_then(|line| line.strip_suffix('"'))
        .unwrap_or_else(|| panic!("Bash completion omits {command:?} options"))
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect()
}

#[cfg(all(feature = "shell-completions", feature = "svg"))]
fn bash_completion_values(script: &str, command: &str, option: &str) -> BTreeSet<String> {
    let block = bash_completion_block(script, command);
    let marker = format!("                {option})");
    let rest = block
        .split_once(&marker)
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| panic!("Bash completion omits {command} {option}"));
    rest.split_once("compgen -W \"")
        .and_then(|(_, rest)| rest.split_once('"').map(|(values, _)| values))
        .unwrap_or_else(|| panic!("Bash completion omits {command} {option} values"))
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect()
}

fn workflow_adapter(flag: &str) {
    match flag {
        "--system-clock" => {
            const SOURCE: &str = "gantt\ndateFormat MM-DD\nsection Demo\nTask: id1,03-01,1d\n";
            let fixed = parse_with_adapter(&[], SOURCE.as_bytes(), &[]);
            let system = parse_with_adapter(&[flag], SOURCE.as_bytes(), &[]);
            assert_ne!(
                system["tasks"][0]["startTime"], fixed["tasks"][0]["startTime"],
                "system clock did not replace the deterministic epoch"
            );
        }
        "--system-timezone" => {
            const SOURCE: &str =
                "gantt\ndateFormat YYYY-MM-DD\nsection Demo\nTask: id1,2026-01-15,1d\n";
            let system =
                parse_with_adapter(&[flag], SOURCE.as_bytes(), &[("TZ", "America/New_York")]);
            assert_eq!(
                system["tasks"][0]["startTime"], 1_768_453_200_000_i64,
                "system time-zone rules did not resolve New York winter midnight"
            );
        }
        "--system-random" => {
            const SOURCE: &str = "gitGraph:\ncommit\n";
            let fixed = parse_with_adapter(&[], SOURCE.as_bytes(), &[]);
            let fixed_id = fixed["commits"][0]["id"]
                .as_str()
                .expect("deterministic GitGraph commit id")
                .to_string();
            let observed_system_value = (0..3).any(|_| {
                let system = parse_with_adapter(&[flag], SOURCE.as_bytes(), &[]);
                system["commits"][0]["id"].as_str() != Some(fixed_id.as_str())
            });
            assert!(
                observed_system_value,
                "system random did not replace the deterministic operation seed"
            );
        }
        "--system-timing" => {
            let output = run(&["parse", flag, "-"], SIMPLE_SOURCE.as_bytes(), None);
            assert_success(&output, flag);
            let _: Value = serde_json::from_slice(&output.stdout).expect("adapter parse JSON");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("[parse-timing]"),
                "system timing adapter did not emit timing diagnostics: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        other => panic!("unknown runtime adapter {other:?}"),
    }
}

fn parse_with_adapter(adapter_args: &[&str], source: &[u8], environment: &[(&str, &str)]) -> Value {
    let mut args = vec!["parse"];
    args.extend_from_slice(adapter_args);
    args.push("-");
    let output = run_with_environment(&args, source, None, environment);
    assert_success(&output, &args.join(" "));
    serde_json::from_slice(&output.stdout).expect("adapter parse JSON")
}

fn workflow_release() {
    workflow_base();
    workflow_analysis();
    workflow_svg();
    workflow_ascii();
    workflow_local_icons();
    workflow_batch("svg", None);
    workflow_network_icons();
    workflow_raster("png", b"\x89PNG\r\n\x1a\n");
    workflow_raster("jpg", &[0xff, 0xd8, 0xff]);
    workflow_raster("pdf", b"%PDF-");
    workflow_batch("pdf", Some("2"));
    workflow_cytoscape();
    workflow_elk();
    workflow_math();
    workflow_rustdoc();
    workflow_completions();
    for flag in [
        "--system-clock",
        "--system-timezone",
        "--system-random",
        "--system-timing",
    ] {
        workflow_adapter(flag);
    }
}

fn workflow_default() {
    workflow_base();
    workflow_analysis();
    workflow_svg();
    workflow_ascii();
    workflow_local_icons();
    workflow_batch("svg", None);
    workflow_network_icons();
    workflow_raster("png", b"\x89PNG\r\n\x1a\n");
    workflow_raster("jpg", &[0xff, 0xd8, 0xff]);
    workflow_raster("pdf", b"%PDF-");
    workflow_batch("pdf", Some("2"));
    workflow_cytoscape();
    workflow_math();
    workflow_rustdoc();
    workflow_completions();
    for flag in [
        "--system-clock",
        "--system-timezone",
        "--system-random",
        "--system-timing",
    ] {
        workflow_adapter(flag);
    }
}

fn workflow_auto() {
    workflow_base();
    #[cfg(feature = "analysis")]
    workflow_analysis();
    #[cfg(feature = "svg")]
    workflow_svg();
    #[cfg(feature = "ascii")]
    workflow_ascii();
    #[cfg(feature = "icons")]
    workflow_local_icons();
    #[cfg(feature = "markdown")]
    workflow_batch("svg", None);
    #[cfg(feature = "network-icons")]
    workflow_network_icons();
    #[cfg(feature = "png")]
    workflow_raster("png", b"\x89PNG\r\n\x1a\n");
    #[cfg(feature = "jpeg")]
    workflow_raster("jpg", &[0xff, 0xd8, 0xff]);
    #[cfg(feature = "pdf")]
    workflow_raster("pdf", b"%PDF-");
    #[cfg(all(feature = "parallel-markdown", feature = "pdf"))]
    workflow_batch("pdf", Some("2"));
    #[cfg(all(feature = "parallel-markdown", not(feature = "pdf")))]
    workflow_batch("svg", Some("2"));
    #[cfg(feature = "layout-cytoscape")]
    workflow_cytoscape();
    #[cfg(feature = "layout-elk")]
    workflow_elk();
    #[cfg(feature = "math")]
    workflow_math();
    #[cfg(feature = "rustdoc")]
    workflow_rustdoc();
    #[cfg(feature = "shell-completions")]
    workflow_completions();
    #[cfg(feature = "system-clock")]
    workflow_adapter("--system-clock");
    #[cfg(feature = "system-timezone")]
    workflow_adapter("--system-timezone");
    #[cfg(feature = "system-random")]
    workflow_adapter("--system-random");
    #[cfg(feature = "system-timing")]
    workflow_adapter("--system-timing");
}

fn execute_primary_workflow(case: &str) {
    match case {
        "base" => workflow_base(),
        "analysis" => workflow_analysis(),
        "svg" => workflow_svg(),
        "ascii" => workflow_ascii(),
        "local-icons" => workflow_local_icons(),
        "markdown" => workflow_batch("svg", None),
        "parallel-markdown" => workflow_batch("svg", Some("2")),
        "network-icons" => workflow_network_icons(),
        "png" => workflow_raster("png", b"\x89PNG\r\n\x1a\n"),
        "jpeg" => workflow_raster("jpg", &[0xff, 0xd8, 0xff]),
        "pdf" => workflow_raster("pdf", b"%PDF-"),
        "parallel-pdf" => workflow_batch("pdf", Some("2")),
        "cytoscape-layout" => workflow_cytoscape(),
        "elk-layout" => workflow_elk(),
        "math" => workflow_math(),
        "rustdoc" => workflow_rustdoc(),
        "completions" => workflow_completions(),
        "svg-completions" => workflow_completions(),
        "system-clock" => workflow_adapter("--system-clock"),
        "system-timezone" => workflow_adapter("--system-timezone"),
        "system-random" => workflow_adapter("--system-random"),
        "system-timing" => workflow_adapter("--system-timing"),
        "default" => workflow_default(),
        "release" => workflow_release(),
        "auto" => workflow_auto(),
        other => panic!("unknown {CASE_ENV} value {other:?}"),
    }
}

#[test]
fn exact_compiled_profile_is_callable() {
    let case = selected_case();
    assert_compiled_surface(&case);
    execute_primary_workflow(&case);
    workflow_mmdc_native_format_guidance();
}
