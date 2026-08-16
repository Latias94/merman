use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

const VALID_SOURCE: &str = "flowchart LR\nA --> B\n";

fn cli() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("merman-cli").to_path_buf()
}

fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new(cli())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run merman-cli")
}

fn exit_code(output: &Output) -> i32 {
    output.status.code().expect("CLI should exit normally")
}

fn write_source(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("source parent")).expect("create source parent");
    fs::write(path, VALID_SOURCE).expect("write source");
}

fn write_config(root: &Path, source: &str) {
    fs::write(root.join("merman-rustdoc.toml"), source).expect("write Rustdoc config");
}

fn valid_config(source: &str) -> String {
    let source = toml::Value::String(source.to_string()).to_string();
    format!(
        "schema = 1\n\n[[fragments]]\nid = \"architecture\"\nsource = {source}\nsource_display = \"details\"\n"
    )
}

fn assert_config_error(root: &Path, config: &str, needles: &[&str]) {
    write_config(root, config);
    let output = run(root, &["rustdoc", "check"]);
    assert_eq!(
        exit_code(&output),
        2,
        "configuration errors must use the usage exit code\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "failure must not write stdout");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    for needle in needles {
        assert!(
            stderr.contains(needle),
            "stderr should contain {needle:?}:\n{stderr}"
        );
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TreeSnapshotEntry {
    kind: TreeSnapshotKind,
    modified: SystemTime,
}

#[derive(Debug, PartialEq, Eq)]
enum TreeSnapshotKind {
    Directory,
    File(Vec<u8>),
    Symlink(PathBuf),
}

fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, TreeSnapshotEntry> {
    fn snapshot_entry(root: &Path, path: &Path) -> TreeSnapshotEntry {
        let metadata = fs::symlink_metadata(path).expect("inspect snapshot entry");
        let modified = metadata
            .modified()
            .expect("snapshot entry modification time");
        let kind = if metadata.file_type().is_dir() {
            TreeSnapshotKind::Directory
        } else if metadata.file_type().is_file() {
            TreeSnapshotKind::File(fs::read(path).expect("read snapshot file"))
        } else if metadata.file_type().is_symlink() {
            TreeSnapshotKind::Symlink(fs::read_link(path).expect("read snapshot symlink"))
        } else {
            panic!(
                "unsupported snapshot entry below {}: {path:?}",
                root.display()
            );
        };
        TreeSnapshotEntry { kind, modified }
    }

    fn visit(root: &Path, current: &Path, snapshot: &mut BTreeMap<PathBuf, TreeSnapshotEntry>) {
        let mut entries = fs::read_dir(current)
            .expect("read snapshot directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read snapshot entries");
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("snapshot path below root")
                .to_path_buf();
            let snapshot_entry = snapshot_entry(root, &path);
            let is_directory = matches!(snapshot_entry.kind, TreeSnapshotKind::Directory);
            snapshot.insert(relative, snapshot_entry);
            if is_directory {
                visit(root, &path, snapshot);
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    snapshot.insert(PathBuf::from("."), snapshot_entry(root, root));
    visit(root, root, &mut snapshot);
    snapshot
}

fn build_single_fragment_fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    write_source(root.path(), "diagram.mmd");
    write_config(root.path(), &valid_config("diagram.mmd"));
    let build = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&build), 0, "stderr: {:?}", build.stderr);
    root
}

fn assert_stale_check_is_read_only(root: &Path, scenario: &str) {
    let before = snapshot_tree(root);
    let output = run(root, &["rustdoc", "check", "--quiet"]);
    assert_eq!(
        exit_code(&output),
        1,
        "{scenario} must be stale; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        snapshot_tree(root),
        before,
        "{scenario} check changed filesystem bytes, types, or mtimes"
    );
}

#[test]
fn rustdoc_help_exposes_only_the_frozen_command_options() {
    let root = tempfile::tempdir().expect("tempdir");
    let root_help = run(root.path(), &["--help"]);
    assert!(root_help.status.success(), "stderr: {:?}", root_help.stderr);
    let root_help = String::from_utf8(root_help.stdout).expect("help should be UTF-8");
    assert!(
        root_help.contains("rustdoc"),
        "root help should expose the Rustdoc workflow:\n{root_help}"
    );

    let rustdoc_help = run(root.path(), &["rustdoc", "--help"]);
    assert!(
        rustdoc_help.status.success(),
        "stderr: {:?}",
        rustdoc_help.stderr
    );
    let rustdoc_help = String::from_utf8(rustdoc_help.stdout).expect("help should be UTF-8");
    for command in ["build", "check"] {
        assert!(
            rustdoc_help.contains(command),
            "Rustdoc help should expose {command}:\n{rustdoc_help}"
        );
    }

    for command in ["build", "check"] {
        let help = run(root.path(), &["rustdoc", command, "--help"]);
        assert!(help.status.success(), "stderr: {:?}", help.stderr);
        let help = String::from_utf8(help.stdout).expect("help should be UTF-8");
        for expected in ["--config", "--quiet", "merman-rustdoc.toml"] {
            assert!(
                help.contains(expected),
                "{command} help should contain {expected}:\n{help}"
            );
        }
        for forbidden in [
            "--output",
            "--resource-profile",
            "--theme",
            "--svg-pipeline",
            "--jobs",
        ] {
            assert!(
                !help.contains(forbidden),
                "{command} must not expose native render option {forbidden}:\n{help}"
            );
        }
    }
}

#[test]
fn check_reports_missing_receipt_without_creating_managed_outputs() {
    let default_root = tempfile::tempdir().expect("tempdir");
    write_source(default_root.path(), "docs/architecture.md");
    write_config(default_root.path(), &valid_config("docs/architecture.md"));

    let output = run(default_root.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(exit_code(&output), 1, "stderr: {:?}", output.stderr);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("Rustdoc output is stale"), "{stderr}");
    assert!(stderr.contains("receipt is missing"), "{stderr}");
    assert!(
        !default_root
            .path()
            .join("docs/generated/merman-rustdoc")
            .exists(),
        "U1 must not create an empty managed root"
    );

    let explicit_root = tempfile::tempdir().expect("tempdir");
    let config_root = explicit_root.path().join("package/config");
    fs::create_dir_all(&config_root).expect("create config root");
    write_source(&config_root, "sources/model.mmd");
    fs::write(
        config_root.join("custom.toml"),
        valid_config("sources/model.mmd"),
    )
    .expect("write explicit config");
    let output = run(
        explicit_root.path(),
        &["rustdoc", "check", "--config", "package/config/custom.toml"],
    );
    assert_eq!(exit_code(&output), 1, "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("receipt is missing"), "{stderr}");
    let expected_receipt = fs::canonicalize(&config_root)
        .expect("canonicalize config root")
        .join("docs/generated/merman-rustdoc/receipt.json");
    assert!(
        stderr.contains(&format!("{expected_receipt:?}")),
        "{stderr}"
    );
}

#[test]
fn stale_check_scenarios_preserve_tree_bytes_types_and_mtimes() {
    let missing_receipt = build_single_fragment_fixture();
    fs::remove_file(
        missing_receipt
            .path()
            .join("docs/generated/merman-rustdoc/receipt.json"),
    )
    .expect("remove receipt");
    assert_stale_check_is_read_only(missing_receipt.path(), "missing receipt");

    let missing_output = build_single_fragment_fixture();
    fs::remove_file(
        missing_output
            .path()
            .join("docs/generated/merman-rustdoc/architecture.md"),
    )
    .expect("remove generated output");
    assert_stale_check_is_read_only(missing_output.path(), "missing output");

    let tampered_output = build_single_fragment_fixture();
    fs::write(
        tampered_output
            .path()
            .join("docs/generated/merman-rustdoc/architecture.md"),
        b"tampered generated output",
    )
    .expect("tamper generated output");
    assert_stale_check_is_read_only(tampered_output.path(), "tampered output");

    let stale_source = build_single_fragment_fixture();
    fs::write(
        stale_source.path().join("diagram.mmd"),
        "flowchart LR\nChanged --> Source\n",
    )
    .expect("change Rustdoc source");
    assert_stale_check_is_read_only(stale_source.path(), "stale source");

    let extra_managed = tempfile::tempdir().expect("tempdir");
    write_source(extra_managed.path(), "one.mmd");
    write_source(extra_managed.path(), "two.mmd");
    write_config(
        extra_managed.path(),
        concat!(
            "schema = 1\n",
            "[[fragments]]\nid = \"one\"\nsource = \"one.mmd\"\n",
            "[[fragments]]\nid = \"two\"\nsource = \"two.mmd\"\n",
        ),
    );
    let build = run(extra_managed.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&build), 0, "stderr: {:?}", build.stderr);
    write_config(
        extra_managed.path(),
        "schema = 1\n[[fragments]]\nid = \"one\"\nsource = \"one.mmd\"\n",
    );
    assert_stale_check_is_read_only(extra_managed.path(), "extra previously-managed output");
}

#[test]
fn check_rejects_malformed_receipt_and_unfinished_transaction_as_operational() {
    let malformed = tempfile::tempdir().expect("tempdir");
    write_source(malformed.path(), "diagram.mmd");
    write_config(malformed.path(), &valid_config("diagram.mmd"));
    let managed = malformed.path().join("docs/generated/merman-rustdoc");
    fs::create_dir_all(&managed).expect("create managed root");
    fs::write(managed.join("receipt.json"), b"{").expect("write malformed receipt");

    let output = run(malformed.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(exit_code(&output), 3, "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("invalid Rustdoc receipt"), "{stderr}");
    assert!(stderr.contains("malformed JSON"), "{stderr}");

    let unfinished = tempfile::tempdir().expect("tempdir");
    write_source(unfinished.path(), "diagram.mmd");
    write_config(unfinished.path(), &valid_config("diagram.mmd"));
    let managed = unfinished.path().join("docs/generated/merman-rustdoc");
    fs::create_dir_all(managed.join(".merman.transaction")).expect("create transaction evidence");

    let output = run(unfinished.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(exit_code(&output), 3, "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("unfinished publication evidence"),
        "{stderr}"
    );

    fs::write(unfinished.path().join("diagram.mmd"), "not-a-diagram\n")
        .expect("replace source with invalid Mermaid");
    let output = run(unfinished.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(exit_code(&output), 3, "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("unfinished publication evidence"),
        "transaction evidence must take precedence over re-rendering: {stderr}"
    );
    assert!(!stderr.contains("not-a-diagram"), "{stderr}");
}

#[test]
fn build_check_repair_and_stale_cleanup_preserve_owned_boundaries() {
    let root = tempfile::tempdir().expect("tempdir");
    write_source(root.path(), "one.mmd");
    write_source(root.path(), "two.mmd");
    write_config(
        root.path(),
        concat!(
            "schema = 1\n",
            "[[fragments]]\n",
            "id = \"one\"\n",
            "source = \"one.mmd\"\n",
            "[[fragments]]\n",
            "id = \"two\"\n",
            "source = \"two.mmd\"\n",
        ),
    );

    let first = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&first), 0, "stderr: {:?}", first.stderr);
    assert!(first.stdout.is_empty());
    assert!(first.stderr.is_empty());

    let managed = root.path().join("docs/generated/merman-rustdoc");
    let one = managed.join("one.md");
    let two = managed.join("two.md");
    let receipt = managed.join("receipt.json");
    for path in [&one, &two, &receipt] {
        assert!(path.is_file(), "missing generated file {path:?}");
    }
    let one_text = fs::read_to_string(&one).expect("read generated fragment");
    let two_owned_bytes = fs::read(&two).expect("read generated stale candidate");
    assert!(one_text.contains("<svg"), "{one_text}");
    assert!(!one_text.contains("<script"), "{one_text}");
    let receipt_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&receipt).expect("read receipt")).expect("valid receipt");
    assert_eq!(
        receipt_json["managed_files"],
        serde_json::json!(["one.md", "receipt.json", "two.md"])
    );

    let check = run(root.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(exit_code(&check), 0, "stderr: {:?}", check.stderr);
    assert!(check.stdout.is_empty());
    assert!(check.stderr.is_empty());

    let one_identity = same_file::Handle::from_path(&one).expect("one identity");
    let receipt_identity = same_file::Handle::from_path(&receipt).expect("receipt identity");
    let one_mtime = fs::metadata(&one).unwrap().modified().unwrap();
    let receipt_mtime = fs::metadata(&receipt).unwrap().modified().unwrap();
    let no_op = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&no_op), 0, "stderr: {:?}", no_op.stderr);
    assert_eq!(
        same_file::Handle::from_path(&one).unwrap(),
        one_identity,
        "no-op build must not replace an unchanged fragment"
    );
    assert_eq!(
        same_file::Handle::from_path(&receipt).unwrap(),
        receipt_identity,
        "no-op build must not replace an unchanged receipt"
    );
    assert_eq!(fs::metadata(&one).unwrap().modified().unwrap(), one_mtime);
    assert_eq!(
        fs::metadata(&receipt).unwrap().modified().unwrap(),
        receipt_mtime
    );

    fs::write(&one, b"tampered").expect("tamper fragment");
    let stale = run(root.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(exit_code(&stale), 1, "stderr: {:?}", stale.stderr);
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("bytes differ"),
        "stderr: {}",
        String::from_utf8_lossy(&stale.stderr)
    );
    let repair = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&repair), 0, "stderr: {:?}", repair.stderr);
    assert!(fs::read_to_string(&one).unwrap().contains("<svg"));

    let unknown = managed.join("notes.txt");
    fs::write(&unknown, b"not owned by Merman").expect("write unknown neighbor");
    write_config(
        root.path(),
        &valid_config("one.mmd").replace("architecture", "one"),
    );
    fs::write(&two, b"user-owned replacement").expect("replace stale candidate");
    let refused_cleanup = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(
        exit_code(&refused_cleanup),
        3,
        "stderr: {:?}",
        refused_cleanup.stderr
    );
    assert_eq!(
        fs::read(&two).unwrap(),
        b"user-owned replacement",
        "receipt path alone must not authorize deleting changed content"
    );
    assert!(
        String::from_utf8_lossy(&refused_cleanup.stderr)
            .contains("no longer matches the output recorded by the prior receipt"),
        "stderr: {}",
        String::from_utf8_lossy(&refused_cleanup.stderr)
    );

    fs::write(&two, two_owned_bytes).expect("restore receipt-owned stale fragment");
    let cleanup = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&cleanup), 0, "stderr: {:?}", cleanup.stderr);
    assert!(
        !two.exists(),
        "receipt-owned stale fragment should be removed"
    );
    assert_eq!(
        fs::read(&unknown).unwrap(),
        b"not owned by Merman",
        "unknown neighboring files must remain untouched"
    );
    let final_check = run(root.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(
        exit_code(&final_check),
        0,
        "stderr: {:?}",
        final_check.stderr
    );
}

