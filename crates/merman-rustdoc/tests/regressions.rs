#![cfg(feature = "svg")]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

mod support;
use support::proc_macro_artifact;

#[test]
fn rustdoc_preserves_markdown_and_scopes_diagrams_to_their_embedding() {
    let temp = TempDir::new();
    let output = rustdoc(
        &temp.0,
        r####"
#[merman_rustdoc::merman]
#[doc = "Introduction.\n\n```mermaid\nflowchart TD\nA-->B\n```\n**After the diagram**"]
pub fn multiline_attribute() {}

#[merman_rustdoc::merman]
/** Introduction.

```mermaid
flowchart TD
A-->B
```
*/
pub fn block_comment() {}

#[merman_rustdoc::merman]
/**
 * Introduction.
 *
 * ```mermaid
 * flowchart TD
 * A-->B
 * ```
 */
pub fn decorated_block_comment() {}

#[merman_rustdoc::merman]
#[doc = "Intro"]
///
///    ```mermaid
///    flowchart TD
///    A-->B
///    ```
pub fn mixed_raw_and_line_comments() {}

#[merman_rustdoc::merman]
#[doc = concat!("Intro", ".")]
///
///     ```mermaid
///     flowchart TD
///     DYNAMIC_EXAMPLE-->B
///     ```
pub fn dynamic_intro_with_example() {}

#[merman_rustdoc::merman]
#[doc = "```mermaid\nflowchart TD\nA-->B\n```"]
#[doc = concat!("**Dynamic", " tail**")]
pub fn diagram_before_dynamic_documentation() {}

#[merman_rustdoc::merman]
#[doc = "```mermaid"]
#[allow(dead_code)]
#[doc = "flowchart TD\nA-->B\n```"]
pub fn interleaved_attributes() {}

#[merman_rustdoc::merman]
/// An example, not a diagram.
///
///     ```mermaid
///     flowchart TD
///     EXAMPLE_ONLY-->B
///     ```
///
/// <!--
/// include_mmd!("missing-comment.mmd")
/// -->
pub fn literal_markdown() {}

#[merman_rustdoc::merman(scope = "tree")]
pub mod conditional {
    #[cfg(any())]
    /// include_mmd!("missing-disabled-function.mmd")
    pub fn disabled_function() {}

    pub struct Fields {
        #[cfg(any())]
        /// include_mmd!("missing-disabled-field.mmd")
        pub disabled: u8,
        /// ```mermaid
        /// flowchart TD
        /// A-->B
        /// ```
        pub enabled: u8,
    }

    pub enum Variants {
        #[cfg(any())]
        /// include_mmd!("missing-disabled-variant.mmd")
        Disabled,
        Enabled {
            #[cfg(any())]
            /// include_mmd!("missing-disabled-variant-field.mmd")
            disabled: u8,
        },
    }
}

#[merman_rustdoc::merman(theme = "dark")]
/// ```mermaid
/// flowchart TD
/// A-->B
/// ```
pub struct RepeatedDiagram;

impl RepeatedDiagram {
    #[merman_rustdoc::merman(theme = "forest")]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn repeated(&self) {}
}

#[merman_rustdoc::merman(background = "#123456", id_prefix = "custom-doc")]
/// ```mermaid
/// flowchart TD
/// A-->B
/// ```
pub fn custom_background() {}

#[merman_rustdoc::merman(source = "details")]
/// ```mermaid
/// flowchart TD
/// %% <script>alert("x")</script> & 'quoted'
/// A-->B
/// ```
pub fn escaped_source() {}

#[merman_rustdoc::merman(scope = "tree", theme = "dark", background = "#112233", source = "details")]
pub mod inherited {
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn parent_options() {}

    #[merman_rustdoc::merman(theme = "forest")]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn local_options() {}

    #[cfg_attr(any(), merman_rustdoc::merman(theme = "forest"))]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn inactive_override() {}

    #[cfg_attr(all(), merman_rustdoc::merman(theme = "forest"))]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn active_override() {}
}
"####,
    );
    assert_success(&output);

    for page in [
        "fn.multiline_attribute.html",
        "fn.block_comment.html",
        "fn.decorated_block_comment.html",
        "fn.mixed_raw_and_line_comments.html",
        "fn.interleaved_attributes.html",
        "conditional/struct.Fields.html",
    ] {
        let html = read_page(&temp.0, page);
        assert_eq!(
            diagram_svgs(&html).len(),
            2,
            "light and dark SVGs in {page}"
        );
        assert_background(&html, "transparent");
    }
    let multiline = read_page(&temp.0, "fn.multiline_attribute.html");
    assert!(multiline.contains("<strong>After the diagram</strong>"));
    let dynamic_tail = read_page(&temp.0, "fn.diagram_before_dynamic_documentation.html");
    assert_eq!(diagram_svgs(&dynamic_tail).len(), 2);
    assert!(dynamic_tail.contains("<strong>Dynamic tail</strong>"));
    let literal = read_page(&temp.0, "fn.literal_markdown.html");
    assert!(diagram_svgs(&literal).is_empty());
    assert!(literal.contains("EXAMPLE_ONLY"));
    assert_eq!(
        preformatted_text(&literal, "EXAMPLE_ONLY").trim(),
        "```mermaid\nflowchart TD\nEXAMPLE_ONLY-->B\n```"
    );
    let dynamic = read_page(&temp.0, "fn.dynamic_intro_with_example.html");
    assert!(diagram_svgs(&dynamic).is_empty());
    assert!(dynamic.contains("Intro."));
    assert_eq!(
        preformatted_text(&dynamic, "DYNAMIC_EXAMPLE").trim(),
        "```mermaid\nflowchart TD\nDYNAMIC_EXAMPLE-->B\n```"
    );

    let repeated = read_page(&temp.0, "struct.RepeatedDiagram.html");
    let svgs = diagram_svgs(&repeated);
    assert_eq!(
        svgs.len(),
        2,
        "both independently attributed diagrams render"
    );
    assert_unique_svg_ids(&svgs);

    let custom = read_page(&temp.0, "fn.custom_background.html");
    assert_eq!(diagram_svgs(&custom).len(), 2);
    assert_background(&custom, "#123456");
    for svg in diagram_svgs(&custom) {
        let document = roxmltree::Document::parse(svg).unwrap();
        assert!(
            document
                .root_element()
                .attribute("id")
                .unwrap()
                .contains("custom-doc"),
            "explicit ID namespace must reach the output"
        );
    }

    let source = read_page(&temp.0, "fn.escaped_source.html");
    let details_start = source
        .find("<details class=\"merman-rustdoc-source\"")
        .unwrap();
    let details = &source[details_start..];
    let details_end = details.find("</details>").unwrap() + "</details>".len();
    let document = roxmltree::Document::parse(&details[..details_end]).unwrap();
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("script"))
    );
    let code = document
        .descendants()
        .find(|node| node.has_tag_name("code"))
        .unwrap();
    assert!(
        code.text()
            .unwrap()
            .contains("%% <script>alert(\"x\")</script> & 'quoted'")
    );

    for page in ["parent_options", "inactive_override"] {
        let html = read_page(&temp.0, &format!("inherited/fn.{page}.html"));
        assert_eq!(diagram_svgs(&html).len(), 1, "parent fixed theme in {page}");
        assert_background(&html, "#112233");
        assert!(html.contains("class=\"merman-rustdoc-source\""));
    }
    for page in ["local_options", "active_override"] {
        let html = read_page(&temp.0, &format!("inherited/fn.{page}.html"));
        assert_eq!(diagram_svgs(&html).len(), 1, "local fixed theme in {page}");
        assert_background(&html, "#112233");
        assert!(html.contains("class=\"merman-rustdoc-source\""));
    }
}

