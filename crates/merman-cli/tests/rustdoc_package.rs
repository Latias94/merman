use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CONSUMER_NAME: &str = "rustdoc-generated-consumer";
const CONSUMER_VERSION: &str = "0.0.0";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

fn cargo() -> PathBuf {
    PathBuf::from(env!("CARGO"))
        .canonicalize()
        .expect("canonical Cargo executable")
}

fn cli() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("merman-cli").to_path_buf()
}

fn run(command: &mut Command) -> Output {
    command.output().expect("run command")
}

fn assert_success(output: &Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn snapshot_tree(root: &Path, ignored_root: Option<&Path>) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(
        root: &Path,
        current: &Path,
        ignored_root: Option<&Path>,
        snapshot: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) {
        if ignored_root.is_some_and(|ignored| current == ignored) {
            return;
        }
        let mut entries = fs::read_dir(current)
            .expect("read snapshot directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read snapshot entries");
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().expect("read snapshot file type");
            if file_type.is_dir() {
                visit(root, &path, ignored_root, snapshot);
            } else if file_type.is_file() {
                snapshot.insert(
                    path.strip_prefix(root)
                        .expect("snapshot path below root")
                        .to_path_buf(),
                    fs::read(path).expect("read snapshot file"),
                );
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    visit(root, root, ignored_root, &mut snapshot);
    snapshot
}

fn write_consumer(root: &Path) {
    fs::create_dir_all(root.join("src")).expect("create consumer source directory");
    fs::create_dir_all(root.join("docs")).expect("create consumer docs directory");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{CONSUMER_NAME}"
version = "{CONSUMER_VERSION}"
edition = "2024"
publish = false
include = ["Cargo.toml", "Cargo.lock", "src/**", "docs/**"]

[workspace]
"#,
        ),
    )
    .expect("write consumer manifest");
    fs::write(
        root.join("src/lib.rs"),
        r#"#![doc = include_str!("../docs/crate-overview.md")]

#[doc = include_str!("../docs/render-module.md")]
pub mod render {
    /// A fixture API documented with a generated module fragment.
    pub struct Renderer;
}
"#,
    )
    .expect("write consumer library");

    let generated = workspace_root().join("crates/merman/docs/generated/merman-rustdoc");
    for file in ["crate-overview.md", "render-module.md", "receipt.json"] {
        fs::copy(generated.join(file), root.join("docs").join(file))
            .unwrap_or_else(|error| panic!("copy generated {file}: {error}"));
    }
}

fn extract_package(archive: &Path, destination: &Path) -> PathBuf {
    fs::create_dir_all(destination).expect("create package extraction directory");
    let output = run(Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(destination));
    assert_success(&output, "extract fixture package");
    destination.join(format!("{CONSUMER_NAME}-{CONSUMER_VERSION}"))
}

fn set_tree_read_only(root: &Path, read_only: bool) {
    fn visit(path: &Path, read_only: bool) {
        if !read_only {
            set_path_read_only(path, false);
        }
        if path.is_dir() {
            let entries = fs::read_dir(path)
                .expect("read permission tree")
                .collect::<Result<Vec<_>, _>>()
                .expect("read permission entries");
            for entry in entries {
                visit(&entry.path(), read_only);
            }
        }
        if read_only {
            set_path_read_only(path, true);
        }
    }

    fn set_path_read_only(path: &Path, read_only: bool) {
        let mut permissions = fs::metadata(path).expect("read permissions").permissions();
        permissions.set_readonly(read_only);
        fs::set_permissions(path, permissions).expect("set permissions");
    }

    visit(root, read_only);
}

fn cargo_without_cli() -> (PathBuf, OsString) {
    let cargo = cargo();
    let toolchain_bin = cargo.parent().expect("Cargo executable parent");
    let isolated_path = toolchain_bin.as_os_str().to_os_string();
    let cli_name = format!("merman-cli{}", std::env::consts::EXE_SUFFIX);
    assert!(
        !toolchain_bin.join(cli_name).exists(),
        "the isolated PATH must not contain merman-cli"
    );
    (cargo, isolated_path)
}

#[test]
fn build_changes_only_the_managed_output_root() {
    let project = tempfile::tempdir().expect("temporary project");
    let root = project.path();
    fs::create_dir_all(root.join("docs/diagrams")).expect("create source directories");
    fs::write(
        root.join("docs/guide.md"),
        "# Guide\n\ninclude_mmd!(\"docs/diagrams/flow.mmd\")\n",
    )
    .expect("write Markdown source");
    fs::write(
        root.join("docs/diagrams/flow.mmd"),
        "flowchart LR\nSource --> Output\n",
    )
    .expect("write Mermaid include");
    fs::write(
        root.join("merman-rustdoc.toml"),
        "schema = 1\n\n[[fragments]]\nid = \"guide\"\nsource = \"docs/guide.md\"\n",
    )
    .expect("write Rustdoc config");

    let managed = root.join("docs/generated/merman-rustdoc");
    let before = snapshot_tree(root, Some(&managed));
    let output = run(Command::new(cli())
        .current_dir(root)
        .args(["rustdoc", "build", "--quiet"]));
    assert_success(&output, "build Rustdoc fragments");
    assert_eq!(
        snapshot_tree(root, Some(&managed)),
        before,
        "Rustdoc build must not modify declared inputs or neighboring files"
    );
    assert!(managed.join("guide.md").is_file());
    assert!(managed.join("receipt.json").is_file());
}