#[test]
fn build_refuses_malformed_prior_receipt_without_replacing_known_outputs() {
    let root = tempfile::tempdir().expect("tempdir");
    write_source(root.path(), "diagram.mmd");
    write_config(root.path(), &valid_config("diagram.mmd"));
    let first = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&first), 0, "stderr: {:?}", first.stderr);

    let managed = root.path().join("docs/generated/merman-rustdoc");
    let output = managed.join("architecture.md");
    let original = fs::read(&output).expect("read original fragment");
    fs::write(managed.join("receipt.json"), b"{").expect("corrupt receipt");
    fs::write(
        root.path().join("diagram.mmd"),
        "flowchart TD\nNew-->Graph\n",
    )
    .expect("change source");

    let build = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&build), 3, "stderr: {:?}", build.stderr);
    assert!(
        String::from_utf8_lossy(&build.stderr).contains("malformed JSON"),
        "stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert_eq!(
        fs::read(&output).unwrap(),
        original,
        "a malformed receipt must fail before replacing known outputs"
    );
}

#[test]
fn a_different_configuration_cannot_adopt_or_delete_an_existing_managed_root() {
    let root = tempfile::tempdir().expect("tempdir");
    write_source(root.path(), "one.mmd");
    write_source(root.path(), "two.mmd");
    fs::write(
        root.path().join("config-a.toml"),
        "schema = 1\n[[fragments]]\nid = \"one\"\nsource = \"one.mmd\"\n",
    )
    .expect("write first config");
    fs::write(
        root.path().join("config-b.toml"),
        "schema = 1\n[[fragments]]\nid = \"two\"\nsource = \"two.mmd\"\n",
    )
    .expect("write second config");

    let first = run(
        root.path(),
        &["rustdoc", "build", "--config", "config-a.toml", "--quiet"],
    );
    assert_eq!(exit_code(&first), 0, "stderr: {:?}", first.stderr);

    let managed = root.path().join("docs/generated/merman-rustdoc");
    let one = fs::read(managed.join("one.md")).expect("read first fragment");
    let receipt = fs::read(managed.join("receipt.json")).expect("read receipt");

    let second = run(
        root.path(),
        &["rustdoc", "build", "--config", "config-b.toml", "--quiet"],
    );
    assert_eq!(exit_code(&second), 3, "stderr: {:?}", second.stderr);
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("receipt belongs to Rustdoc configuration"),
        "{stderr}"
    );
    assert_eq!(fs::read(managed.join("one.md")).unwrap(), one);
    assert_eq!(fs::read(managed.join("receipt.json")).unwrap(), receipt);
    assert!(!managed.join("two.md").exists());

    let second_check = run(
        root.path(),
        &["rustdoc", "check", "--config", "config-b.toml", "--quiet"],
    );
    assert_eq!(
        exit_code(&second_check),
        3,
        "stderr: {:?}",
        second_check.stderr
    );

    let check = run(
        root.path(),
        &["rustdoc", "check", "--config", "config-a.toml", "--quiet"],
    );
    assert_eq!(exit_code(&check), 0, "stderr: {:?}", check.stderr);
}

