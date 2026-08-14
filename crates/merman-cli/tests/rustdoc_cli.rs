use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    format!(
        "schema = 1\n\n[[fragments]]\nid = \"architecture\"\nsource = \"{source}\"\nsource_display = \"details\"\n"
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
fn default_and_explicit_configs_reach_an_explicit_unimplemented_dispatch() {
    let default_root = tempfile::tempdir().expect("tempdir");
    write_source(default_root.path(), "docs/architecture.md");
    write_config(default_root.path(), &valid_config("docs/architecture.md"));

    for command in ["build", "check"] {
        let output = run(default_root.path(), &["rustdoc", command, "--quiet"]);
        assert_eq!(exit_code(&output), 3, "stderr: {:?}", output.stderr);
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
        assert!(
            stderr.contains(&format!("rustdoc {command} execution is not implemented")),
            "valid {command} must fail explicitly until the renderer lands:\n{stderr}"
        );
    }
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
    assert_eq!(exit_code(&output), 3, "stderr: {:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("rustdoc check execution is not implemented"));
    assert!(stderr.contains("custom.toml"));
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

#[cfg(unix)]
#[test]
fn source_symlink_escapes_are_rejected() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    write_source(outside.path(), "escaped.mmd");
    symlink(outside.path(), root.path().join("outside-link")).expect("create source symlink");

    assert_config_error(
        root.path(),
        &valid_config("outside-link/escaped.mmd"),
        &["at fragments[0].source", "escapes the configuration root"],
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