#[test]
fn renamed_dependency_supports_macro_generated_items_with_distinct_ids() {
    let temp = TempDir::new();
    fs::write(
        temp.0.join("Cargo.toml"),
        r#"[package]
name = "merman-regressions"
version = "0.0.0"
edition = "2024"

[dependencies]
diagrams = { package = "merman-rustdoc", version = "*" }
"#,
    )
    .unwrap();
    let output = rustdoc_with_dependency(
        &temp.0,
        r#"
pub struct Generated;

macro_rules! documented_methods {
    ($($name:ident),*) => {
        $(
            #[diagrams::merman]
            /// ```mermaid
            /// flowchart TD
            /// A-->B
            /// ```
            pub fn $name(&self) {}
        )*
    };
}

impl Generated {
    documented_methods!(first, second);
}

pub trait First {
    fn run(&self);
}

pub trait Second {
    fn run(&self);
}

pub struct SeparateInvocations;

macro_rules! implement {
    ($trait:ident) => {
        impl $trait for SeparateInvocations {
            #[diagrams::merman]
            /// ```mermaid
            /// flowchart TD
            /// A-->B
            /// ```
            fn run(&self) {}
        }
    };
}

implement!(First);
implement!(Second);

pub struct TreeGenerated;

macro_rules! implement_together {
    ($($trait:ident),*) => {
        $(
            #[diagrams::merman(scope = "tree")]
            impl $trait for TreeGenerated {
                /// ```mermaid
                /// flowchart TD
                /// A-->B
                /// ```
                fn run(&self) {}
            }
        )*
    };
}

implement_together!(First, Second);

#[diagrams::merman(scope = "tree")]
pub mod nested {
    #[cfg(any())]
    /// include_mmd!("missing-disabled.mmd")
    pub fn disabled() {}

    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn enabled() {}
}
"#,
        "diagrams",
    );
    assert_success(&output);
    let generated = read_page(&temp.0, "struct.Generated.html");
    let svgs = diagram_svgs(&generated);
    assert_eq!(
        svgs.len(),
        4,
        "two macro-generated methods each have light and dark diagrams"
    );
    assert_unique_svg_ids(&svgs);
    for page in [
        "struct.SeparateInvocations.html",
        "struct.TreeGenerated.html",
    ] {
        let html = read_page(&temp.0, page);
        let svgs = diagram_svgs(&html);
        assert_eq!(svgs.len(), 4, "both trait implementations render in {page}");
        assert_unique_svg_ids(&svgs);
    }
    let nested = read_page(&temp.0, "nested/fn.enabled.html");
    assert_eq!(diagram_svgs(&nested).len(), 2);
}