#[test]
fn schema_and_fragment_fields_are_fail_closed_with_stable_locations() {
    let cases = [
        (
            "schema = 2\n[[fragments]]\nid = \"architecture\"\nsource = \"diagram.mmd\"\n",
            vec!["merman-rustdoc.toml", "at schema", "unsupported schema 2"],
        ),
        (
            "schema = 1\nfuture = true\n[[fragments]]\nid = \"architecture\"\nsource = \"diagram.mmd\"\n",
            vec![
                "merman-rustdoc.toml",
                "at line 2, column",
                "unknown field `future`",
            ],
        ),
        (
            "schema = 1\n[[fragments]]\nid = \"architecture\"\nsource = \"diagram.mmd\"\nfuture = true\n",
            vec![
                "merman-rustdoc.toml",
                "at line 5, column",
                "unknown field `future`",
            ],
        ),
        (
            "schema = 1\nfragments = []\n",
            vec![
                "merman-rustdoc.toml",
                "at fragments",
                "at least one fragment",
            ],
        ),
        (
            "schema = 1\n[[fragments]]\nid = \"\"\nsource = \"diagram.mmd\"\n",
            vec![
                "merman-rustdoc.toml",
                "at fragments[0].id",
                "must not be empty",
            ],
        ),
        (
            "schema = 1\n[[fragments]]\nid = \"architecture\"\nsource = \"  \"\n",
            vec![
                "merman-rustdoc.toml",
                "at fragments[0].source",
                "must not be empty",
            ],
        ),
        (
            "schema = 1\n[[fragments]]\nid = \"architecture\"\nsource = \"diagram.mmd\"\nsource_display = \"show\"\n",
            vec![
                "merman-rustdoc.toml",
                "at line 5, column",
                "unknown variant `show`",
            ],
        ),
    ];

    for (config, needles) in cases {
        let root = tempfile::tempdir().expect("tempdir");
        write_source(root.path(), "diagram.mmd");
        assert_config_error(root.path(), config, &needles);
    }
}