#[test]
fn packaged_fragments_document_offline_without_a_cli_dependency() {
    let workspace = workspace_root();
    let package_list = run(Command::new(cargo()).current_dir(&workspace).args([
        "package",
        "-p",
        "merman",
        "--list",
        "--allow-dirty",
    ]));
    assert_success(&package_list, "list merman package contents");
    let package_list = String::from_utf8(package_list.stdout).expect("package list is UTF-8");
    for required in [
        "merman-rustdoc.toml",
        "docs/rustdoc-src/crate-overview.md",
        "docs/rustdoc-src/render-module.md",
        "docs/generated/merman-rustdoc/crate-overview.md",
        "docs/generated/merman-rustdoc/render-module.md",
        "docs/generated/merman-rustdoc/receipt.json",
    ] {
        assert!(
            package_list.lines().any(|line| line == required),
            "published merman package is missing {required}:\n{package_list}"
        );
    }

    let fixture = tempfile::tempdir().expect("temporary fixture");
    let source = fixture.path().join("source");
    write_consumer(&source);
    let lock = run(Command::new(cargo())
        .current_dir(&source)
        .args(["generate-lockfile", "--offline"]));
    assert_success(&lock, "generate fixture lockfile");

    let package_target = fixture.path().join("package-target");
    let package = run(Command::new(cargo())
        .current_dir(&source)
        .env("CARGO_TARGET_DIR", &package_target)
        .args(["package", "--allow-dirty", "--no-verify"]));
    assert_success(&package, "package zero-dependency fixture");
    let archive = package_target.join(format!("package/{CONSUMER_NAME}-{CONSUMER_VERSION}.crate"));
    assert!(archive.is_file(), "missing fixture archive {archive:?}");
    let unpacked = extract_package(&archive, &fixture.path().join("unpacked"));

    let (cargo, isolated_path) = cargo_without_cli();
    let doc_target = fixture.path().join("doc-target");
    set_tree_read_only(&unpacked, true);
    let tree = run(Command::new(&cargo)
        .current_dir(&unpacked)
        .env("PATH", &isolated_path)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", fixture.path().join("tree-target"))
        .args(["tree", "--locked", "--offline", "--edges", "normal,build"]));
    let docs = run(Command::new(&cargo)
        .current_dir(&unpacked)
        .env("PATH", &isolated_path)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", &doc_target)
        .args(["doc", "--locked", "--offline", "--no-deps"]));
    set_tree_read_only(&unpacked, false);
    assert_success(&tree, "inspect consumer dependency closure");
    assert_success(&docs, "document unpacked read-only fixture offline");

    let tree = String::from_utf8(tree.stdout).expect("cargo tree is UTF-8");
    let packages = tree.lines().filter(|line| !line.trim().is_empty()).count();
    assert_eq!(packages, 1, "consumer must have no dependencies:\n{tree}");
    for forbidden in [
        "merman ",
        "merman-rustdoc",
        "merman-render",
        "merman-layout",
        "ratex",
        "syn ",
        "quote ",
        "proc-macro2",
    ] {
        assert!(
            !tree.contains(forbidden),
            "consumer closure unexpectedly contains {forbidden:?}:\n{tree}"
        );
    }

    for html in [
        doc_target.join("doc/rustdoc_generated_consumer/index.html"),
        doc_target.join("doc/rustdoc_generated_consumer/render/index.html"),
    ] {
        let html = fs::read_to_string(&html)
            .unwrap_or_else(|error| panic!("read generated Rustdoc HTML {html:?}: {error}"));
        assert!(html.contains("data-merman-rustdoc=\"true\""), "{html}");
        assert!(html.contains("<svg"), "{html}");
    }

    fs::remove_file(unpacked.join("docs/render-module.md"))
        .expect("remove generated fragment from fixture");
    let missing = run(Command::new(&cargo)
        .current_dir(&unpacked)
        .env("PATH", &isolated_path)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", fixture.path().join("missing-target"))
        .args(["doc", "--locked", "--offline", "--no-deps"]));
    assert!(
        !missing.status.success(),
        "Rustdoc must fail when a committed generated fragment is missing"
    );
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        stderr.contains("render-module.md") && stderr.contains("include_str"),
        "unexpected missing-fragment diagnostic:\n{stderr}"
    );
}