fn preformatted_text(html: &str, marker: &str) -> String {
    let example = html
        .split("<pre")
        .skip(1)
        .find(|block| block.split("</pre>").next().unwrap().contains(marker))
        .expect("indented example remains inside a preformatted block");
    let example = format!(
        "<pre{}",
        &example[..example.find("</pre>").unwrap() + "</pre>".len()]
    );
    let example = roxmltree::Document::parse(&example).unwrap();
    let code = example
        .descendants()
        .find(|node| node.has_tag_name("code"))
        .unwrap();
    code.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|node| node.text())
        .collect::<String>()
}

fn assert_unique_svg_ids(svgs: &[&str]) {
    let mut ids = HashSet::new();
    for svg in svgs {
        let document = roxmltree::Document::parse(svg).unwrap();
        let mut diagram_ids = 0;
        for node in document.descendants().filter(roxmltree::Node::is_element) {
            if let Some(id) = node.attribute("id") {
                diagram_ids += 1;
                assert!(ids.insert(id.to_owned()), "duplicate embedded SVG ID: {id}");
            }
        }
        assert!(diagram_ids > 1, "check internal IDs as well as the root");
    }
}

#[test]
fn strict_mode_rejects_html_resources_and_escaped_css_urls() {
    for (name, source, evidence) in [
        (
            "html_resource",
            "flowchart TD\nA[<img src='https://example.invalid/image.png'>]",
            "https://example.invalid/image.png",
        ),
        (
            "escaped_css",
            r#"flowchart TD
A-->B
style A fill:u\72l(https://example.invalid/image.svg)"#,
            r#"u\72l(https://example.invalid/image.svg)"#,
        ),
    ] {
        let temp = TempDir::new();
        let fixture = |mode| {
            format!(
                "#[merman_rustdoc::merman(sanitize = {mode:?}, theme = \"default\")]\n{doc}\npub fn {name}() {{}}",
                doc = format!("```mermaid\n{source}\n```")
                    .lines()
                    .map(|line| format!("/// {line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        };
        let permissive = rustdoc(&temp.0, &fixture("off"));
        assert_success(&permissive);
        let html = read_page(&temp.0, &format!("fn.{name}.html"));
        let svgs = diagram_svgs(&html);
        assert_eq!(svgs.len(), 1);
        assert!(
            svgs[0].contains(evidence),
            "fixture must reach SVG output: {name}"
        );

        let strict = rustdoc(&temp.0, &fixture("strict"));
        assert!(!strict.status.success(), "strict mode accepted {name}");
        let stderr = String::from_utf8_lossy(&strict.stderr);
        assert!(
            stderr.contains("strict") || stderr.contains("resource") || stderr.contains("sanit"),
            "expected resource validation diagnostic for {name}: {stderr}"
        );
        assert!(
            !stderr.contains("failed to parse"),
            "payload must parse: {stderr}"
        );
    }
}

#[test]
fn cargo_build_script_tracks_external_diagram_changes_and_recovery() {
    let temp = TempDir::new();
    let consumer = temp.0.join("consumer");
    let diagrams = temp.0.join("shared-diagrams");
    fs::create_dir(&consumer).unwrap();
    fs::create_dir(&diagrams).unwrap();
    fs::write(
        consumer.join("Cargo.toml"),
        "[package]\nname = \"tracked-diagrams\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\npath = \"lib.rs\"\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        consumer.join("build.rs"),
        "fn main() { println!(\"cargo::rerun-if-changed=../shared-diagrams\"); }\n",
    )
    .unwrap();
    fs::write(
        consumer.join("lib.rs"),
        r#"
#[cfg_attr(doc, merman_rustdoc::merman(scope = "tree", fail = "keep-source", theme = "default"))]
pub struct Diagram {
    /// include_mmd!("../shared-diagrams/architecture.mmd")
    pub active: u8,
    #[cfg(any())]
    /// include_mmd!("../shared-diagrams/never-created.mmd")
    pub disabled: u8,
}
"#,
    )
    .unwrap();
    let diagram = diagrams.join("architecture.mmd");
    fs::write(&diagram, "flowchart TD\nA[FIRST_REVISION]-->B\n").unwrap();
    let (deps_dir, macro_artifact) = proc_macro_artifact();
    let flags = [
        "--extern".to_string(),
        format!("merman_rustdoc={}", macro_artifact.display()),
        "-L".to_string(),
        format!("dependency={}", deps_dir.display()),
    ]
    .join("\u{1f}");
    let build = || {
        Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["doc", "--offline", "--no-deps", "--quiet"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", temp.0.join("target"))
            .env("CARGO_ENCODED_RUSTDOCFLAGS", &flags)
            .output()
            .unwrap()
    };
    let page = temp
        .0
        .join("target/doc/tracked_diagrams/struct.Diagram.html");
    assert_success(&build());
    let html = fs::read_to_string(&page).unwrap();
    assert!(html.contains("FIRST_REVISION"));
    assert_eq!(diagram_svgs(&html).len(), 1);
    let unchanged = fs::metadata(&page).unwrap().modified().unwrap();
    assert_success(&build());
    assert_eq!(fs::metadata(&page).unwrap().modified().unwrap(), unchanged);

    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(&diagram, "flowchart TD\nA[SECOND_REVISION]-->B\n").unwrap();
    assert_success(&build());
    let html = fs::read_to_string(&page).unwrap();
    assert!(html.contains("SECOND_REVISION"));
    assert!(!html.contains("FIRST_REVISION"));

    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::remove_file(&diagram).unwrap();
    assert_success(&build());
    let html = fs::read_to_string(&page).unwrap();
    assert!(diagram_svgs(&html).is_empty());
    assert!(html.contains("include_mmd!"));

    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(&diagram, "flowchart TD\nA[RESTORED_REVISION]-->B\n").unwrap();
    assert_success(&build());
    let html = fs::read_to_string(&page).unwrap();
    assert!(html.contains("RESTORED_REVISION"));
    assert_eq!(diagram_svgs(&html).len(), 1);
}

#[test]
fn rustdoc_renders_diagrams_inside_markdown_containers() {
    let temp = TempDir::new();
    fs::write(
        temp.0.join("Cargo.toml"),
        "[package]\nname = \"merman-regressions\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(temp.0.join("diagram.mmd"), "flowchart TD\nA-->B\n").unwrap();
    let output = rustdoc(
        &temp.0,
        r#"
#[merman_rustdoc::merman(theme = "default")]
/// > Quoted diagram.
/// >
/// > ```mermaid
/// > flowchart TD
/// > A-->B
/// > ```
/// >
/// > **Quoted tail**
///
/// **Outside quote**
pub fn quoted() {}

#[merman_rustdoc::merman(theme = "default")]
/// 1. Diagram item.
///
///    include_mmd!("diagram.mmd")
///
///    **Item tail**
///
/// 2. **Next item**
pub fn listed() {}

#[merman_rustdoc::merman(theme = "default")]
/// Reference[^diagram].
///
/// [^diagram]:
///     ```mermaid
///     flowchart TD
///     A-->B
///     ```
///
///     **Footnote tail**
pub fn footnoted() {}
"#,
    );
    assert_success(&output);
    let quoted = read_page(&temp.0, "fn.quoted.html");
    let quote_start = quoted.find("<blockquote>").unwrap();
    let quote_end = quoted[quote_start..].find("</blockquote>").unwrap() + quote_start;
    assert_eq!(diagram_svgs(&quoted[quote_start..quote_end]).len(), 1);
    assert!(quoted[quote_start..quote_end].contains("<strong>Quoted tail</strong>"));
    assert!(quoted[quote_end..].contains("<strong>Outside quote</strong>"));
    let listed = read_page(&temp.0, "fn.listed.html");
    let item_start = listed.find("Diagram item.").unwrap();
    let item_end = listed[item_start..].find("</li>").unwrap() + item_start;
    assert_eq!(diagram_svgs(&listed[item_start..item_end]).len(), 1);
    assert!(listed[item_start..item_end].contains("<strong>Item tail</strong>"));
    assert!(listed[item_end..].contains("<strong>Next item</strong>"));
    let footnoted = read_page(&temp.0, "fn.footnoted.html");
    let note_start = footnoted.find("id=\"fn1\"").unwrap();
    let note_end = footnoted[note_start..].find("</li>").unwrap() + note_start;
    assert_eq!(diagram_svgs(&footnoted[note_start..note_end]).len(), 1);
    assert!(footnoted[note_start..note_end].contains("<strong>Footnote tail</strong>"));
}

#[test]
fn rustdoc_math_requires_explicit_feature() {
    let temp = TempDir::new();
    let output = rustdoc(
        &temp.0,
        r####"
#[merman_rustdoc::merman(theme = "default")]
/// ```mermaid
/// flowchart TD
/// A["$$x^2$$"] --> B
/// ```
pub fn math_diagram() {}
"####,
    );

    #[cfg(feature = "math")]
    {
        assert_success(&output);
        let html = read_page(&temp.0, "fn.math_diagram.html");
        assert!(html.contains(r#"class="merman-rustdoc-diagram""#));
        assert!(html.contains("<svg"));
        assert!(!html.contains("$$x^2$$"));
    }

    #[cfg(not(feature = "math"))]
    {
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("lacks capability `math`"), "{stderr}");
        assert!(
            stderr.contains(
                "enable the `math` or `complete-svg` feature on the `merman-rustdoc` dependency"
            ),
            "{stderr}"
        );
    }
}

fn read_page(temp: &Path, relative: &str) -> String {
    let path = temp.join("doc/merman_regressions").join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn diagram_svgs(html: &str) -> Vec<&str> {
    let mut remaining = html;
    let mut result = Vec::new();
    while let Some(start) = remaining.find("<svg") {
        remaining = &remaining[start..];
        let end = remaining.find("</svg>").expect("closed inline SVG") + "</svg>".len();
        let svg = &remaining[..end];
        let document = roxmltree::Document::parse(svg).expect("inline SVG is valid XML");
        if document
            .root_element()
            .attribute("aria-roledescription")
            .is_some()
        {
            result.push(svg);
        }
        remaining = &remaining[end..];
    }
    result
}

fn assert_background(html: &str, expected: &str) {
    let svgs = diagram_svgs(html);
    assert!(
        !svgs.is_empty(),
        "background assertion needs a rendered diagram"
    );
    for svg in svgs {
        let document = roxmltree::Document::parse(svg).unwrap();
        let style = document
            .root_element()
            .attribute("style")
            .unwrap_or_default();
        let background = style
            .split(';')
            .filter_map(|rule| rule.split_once(':'))
            .find_map(|(name, value)| (name.trim() == "background-color").then_some(value.trim()));
        assert_eq!(background, Some(expected), "root style: {style}");
    }
}

fn rustdoc(temp: &Path, source: &str) -> Output {
    rustdoc_with_dependency(temp, source, "merman_rustdoc")
}

fn rustdoc_with_dependency(temp: &Path, source: &str, dependency: &str) -> Output {
    let input = temp.join("lib.rs");
    fs::write(&input, source).unwrap();
    let (deps_dir, macro_artifact) = proc_macro_artifact();
    let mut command = Command::new(std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into()));
    if temp.join("Cargo.toml").exists() {
        command
            .env("CARGO_MANIFEST_DIR", temp)
            .env("CARGO_PKG_NAME", "merman-regressions");
    }
    command
        .arg("--edition=2024")
        .args(["--crate-name", "merman_regressions"])
        .arg("--extern")
        .arg(format!("{dependency}={}", macro_artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps_dir.display()))
        .arg("-o")
        .arg(temp.join("doc"))
        .arg(input)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "rustdoc failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "merman-rustdoc-regressions-{}-{now}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