#[test]
fn fragment_ids_reject_duplicates_aliases_and_non_portable_names() {
    for (second_id, duplicate_label) in [
        ("architecture", "architecture"),
        ("Architecture", "Architecture"),
        ("e\u{301}", "\\u{301}"),
    ] {
        let root = tempfile::tempdir().expect("tempdir");
        write_source(root.path(), "diagram.mmd");
        let first_id = if second_id == "e\u{301}" {
            "\u{e9}"
        } else {
            "architecture"
        };
        let config = format!(
            "schema = 1\n[[fragments]]\nid = \"{first_id}\"\nsource = \"diagram.mmd\"\n[[fragments]]\nid = \"{second_id}\"\nsource = \"diagram.mmd\"\n"
        );
        assert_config_error(
            root.path(),
            &config,
            &["at fragments[1].id", "aliases fragment", duplicate_label],
        );
    }

    for id in ["-leading", "trailing.", "a/b", "con", "caf\u{e9}"] {
        let root = tempfile::tempdir().expect("tempdir");
        write_source(root.path(), "diagram.mmd");
        let config =
            format!("schema = 1\n[[fragments]]\nid = \"{id}\"\nsource = \"diagram.mmd\"\n");
        assert_config_error(
            root.path(),
            &config,
            &["at fragments[0].id", "portable ASCII identifier"],
        );
    }
}

