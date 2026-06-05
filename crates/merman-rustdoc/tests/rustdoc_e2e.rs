use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn rustdoc_outputs_inline_svg_for_mermaid_fence() {
    let temp = unique_temp_dir();
    let source = temp.join("lib.rs");
    let out_dir = temp.join("doc");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &source,
        r####"
#[merman_rustdoc::merman(source = "details")]
/// Rendered by rustdoc.
///
/// ```mermaid
/// flowchart TD
///   A[Start] --> B[Done]
/// ```
pub fn documented_fence() {}

#[merman_rustdoc::merman]
/// Rendered from an external Mermaid file.
///
/// include_mmd!("tests/fixtures/simple.mmd")
pub fn documented_include() {}

#[merman_rustdoc::merman(fail = "keep-source")]
/// Preserved when an include cannot be read.
///
/// include_mmd!("tests/fixtures/missing.mmd")
pub fn tolerant_missing_include() {}

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   M[Module] --> D[Docs]
/// ```
pub mod documented_module {}

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   S[Struct] --> D[Docs]
/// ```
pub struct DocumentedStruct;

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   T[Trait] --> D[Docs]
/// ```
pub trait DocumentedTrait {
    fn run(&self);
}

pub struct ImplTarget;

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   I[Impl] --> D[Docs]
/// ```
impl ImplTarget {
    pub fn method(&self) {}
}

#[merman_rustdoc::merman]
/// Diagram with a footnote reference.[^render-note]
///
/// ```mermaid
/// flowchart TD
///   F[Footnote] --> D[Diagram]
/// ```
///
/// [^render-note]: Footnote content should still render.
pub fn documented_footnote() {}

#[merman_rustdoc::merman(scope = "tree")]
pub mod tree_scope {
    /// Nested function diagram.
    ///
    /// ```mermaid
    /// flowchart TD
    ///   NestedFunction --> Docs
    /// ```
    pub fn nested_function() {}

    pub struct NestedStruct {
        /// Nested field diagram.
        ///
        /// ```mermaid
        /// flowchart TD
        ///   Field --> Docs
        /// ```
        pub field: u8,
    }

    pub trait NestedTrait {
        /// Nested trait method diagram.
        ///
        /// ```mermaid
        /// flowchart TD
        ///   TraitMethod --> Docs
        /// ```
        fn nested_trait_method(&self);
    }

    pub struct NestedImpl;

    impl NestedImpl {
        /// Nested impl method diagram.
        ///
        /// ```mermaid
        /// flowchart TD
        ///   ImplMethod --> Docs
        /// ```
        pub fn nested_method(&self) {}
    }
}
"####,
    )
    .unwrap();

    let (deps_dir, macro_artifact) = proc_macro_artifact();
    let rustdoc = std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into());
    let output = Command::new(rustdoc)
        .arg("--edition=2024")
        .arg("--crate-name")
        .arg("merman_rustdoc_e2e")
        .arg("--extern")
        .arg(format!("merman_rustdoc={}", macro_artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps_dir.display()))
        .arg("-o")
        .arg(&out_dir)
        .arg(&source)
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "rustdoc e2e failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    assert_doc_contains_svg(&out_dir, "merman_rustdoc_e2e/fn.documented_fence.html");
    assert_doc_contains_svg(&out_dir, "merman_rustdoc_e2e/fn.documented_include.html");
    assert_doc_contains_svg(&out_dir, "merman_rustdoc_e2e/documented_module/index.html");
    assert_doc_contains_svg(&out_dir, "merman_rustdoc_e2e/struct.DocumentedStruct.html");
    assert_doc_contains_svg(&out_dir, "merman_rustdoc_e2e/trait.DocumentedTrait.html");
    assert_doc_contains_svg(&out_dir, "merman_rustdoc_e2e/struct.ImplTarget.html");
    assert_doc_contains_svg(&out_dir, "merman_rustdoc_e2e/fn.documented_footnote.html");
    assert_doc_contains_svg(
        &out_dir,
        "merman_rustdoc_e2e/tree_scope/fn.nested_function.html",
    );
    assert_doc_contains_svg(
        &out_dir,
        "merman_rustdoc_e2e/tree_scope/struct.NestedStruct.html",
    );
    assert_doc_contains_svg(
        &out_dir,
        "merman_rustdoc_e2e/tree_scope/trait.NestedTrait.html",
    );
    assert_doc_contains_svg(
        &out_dir,
        "merman_rustdoc_e2e/tree_scope/struct.NestedImpl.html",
    );

    let function_html =
        fs::read_to_string(out_dir.join("merman_rustdoc_e2e/fn.documented_fence.html")).unwrap();
    assert!(function_html.contains(r#"class="merman-rustdoc-source""#));
    assert!(function_html.contains("language-mermaid"));

    let footnote_html =
        fs::read_to_string(out_dir.join("merman_rustdoc_e2e/fn.documented_footnote.html")).unwrap();
    assert!(footnote_html.contains(r#"<sup id="fnref"#));
    assert!(footnote_html.contains(r#"class="footnotes""#));
    assert!(footnote_html.contains("Footnote content should still render."));

    let tolerant_html =
        fs::read_to_string(out_dir.join("merman_rustdoc_e2e/fn.tolerant_missing_include.html"))
            .unwrap();
    assert!(tolerant_html.contains("missing.mmd"));
    assert!(!tolerant_html.contains(r#"class="merman-rustdoc-diagram""#));

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn rustdoc_reexports_preserve_upstream_inline_svg() {
    let temp = unique_temp_dir();
    fs::create_dir_all(&temp).unwrap();

    let upstream_source = temp.join("upstream.rs");
    let upstream_rlib = temp.join("libupstream_docs.rlib");
    fs::write(
        &upstream_source,
        r####"
#[merman_rustdoc::merman]
/// Re-exported diagram.
///
/// ```mermaid
/// flowchart TD
///   U[Upstream] --> R[Re-export]
/// ```
pub struct ReexportedDiagram;
"####,
    )
    .unwrap();

    let (deps_dir, macro_artifact) = proc_macro_artifact();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("--edition=2024")
        .arg("--crate-type=lib")
        .arg("--crate-name")
        .arg("upstream_docs")
        .arg("--extern")
        .arg(format!("merman_rustdoc={}", macro_artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps_dir.display()))
        .arg("-o")
        .arg(&upstream_rlib)
        .arg(&upstream_source)
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "upstream rustc failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let downstream_source = temp.join("downstream.rs");
    let out_dir = temp.join("doc");
    fs::write(
        &downstream_source,
        r#"
#[doc(inline)]
pub use upstream_docs::ReexportedDiagram;
"#,
    )
    .unwrap();

    let rustdoc = std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into());
    let output = Command::new(rustdoc)
        .arg("--edition=2024")
        .arg("--crate-name")
        .arg("downstream_docs")
        .arg("--extern")
        .arg(format!("upstream_docs={}", upstream_rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps_dir.display()))
        .arg("-o")
        .arg(&out_dir)
        .arg(&downstream_source)
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "downstream rustdoc failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    assert_doc_contains_svg(&out_dir, "downstream_docs/struct.ReexportedDiagram.html");

    let _ = fs::remove_dir_all(temp);
}

fn assert_doc_contains_svg(out_dir: &Path, relative: &str) {
    let html = fs::read_to_string(out_dir.join(relative)).unwrap_or_else(|err| {
        panic!("failed to read rustdoc HTML `{relative}`: {err}");
    });
    assert!(
        html.contains(r#"class="merman-rustdoc-diagram""#),
        "expected merman-rustdoc wrapper in {relative}"
    );
    assert!(html.contains("<svg"), "expected inline SVG in {relative}");
}

fn proc_macro_artifact() -> (PathBuf, PathBuf) {
    let deps_dir = target_dir().join("debug/deps");
    let extension = std::env::consts::DLL_EXTENSION;
    let artifact = fs::read_dir(&deps_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| is_proc_macro_artifact(path, extension))
        .max_by_key(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH)
        })
        .unwrap_or_else(|| {
            panic!(
                "failed to find compiled merman_rustdoc proc-macro artifact in {}",
                deps_dir.display()
            )
        });
    (deps_dir, artifact)
}

fn is_proc_macro_artifact(path: &Path, extension: &str) -> bool {
    let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    path.extension().and_then(OsStr::to_str) == Some(extension)
        && file_name.contains("merman_rustdoc")
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn unique_temp_dir() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("merman-rustdoc-e2e-{}-{now}", std::process::id()))
}