#[test]
fn source_paths_reject_missing_unsupported_absolute_and_parent_inputs() {
    let missing = tempfile::tempdir().expect("tempdir");
    assert_config_error(
        missing.path(),
        &valid_config("missing.mmd"),
        &["at fragments[0].source", "doesn't exist"],
    );

    let unsupported = tempfile::tempdir().expect("tempdir");
    write_source(unsupported.path(), "diagram.txt");
    assert_config_error(
        unsupported.path(),
        &valid_config("diagram.txt"),
        &["at fragments[0].source", "unsupported extension"],
    );

    let absolute = tempfile::tempdir().expect("tempdir");
    write_source(absolute.path(), "diagram.mmd");
    let absolute_source = absolute.path().join("diagram.mmd");
    assert_config_error(
        absolute.path(),
        &valid_config(
            absolute_source
                .to_str()
                .expect("temporary path should be UTF-8"),
        ),
        &["at fragments[0].source", "must be relative"],
    );

    let parent = tempfile::tempdir().expect("tempdir");
    let config_root = parent.path().join("config");
    fs::create_dir(&config_root).expect("create config root");
    write_source(parent.path(), "outside.mmd");
    assert_config_error(
        &config_root,
        &valid_config("../outside.mmd"),
        &["at fragments[0].source", "parent components"],
    );
}

#[cfg(not(windows))]
#[test]
fn source_and_include_paths_reject_nonportable_names_before_publication() {
    for (source, reason) in [
        ("a:b.mmd", "portable"),
        ("CON.mmd", "reserved Windows device"),
        ("trailing.mmd.", "end with a dot"),
    ] {
        let root = tempfile::tempdir().expect("tempdir");
        write_source(root.path(), source);
        assert_config_error(
            root.path(),
            &valid_config(source),
            &["at fragments[0].source", "source", reason],
        );
        assert!(
            !root.path().join("docs/generated/merman-rustdoc").exists(),
            "invalid source names must fail before publication"
        );
    }

    let include = tempfile::tempdir().expect("tempdir");
    fs::write(
        include.path().join("source.md"),
        "include_mmd!(\"a:b.mmd\")\n",
    )
    .expect("write Markdown source");
    write_source(include.path(), "a:b.mmd");
    write_config(include.path(), &valid_config("source.md"));
    let output = run(include.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&output), 1, "stderr: {:?}", output.stderr);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("portable relative path"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !include
            .path()
            .join("docs/generated/merman-rustdoc")
            .exists(),
        "invalid include names must fail before publication"
    );
}

#[cfg(unix)]
#[test]
fn source_symlink_escapes_are_rejected() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    fs::write(outside.path().join("escaped.mmd"), [0xff]).expect("write invalid outside source");
    symlink(outside.path(), root.path().join("outside-link")).expect("create source symlink");

    assert_config_error(
        root.path(),
        &valid_config("outside-link/escaped.mmd"),
        &["at fragments[0].source", "escapes the configuration root"],
    );
}

#[cfg(unix)]
#[test]
fn source_kind_follows_the_declared_path_instead_of_a_symlink_target_extension() {
    use std::os::unix::fs::symlink;

    let markdown = tempfile::tempdir().expect("tempdir");
    fs::write(markdown.path().join("raw-target.mmd"), VALID_SOURCE).expect("write raw target");
    symlink("raw-target.mmd", markdown.path().join("source.md")).expect("create source symlink");
    write_config(markdown.path(), &valid_config("source.md"));

    let build = run(markdown.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&build), 0, "stderr: {:?}", build.stderr);
    assert_eq!(
        fs::read_to_string(
            markdown
                .path()
                .join("docs/generated/merman-rustdoc/architecture.md")
        )
        .unwrap(),
        VALID_SOURCE,
        "a declared Markdown source must remain Markdown even when its symlink target ends in .mmd"
    );

    let mermaid = tempfile::tempdir().expect("tempdir");
    fs::write(
        mermaid.path().join("markdown-target.md"),
        "```mermaid\nflowchart LR\nA --> B\n```\n",
    )
    .expect("write Markdown target");
    symlink("markdown-target.md", mermaid.path().join("source.mmd"))
        .expect("create source symlink");
    write_config(mermaid.path(), &valid_config("source.mmd"));

    let build = run(mermaid.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(
        exit_code(&build),
        1,
        "a declared Mermaid source must be parsed as raw Mermaid: {:?}",
        build.stderr
    );
    assert!(
        !mermaid
            .path()
            .join("docs/generated/merman-rustdoc")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn managed_output_root_rejects_symlink_components_even_within_config_root() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("tempdir");
    write_source(root.path(), "diagram.mmd");
    write_config(root.path(), &valid_config("diagram.mmd"));
    let sibling = root.path().join("redirected-output");
    fs::create_dir(&sibling).expect("create sibling output directory");
    fs::create_dir_all(root.path().join("docs/generated")).expect("create output ancestors");
    symlink(&sibling, root.path().join("docs/generated/merman-rustdoc"))
        .expect("create managed-root symlink");

    for action in ["build", "check"] {
        let output = run(root.path(), &["rustdoc", action, "--quiet"]);
        assert_eq!(exit_code(&output), 2, "stderr: {:?}", output.stderr);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("symlink component"),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fs::read_dir(&sibling).unwrap().count(), 0);
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn managed_output_root_uses_the_filesystem_canonical_spelling() {
    let root = tempfile::tempdir().expect("tempdir");
    write_source(root.path(), "Docs/rustdoc-src/diagram.mmd");
    write_config(root.path(), &valid_config("docs/rustdoc-src/diagram.mmd"));

    let build = run(root.path(), &["rustdoc", "build", "--quiet"]);
    assert_eq!(exit_code(&build), 0, "stderr: {:?}", build.stderr);
    let fragment = root
        .path()
        .join("Docs/generated/merman-rustdoc/architecture.md");
    assert!(fragment.is_file(), "missing fragment {fragment:?}");

    let check = run(root.path(), &["rustdoc", "check", "--quiet"]);
    assert_eq!(exit_code(&check), 0, "stderr: {:?}", check.stderr);
}

#[test]
fn non_regular_config_is_an_operational_failure() {
    let root = tempfile::tempdir().expect("tempdir");
    fs::create_dir(root.path().join("config-directory")).expect("create config directory");

    let output = run(
        root.path(),
        &[
            "rustdoc",
            "check",
            "--config",
            "config-directory",
            "--quiet",
        ],
    );

    assert_eq!(exit_code(&output), 3, "stderr: {:?}", output.stderr);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not a regular file"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn source_and_managed_output_overlap_is_rejected_by_path_and_identity() {
    let lexical = tempfile::tempdir().expect("tempdir");
    write_source(
        lexical.path(),
        "docs/generated/merman-rustdoc/architecture.md",
    );
    assert_config_error(
        lexical.path(),
        &valid_config("docs/generated/merman-rustdoc/architecture.md"),
        &["managed output root", "source"],
    );

    let identity = tempfile::tempdir().expect("tempdir");
    let output = identity
        .path()
        .join("docs/generated/merman-rustdoc/architecture.md");
    fs::create_dir_all(output.parent().expect("output parent")).expect("create output root");
    fs::write(&output, VALID_SOURCE).expect("write existing managed output");
    let source = identity.path().join("sources/architecture.md");
    fs::create_dir_all(source.parent().expect("source parent")).expect("create source root");
    fs::hard_link(&output, &source).expect("create same-file alias");
    assert_config_error(
        identity.path(),
        &valid_config("sources/architecture.md"),
        &["aliases protected Rustdoc source"],
    );
}

#[test]
fn invalid_utf8_is_rejected_for_config_and_fragment_sources() {
    let invalid_config = tempfile::tempdir().expect("tempdir");
    fs::write(
        invalid_config.path().join("merman-rustdoc.toml"),
        [b's', b'c', b'h', b'e', b'm', b'a', b' ', b'=', b' ', 0xff],
    )
    .expect("write invalid config");
    let output = run(invalid_config.path(), &["rustdoc", "check"]);
    assert_eq!(exit_code(&output), 2, "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("Rustdoc configuration"));
    assert!(stderr.contains("not valid UTF-8"));

    let invalid_source = tempfile::tempdir().expect("tempdir");
    fs::write(invalid_source.path().join("diagram.mmd"), [b'A', 0xff])
        .expect("write invalid source");
    assert_config_error(
        invalid_source.path(),
        &valid_config("diagram.mmd"),
        &["at fragments[0].source", "not valid UTF-8"],
    );
}
