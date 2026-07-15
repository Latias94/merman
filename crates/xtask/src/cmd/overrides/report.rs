//! Inventory and reporting for parity overrides.

use crate::XtaskError;
use merman_core::baseline::{
    LEGACY_GENERATED_BASELINE_SUFFIX, PINNED_MERMAID_BASELINE_TAG, PINNED_MERMAID_BASELINE_VERSION,
};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use syn::visit::Visit;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OverrideCategory {
    RootViewport,
    TextLookup,
    SvgTextMetrics,
    FontMetrics,
    HandCuratedHelpers,
    RawPathBridge,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct OverrideCategoryMetadata {
    owner: &'static str,
    source: &'static str,
    allowed_use: &'static str,
    expected_removal: &'static str,
}

impl OverrideCategory {
    const ALL: [OverrideCategory; 6] = [
        OverrideCategory::RootViewport,
        OverrideCategory::TextLookup,
        OverrideCategory::SvgTextMetrics,
        OverrideCategory::FontMetrics,
        OverrideCategory::HandCuratedHelpers,
        OverrideCategory::RawPathBridge,
    ];

    fn heading(self) -> &'static str {
        match self {
            OverrideCategory::RootViewport => "Root viewport overrides",
            OverrideCategory::TextLookup => "Text metric lookup overrides",
            OverrideCategory::SvgTextMetrics => "SVG text metric tables",
            OverrideCategory::FontMetrics => "Font metric tables",
            OverrideCategory::HandCuratedHelpers => "Hand-curated helper overrides",
            OverrideCategory::RawPathBridge => "Manual raw SVG/path bridges",
        }
    }

    fn total_unit(self) -> &'static str {
        match self {
            OverrideCategory::RootViewport => "entries",
            OverrideCategory::TextLookup => "lookup entries",
            OverrideCategory::SvgTextMetrics => "table rows",
            OverrideCategory::FontMetrics => "table rows",
            OverrideCategory::HandCuratedHelpers => "helper functions",
            OverrideCategory::RawPathBridge => "bridge functions",
        }
    }

    fn no_growth_budget(self) -> usize {
        match self {
            OverrideCategory::RootViewport => 192,
            OverrideCategory::TextLookup => 689,
            OverrideCategory::SvgTextMetrics => 1039,
            OverrideCategory::FontMetrics => 3774,
            OverrideCategory::HandCuratedHelpers => 0,
            OverrideCategory::RawPathBridge => 0,
        }
    }

    fn metadata(self) -> OverrideCategoryMetadata {
        match self {
            OverrideCategory::RootViewport => OverrideCategoryMetadata {
                owner: "render parity workstream",
                source: "fixture-derived upstream SVG root viewBox/max-width baselines for the pinned Mermaid baseline",
                allowed_use: "narrow export-bound pins when browser insertion or emitted bounds differ from deterministic Rust layout",
                expected_removal: "delete entries once typed layout/emitted bounds can derive the same root viewport or a baseline upgrade removes the pinned behavior",
            },
            OverrideCategory::TextLookup => OverrideCategoryMetadata {
                owner: "render parity workstream",
                source: "fixture or browser-probe HTML/SVG text measurements for exact diagram text contexts",
                allowed_use: "exact diagram/text/font-size lookups for browser/font measurement facts that shared metrics cannot derive yet",
                expected_removal: "delete entries once vendored/shared text measurement returns the upstream dimensions without fixture-specific lookup arms",
            },
            OverrideCategory::SvgTextMetrics => OverrideCategoryMetadata {
                owner: "render parity workstream",
                source: "browser getBBox/getComputedTextLength measurements extracted from upstream SVG text nodes",
                allowed_use: "font-keyed SVG text overhang and scale correction for Mermaid baseline parity",
                expected_removal: "replace with shared font metrics or browser-probe imports, then delete stale rows",
            },
            OverrideCategory::FontMetrics => OverrideCategoryMetadata {
                owner: "shared text measurement owner",
                source: "browser-measured glyph, kerning, trigram, HTML, and SVG correction tables",
                allowed_use: "deterministic text measurement support when runtime browser measurement is unavailable",
                expected_removal: "regenerate or trim when better vendored font/probe data covers the drift; remove only if a real measurement backend becomes the default",
            },
            OverrideCategory::HandCuratedHelpers => OverrideCategoryMetadata {
                owner: "diagram renderer owner",
                source: "small hand-curated constants for known Mermaid browser/layout quirks",
                allowed_use: "narrow constants that are stable, tested, and cheaper than broad generated tables",
                expected_removal: "replace with repeatable generated data or typed model/layout computations as soon as a reliable source exists",
            },
            OverrideCategory::RawPathBridge => OverrideCategoryMetadata {
                owner: "diagram-specific svg/parity module owner",
                source: "hand-authored maybe_override_* functions under svg/parity",
                allowed_use: "temporary exact raw SVG/path bridges for literal upstream behavior that the generic emitter cannot reproduce yet",
                expected_removal: "delete once typed layout/path emission reproduces the upstream literal behavior; keep local owner/removal notes beside each bridge",
            },
        }
    }
}

#[derive(Debug, Clone)]
struct OverrideFootprintEntry {
    file_name: String,
    category: OverrideCategory,
    count: usize,
    unit: &'static str,
}

pub(crate) fn report_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    let mut check_no_growth = false;

    for arg in args {
        match arg.as_str() {
            "--check-no-growth" => check_no_growth = true,
            "--help" | "-h" => {
                println!("usage: xtask report-overrides [--check-no-growth]");
                println!();
                println!("Prints a parity override footprint inventory.");
                println!("This is intended for CI logs and drift reviews.");
                println!();
                println!("Options:");
                println!(
                    "  --check-no-growth  fail if any category grows beyond the explicit budget or root viewport ownership bypasses the typed router"
                );
                return Ok(());
            }
            _ => return Err(XtaskError::Usage),
        }
    }

    if check_no_growth {
        println!("Override growth budget: enabled");
        println!();
    }

    let workspace_root = crate::cmd::workspace_root();

    let generated_dir = workspace_root
        .join("crates")
        .join("merman-render")
        .join("src")
        .join("generated");
    let parity_dir = workspace_root
        .join("crates")
        .join("merman-render")
        .join("src")
        .join("svg")
        .join("parity");
    let source_root = workspace_root
        .join("crates")
        .join("merman-render")
        .join("src");

    let generated_entries = collect_generated_override_footprint_entries(&generated_dir)?;
    let manual_entries = collect_manual_bridge_footprint_entries(&parity_dir, &source_root)?;

    println!(
        "Mermaid baseline: {}",
        pinned_mermaid_baseline_label(&workspace_root)
    );
    println!();
    println!(
        "Generated override modules scanned: {}",
        generated_entries.len()
    );
    println!(
        "Manual raw SVG/path bridge files scanned: {}",
        manual_entries.len()
    );
    println!();

    let mut entries = generated_entries;
    entries.extend(manual_entries);

    for category in OverrideCategory::ALL {
        print_category(&entries, category);
    }

    if check_no_growth {
        check_override_no_growth(&entries)?;
        println!("Override growth check: ok");
        check_root_viewport_architecture(&generated_dir, &parity_dir, &source_root)?;
        println!("Root viewport ownership check: ok");
        println!();
    }

    println!("Notes:");
    println!("- Counts are inventory units and are not directly comparable across categories.");
    println!(
        "- Generated module counts cover `crates/merman-render/src/generated`, while manual bridge counts cover hand-authored path-bridge helpers under `crates/merman-render/src/svg/parity`."
    );
    println!("- Root viewport entries count match arms returning `Some((viewBox, max_width))`.");
    println!(
        "- Root viewport tables are private implementation details reached only through the typed `RenderFamilyKind` router."
    );
    println!(
        "- Text lookup entries count generated or hand-curated `=> Some(...)` parity branches and rows in generated lookup tables."
    );
    println!("- Table rows count tuple rows in generated font/SVG metric arrays.");

    Ok(())
}

fn check_override_no_growth(entries: &[OverrideFootprintEntry]) -> Result<(), XtaskError> {
    let mut failures = Vec::new();
    for category in OverrideCategory::ALL {
        let total = category_total(entries, category);
        let budget = category.no_growth_budget();
        if total > budget {
            failures.push(format!(
                "{} grew to {total} {}, budget {budget}",
                category.heading(),
                category.total_unit()
            ));
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::VerifyFailed(format!(
        "override footprint grew beyond the explicit no-growth budget:\n{}",
        failures.join("\n")
    )))
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RootViewportArchitectureViolation {
    file_name: String,
    line_number: usize,
    line: String,
    message: String,
}

#[derive(Debug, Clone, Copy)]
struct RootViewportTableRoute {
    family_variant: &'static str,
    module: &'static str,
    lookup: &'static str,
}

const ROOT_VIEWPORT_TABLE_ROUTES: [RootViewportTableRoute; 10] = [
    RootViewportTableRoute {
        family_variant: "C4",
        module: "c4_root_overrides_11_12_2",
        lookup: "lookup_c4_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "Er",
        module: "er_root_overrides_11_12_2",
        lookup: "lookup_er_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "EventModeling",
        module: "eventmodeling_root_overrides_11_15_0",
        lookup: "lookup_eventmodeling_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "Flowchart",
        module: "flowchart_root_overrides_11_12_2",
        lookup: "lookup_flowchart_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "Mindmap",
        module: "mindmap_root_overrides_11_12_2",
        lookup: "lookup_mindmap_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "Pie",
        module: "pie_root_overrides_11_12_2",
        lookup: "lookup_pie_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "Sankey",
        module: "sankey_root_overrides_11_12_2",
        lookup: "lookup_sankey_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "Sequence",
        module: "sequence_root_overrides_11_16_0",
        lookup: "lookup_sequence_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "State",
        module: "state_root_overrides_11_12_2",
        lookup: "lookup_state_root_viewport_override",
    },
    RootViewportTableRoute {
        family_variant: "Timeline",
        module: "timeline_root_overrides_11_12_2",
        lookup: "lookup_timeline_root_viewport_override",
    },
];

fn check_root_viewport_architecture(
    generated_dir: &Path,
    parity_dir: &Path,
    source_root: &Path,
) -> Result<(), XtaskError> {
    let router_path = generated_dir.join("root_viewports.rs");
    let generated_mod_path = generated_dir.join("mod.rs");
    let router_text = read_text(&router_path)?;
    let generated_mod_text = read_text(&generated_mod_path)?;

    let mut violations = find_root_viewport_router_violations(&router_text);
    violations.extend(find_root_viewport_module_violations(&generated_mod_text));

    let mut files = collect_parity_rs_files(parity_dir)?;
    files.sort();

    for path in files {
        let Some(file_name) = path.strip_prefix(source_root).ok().map(report_path_name) else {
            continue;
        };
        let text = read_text(&path)?;
        violations.extend(find_root_viewport_renderer_violations(&file_name, &text));
    }

    if violations.is_empty() {
        return Ok(());
    }

    let details = violations
        .iter()
        .map(|violation| {
            format!(
                "{}:{}: {} ({})",
                violation.file_name,
                violation.line_number,
                violation.message,
                violation.line.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Err(XtaskError::VerifyFailed(format!(
        "root viewport ownership bypassed the typed generated router:\n{details}"
    )))
}

fn find_root_viewport_router_violations(text: &str) -> Vec<RootViewportArchitectureViolation> {
    const FILE_NAME: &str = "generated/root_viewports.rs";
    let mut violations = Vec::new();

    for (needle, message) in [
        (
            "use crate::family::RenderFamilyKind;",
            "typed router must import RenderFamilyKind",
        ),
        (
            "pub(crate) struct GeneratedRootViewport",
            "typed router must return GeneratedRootViewport",
        ),
        (
            "pub(crate) fn lookup_root_viewport_override",
            "typed root viewport lookup router is missing",
        ),
        (
            "family: RenderFamilyKind",
            "root viewport router must dispatch on RenderFamilyKind",
        ),
        (
            "Option<GeneratedRootViewport>",
            "root viewport router must return the typed viewport value",
        ),
        (
            "PINNED_MERMAID_BASELINE_VERSION",
            "root viewport router must reject non-pinned baselines",
        ),
        (
            "GeneratedRootViewport {",
            "raw generated table values must be wrapped in the typed viewport value",
        ),
        (
            "raw.map",
            "typed router must convert raw table tuples in one place",
        ),
    ] {
        if !text.contains(needle) {
            violations.push(root_viewport_violation(FILE_NAME, text, 0, message));
        }
    }

    let arms = match parse_root_viewport_match_arms(text) {
        Ok(arms) => arms,
        Err(message) => {
            violations.push(root_viewport_violation(
                FILE_NAME,
                text,
                text.find("lookup_root_viewport_override")
                    .unwrap_or_default(),
                &message,
            ));
            return violations;
        }
    };

    for arm in &arms {
        if arm.wildcard || arm.variants.is_empty() {
            violations.push(root_viewport_violation(
                FILE_NAME,
                text,
                text.find("match family").unwrap_or_default(),
                "RenderFamilyKind router must not use a wildcard or catch-all arm",
            ));
        }
    }

    for route in ROOT_VIEWPORT_TABLE_ROUTES {
        let family = format!("RenderFamilyKind::{}", route.family_variant);
        let matching_arms = arms
            .iter()
            .filter(|arm| {
                arm.variants
                    .iter()
                    .any(|variant| variant == route.family_variant)
            })
            .collect::<Vec<_>>();
        if matching_arms.len() != 1
            || matching_arms[0].variants.as_slice() != [route.family_variant]
            || !matching_arms[0].calls_lookup(route.module, route.lookup)
        {
            let offset = text.find(&family).unwrap_or_default();
            violations.push(root_viewport_violation(
                FILE_NAME,
                text,
                offset,
                &format!(
                    "{family} must route exactly once through private table `{}`",
                    route.module
                ),
            ));
        }
    }

    for arm in arms {
        let owns_generated_table = arm.variants.iter().any(|variant| {
            ROOT_VIEWPORT_TABLE_ROUTES
                .iter()
                .any(|route| route.family_variant == variant)
        });
        if !arm.variants.is_empty() && !owns_generated_table && !arm.returns_none {
            violations.push(root_viewport_violation(
                FILE_NAME,
                text,
                text.find("match family").unwrap_or_default(),
                "families without generated root tables must explicitly return None",
            ));
        }
    }

    violations
}

fn find_root_viewport_module_violations(text: &str) -> Vec<RootViewportArchitectureViolation> {
    const FILE_NAME: &str = "generated/mod.rs";
    let mut violations = Vec::new();

    for route in ROOT_VIEWPORT_TABLE_ROUTES {
        let private_declaration = format!("mod {};", route.module);
        let declarations = text
            .lines()
            .enumerate()
            .filter_map(|(line_index, line)| {
                let code = strip_line_comment(line).trim();
                code.ends_with(&private_declaration)
                    .then_some((line_index + 1, line, code))
            })
            .collect::<Vec<_>>();
        if declarations.len() != 1 {
            violations.push(RootViewportArchitectureViolation {
                file_name: FILE_NAME.to_string(),
                line_number: declarations.first().map_or(1, |item| item.0),
                line: declarations
                    .first()
                    .map_or_else(String::new, |item| item.1.to_string()),
                message: format!(
                    "generated root table `{}` must have exactly one module declaration",
                    route.module
                ),
            });
            continue;
        }

        let (line_number, line, code) = declarations[0];
        if code != private_declaration {
            violations.push(RootViewportArchitectureViolation {
                file_name: FILE_NAME.to_string(),
                line_number,
                line: line.to_string(),
                message: format!(
                    "generated root table `{}` must remain a private module",
                    route.module
                ),
            });
        }
    }

    violations
}

fn find_root_viewport_renderer_violations(
    file_name: &str,
    text: &str,
) -> Vec<RootViewportArchitectureViolation> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r#"(?:\blookup_[A-Za-z0-9_]+_root_viewport_override\b|\b[A-Za-z0-9_]+_root_overrides_[A-Za-z0-9_]+\b)"#,
        )
        .expect("valid regex")
    });

    let mut violations = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let code = strip_line_comment(line);
        if re.is_match(code) {
            violations.push(RootViewportArchitectureViolation {
                file_name: file_name.to_string(),
                line_number: line_index + 1,
                line: line.to_string(),
                message: "family renderer must not reference generated root tables directly"
                    .to_string(),
            });
        }
    }

    violations
}

fn root_viewport_violation(
    file_name: &str,
    text: &str,
    offset: usize,
    message: &str,
) -> RootViewportArchitectureViolation {
    let line_number = text[..offset.min(text.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let line = text.lines().nth(line_number - 1).unwrap_or_default();
    RootViewportArchitectureViolation {
        file_name: file_name.to_string(),
        line_number,
        line: line.to_string(),
        message: message.to_string(),
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ParsedRootViewportMatchArm {
    variants: Vec<String>,
    wildcard: bool,
    returns_none: bool,
    paths: Vec<Vec<String>>,
}

impl ParsedRootViewportMatchArm {
    fn calls_lookup(&self, module: &str, lookup: &str) -> bool {
        self.paths.iter().any(|path| {
            path.len() >= 2 && path[path.len() - 2] == module && path[path.len() - 1] == lookup
        })
    }
}

fn parse_root_viewport_match_arms(text: &str) -> Result<Vec<ParsedRootViewportMatchArm>, String> {
    let file = syn::parse_file(text)
        .map_err(|error| format!("typed root viewport router must parse as Rust: {error}"))?;
    let function = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "lookup_root_viewport_override" => {
                Some(function)
            }
            _ => None,
        })
        .ok_or_else(|| "typed root viewport lookup function is missing".to_string())?;
    let family_match = function
        .block
        .stmts
        .iter()
        .find_map(|statement| {
            let syn::Stmt::Local(local) = statement else {
                return None;
            };
            let syn::Pat::Ident(binding) = &local.pat else {
                return None;
            };
            if binding.ident != "raw" {
                return None;
            }
            let expression = local.init.as_ref()?.expr.as_ref();
            let syn::Expr::Match(family_match) = expression else {
                return None;
            };
            expr_is_ident(&family_match.expr, "family").then_some(family_match)
        })
        .ok_or_else(|| {
            "typed router must assign `raw` from an explicit `match family`".to_string()
        })?;

    Ok(family_match
        .arms
        .iter()
        .map(|arm| {
            let mut variants = Vec::new();
            let mut wildcard = false;
            collect_render_family_pattern(&arm.pat, &mut variants, &mut wildcard);
            let mut paths = RootViewportPathCollector::default();
            paths.visit_expr(&arm.body);
            ParsedRootViewportMatchArm {
                variants,
                wildcard,
                returns_none: expr_is_none(&arm.body),
                paths: paths.paths,
            }
        })
        .collect())
}

fn expr_is_ident(expression: &syn::Expr, expected: &str) -> bool {
    let syn::Expr::Path(path) = expression else {
        return false;
    };
    path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == expected
}

fn expr_is_none(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Path(path) => {
            path.qself.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].ident == "None"
        }
        syn::Expr::Group(group) => expr_is_none(&group.expr),
        syn::Expr::Paren(paren) => expr_is_none(&paren.expr),
        syn::Expr::Block(block) => block.block.stmts.last().is_some_and(|statement| {
            matches!(statement, syn::Stmt::Expr(expression, None) if expr_is_none(expression))
        }),
        _ => false,
    }
}

fn collect_render_family_pattern(
    pattern: &syn::Pat,
    variants: &mut Vec<String>,
    wildcard: &mut bool,
) {
    match pattern {
        syn::Pat::Or(or_pattern) => {
            for case in &or_pattern.cases {
                collect_render_family_pattern(case, variants, wildcard);
            }
        }
        syn::Pat::Path(path_pattern) => {
            let segments = path_pattern.path.segments.iter().collect::<Vec<_>>();
            if segments.len() >= 2 && segments[segments.len() - 2].ident == "RenderFamilyKind" {
                variants.push(segments[segments.len() - 1].ident.to_string());
            }
        }
        syn::Pat::Paren(paren) => {
            collect_render_family_pattern(&paren.pat, variants, wildcard);
        }
        syn::Pat::Reference(reference) => {
            collect_render_family_pattern(&reference.pat, variants, wildcard);
        }
        syn::Pat::Type(typed) => {
            collect_render_family_pattern(&typed.pat, variants, wildcard);
        }
        syn::Pat::Wild(_) => *wildcard = true,
        _ => {}
    }
}

#[derive(Default)]
struct RootViewportPathCollector {
    paths: Vec<Vec<String>>,
}

impl<'ast> Visit<'ast> for RootViewportPathCollector {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.paths.push(
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect(),
        );
        syn::visit::visit_path(self, path);
    }
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _)| code)
}

fn collect_generated_override_footprint_entries(
    generated_dir: &Path,
) -> Result<Vec<OverrideFootprintEntry>, XtaskError> {
    let mut files = collect_generated_rs_files(generated_dir)?;
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let Some(file_name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if file_name == "mod.rs" {
            continue;
        }

        let text = read_text(&path)?;
        entries.extend(classify_generated_override_file(file_name, &text));
    }

    Ok(entries)
}

fn collect_manual_bridge_footprint_entries(
    parity_dir: &Path,
    source_root: &Path,
) -> Result<Vec<OverrideFootprintEntry>, XtaskError> {
    let mut files = collect_parity_rs_files(parity_dir)?;
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let Some(file_name) = path.strip_prefix(source_root).ok().map(report_path_name) else {
            continue;
        };
        let text = read_text(&path)?;
        let count = count_manual_bridge_functions(text.as_str());
        if count == 0 {
            continue;
        }
        entries.push(OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::RawPathBridge,
            count,
            unit: "bridge functions",
        });
    }

    Ok(entries)
}

fn collect_generated_rs_files(generated_dir: &Path) -> Result<Vec<PathBuf>, XtaskError> {
    let read_dir = fs::read_dir(generated_dir).map_err(|source| XtaskError::ReadFile {
        path: generated_dir.display().to_string(),
        source,
    })?;

    let mut files = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: generated_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }

    Ok(files)
}

fn collect_parity_rs_files(parity_dir: &Path) -> Result<Vec<PathBuf>, XtaskError> {
    let mut stack = vec![parity_dir.to_path_buf()];
    let mut files = Vec::new();

    while let Some(dir) = stack.pop() {
        let read_dir = fs::read_dir(&dir).map_err(|source| XtaskError::ReadFile {
            path: dir.display().to_string(),
            source,
        })?;
        for entry in read_dir {
            let entry = entry.map_err(|source| XtaskError::ReadFile {
                path: dir.display().to_string(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn classify_generated_override_file(file_name: String, text: &str) -> Vec<OverrideFootprintEntry> {
    if file_name.contains("_root_overrides_") {
        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::RootViewport,
            count: count_root_viewport_entries(text),
            unit: "entries",
        }];
    }

    if file_name.starts_with("font_metrics_") {
        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::FontMetrics,
            count: count_tuple_rows(text),
            unit: "table rows",
        }];
    }

    if file_name.starts_with("svg_overrides_") {
        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::SvgTextMetrics,
            count: count_tuple_rows(text),
            unit: "table rows",
        }];
    }

    if file_name.contains("_text_overrides_") {
        if file_name == format!("c4_text_overrides_{LEGACY_GENERATED_BASELINE_SUFFIX}.rs") {
            return vec![OverrideFootprintEntry {
                file_name,
                category: OverrideCategory::TextLookup,
                count: count_local_lookup_table_rows(text),
                unit: "lookup entries",
            }];
        }

        if file_name == format!("class_text_overrides_{LEGACY_GENERATED_BASELINE_SUFFIX}.rs") {
            let class_entries = classify_class_text_override_file(&file_name, text);
            if !class_entries.is_empty() {
                return class_entries;
            }
        }

        let lookup_entries = count_some_match_arms(text) + count_static_override_table_rows(text);
        if lookup_entries > 0 {
            return vec![OverrideFootprintEntry {
                file_name,
                category: OverrideCategory::TextLookup,
                count: lookup_entries,
                unit: "lookup entries",
            }];
        }

        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::HandCuratedHelpers,
            count: count_visible_functions(text),
            unit: "helper functions",
        }];
    }

    Vec::new()
}

fn classify_class_text_override_file(file_name: &str, text: &str) -> Vec<OverrideFootprintEntry> {
    let mut entries = Vec::new();
    for (fn_name, label) in [
        ("lookup_class_calc_text_width_px", "calc text width entries"),
        ("lookup_class_rendered_width_px", "rendered width entries"),
        ("lookup_class_namespace_width_px", "namespace width entries"),
        ("lookup_class_note_width_px", "note width entries"),
    ] {
        let count = count_some_match_arms_in_function(text, fn_name);
        if count == 0 {
            continue;
        }
        entries.push(OverrideFootprintEntry {
            file_name: format!("{file_name}::{fn_name}"),
            category: OverrideCategory::TextLookup,
            count,
            unit: label,
        });
    }
    entries
}

fn print_category(entries: &[OverrideFootprintEntry], category: OverrideCategory) {
    let category_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.category == category)
        .collect();

    let total: usize = category_entries.iter().map(|entry| entry.count).sum();
    let metadata = category.metadata();
    println!("{}:", category.heading());
    println!("- owner: {}", metadata.owner);
    println!("- source: {}", metadata.source);
    println!("- allowed use: {}", metadata.allowed_use);
    println!("- expected removal: {}", metadata.expected_removal);
    println!("- total: {total} {}", category.total_unit());
    if category_entries.is_empty() {
        println!("- no entries");
    } else {
        for entry in category_entries {
            println!("- {}: {} {}", entry.file_name, entry.count, entry.unit);
        }
    }
    println!();
}

fn category_total(entries: &[OverrideFootprintEntry], category: OverrideCategory) -> usize {
    entries
        .iter()
        .filter(|entry| entry.category == category)
        .map(|entry| entry.count)
        .sum()
}

fn read_text(path: &Path) -> Result<String, XtaskError> {
    fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

fn report_path_name(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn pinned_mermaid_baseline_label(workspace_root: &Path) -> String {
    let lock_path = workspace_root
        .join("tools")
        .join("upstreams")
        .join("REPOS.lock.json");
    let Ok(text) = fs::read_to_string(lock_path) else {
        return format!("@{PINNED_MERMAID_BASELINE_VERSION}");
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return format!("@{PINNED_MERMAID_BASELINE_VERSION}");
    };
    let Some(reference) = value
        .get("repos")
        .and_then(|repos| repos.get("mermaid"))
        .and_then(|mermaid| mermaid.get("ref"))
        .and_then(|reference| reference.as_str())
        .filter(|reference| !reference.trim().is_empty())
    else {
        return format!("@{PINNED_MERMAID_BASELINE_VERSION}");
    };

    reference
        .strip_prefix("mermaid")
        .map(|suffix| suffix.to_string())
        .unwrap_or_else(|| {
            PINNED_MERMAID_BASELINE_TAG
                .strip_prefix("mermaid")
                .unwrap_or(PINNED_MERMAID_BASELINE_TAG)
                .to_string()
        })
}

fn count_root_viewport_entries(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r#""[^"]+"\s*=>\s*(?:\{\s*)?Some\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_some_match_arms(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"=>\s*(?:\{\s*)?Some\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_some_match_arms_in_function(text: &str, fn_name: &str) -> usize {
    let Some(body) = extract_function_body(text, fn_name) else {
        return 0;
    };
    count_some_match_arms(body)
}

fn count_tuple_rows(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"(?m)^\s*\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_static_override_table_rows(text: &str) -> usize {
    let mut in_override_table = false;
    let mut rows = 0usize;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if !in_override_table {
            in_override_table = trimmed.starts_with("static ")
                && trimmed.contains("_OVERRIDES")
                && trimmed.contains("&[");
            continue;
        }

        if trimmed.starts_with("];") {
            in_override_table = false;
            continue;
        }

        if trimmed.starts_with('(') {
            rows += 1;
        }
    }

    rows
}

fn count_local_lookup_table_rows(text: &str) -> usize {
    let mut in_lookup_table = false;
    let mut rows = 0usize;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if !in_lookup_table {
            in_lookup_table =
                trimmed.starts_with("let ") && trimmed.contains("tbl") && trimmed.contains("&[");
            continue;
        }

        if trimmed.starts_with("];") {
            in_lookup_table = false;
            continue;
        }

        if trimmed.starts_with('(') {
            rows += 1;
        }
    }

    rows
}

fn count_visible_functions(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?m)^pub(?:\([^)]+\))?\s+fn\s+[A-Za-z0-9_]+\s*\("#).expect("valid regex")
    });
    count_matches(re, text)
}

fn count_manual_bridge_functions(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?m)^(?:pub(?:\([^)]+\))?\s+)?fn\s+maybe_override_[A-Za-z0-9_]+\s*\("#)
            .expect("valid regex")
    });
    count_matches(re, text)
}

fn count_matches(re: &Regex, text: &str) -> usize {
    re.find_iter(text).count()
}

fn extract_function_body<'a>(text: &'a str, fn_name: &str) -> Option<&'a str> {
    let fn_marker = format!("fn {fn_name}(");
    let start = text.find(&fn_marker)?;
    let body_start = text[start..].find('{')? + start + 1;
    let body = &text[body_start..];
    let mut depth = 1i32;
    for (idx, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&body[..idx]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        OverrideCategory, OverrideFootprintEntry, check_override_no_growth,
        classify_class_text_override_file, classify_generated_override_file,
        count_manual_bridge_functions, count_some_match_arms, count_some_match_arms_in_function,
        count_static_override_table_rows, count_visible_functions, extract_function_body,
        find_root_viewport_module_violations, find_root_viewport_renderer_violations,
        find_root_viewport_router_violations, pinned_mermaid_baseline_label, report_path_name,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn counts_manual_bridge_functions_in_flowchart_path_override() {
        let text = r#"
//! Flowchart edge path overrides.
pub(in crate::svg::parity::flowchart) fn maybe_override_degenerate_subgraph_edge_path_d(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    data_points: &[crate::model::LayoutPoint],
) -> Option<String> {
    None
}
"#;

        assert_eq!(count_manual_bridge_functions(text), 1);
    }

    #[test]
    fn ignores_non_bridge_functions() {
        let text = r#"
pub fn not_a_bridge() {}
fn definitely_not_a_bridge() {}
"#;

        assert_eq!(count_manual_bridge_functions(text), 0);
    }

    #[test]
    fn counts_visible_helper_functions() {
        let text = r#"
pub fn helper_one() {}
pub(crate) fn helper_two(
) {}
pub(in crate::cmd) fn helper_three(
) {}
fn private_helper() {}
"#;

        assert_eq!(count_visible_functions(text), 3);
    }

    #[test]
    fn counts_block_wrapped_some_match_arms() {
        let text = r#"
match (font_key, text) {
    ("trebuchetms,verdana,arial,sans-serif", "wide label") => {
        Some((84.1328125, 84.1328125))
    }
    ("trebuchetms,verdana,arial,sans-serif", "short label") => Some(42.0),
    _ => None,
}
"#;

        assert_eq!(count_some_match_arms(text), 2);
    }

    #[test]
    fn counts_static_override_lookup_rows() {
        let text = r#"
static HTML_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &[
    (1600, "A", 9.4375),
    (
        2400,
        "Font size precedence should widen this block",
        487.890625,
    ),
];

static OTHER_ROWS: &[(u16, &str, f64)] = &[
    (1600, "ignored", 1.0),
];
"#;

        assert_eq!(count_static_override_table_rows(text), 2);
    }

    #[test]
    fn counts_some_match_arms_only_within_named_function() {
        let text = r#"
pub fn lookup_a() -> Option<i32> {
    match 1 {
        1 => Some(1),
        _ => None,
    }
}

pub fn lookup_b() -> Option<i32> {
    match 2 {
        2 => {
            Some(2)
        }
        _ => None,
    }
}
"#;

        assert_eq!(count_some_match_arms_in_function(text, "lookup_a"), 1);
        assert_eq!(count_some_match_arms_in_function(text, "lookup_b"), 1);
    }

    #[test]
    fn extracts_function_body_for_classification() {
        let text = r#"
pub fn lookup_sample() -> Option<i32> {
    if true {
        return Some(1);
    }
    None
}
"#;

        let body = extract_function_body(text, "lookup_sample").expect("body");
        assert!(body.contains("return Some(1);"));
        assert!(!body.contains("pub fn"));
    }

    #[test]
    fn class_text_override_file_reports_per_lookup_section() {
        let text = r#"
pub fn lookup_class_calc_text_width_px(font_size_px: i64, text: &str) -> Option<i64> {
    match (font_size_px, text.trim()) {
        (16, "A") => Some(10),
        _ => None,
    }
}

pub fn lookup_class_rendered_width_px(font_size_px: i64, is_bold: bool, text: &str) -> Option<f64> {
    match (font_size_px, is_bold, text.trim()) {
        (16, true, "B") => Some(20.0),
        (16, false, "C") => {
            Some(21.0)
        }
        _ => None,
    }
}
"#;

        let entries = classify_class_text_override_file("class_text_overrides_11_12_2.rs", text);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].file_name,
            "class_text_overrides_11_12_2.rs::lookup_class_calc_text_width_px"
        );
        assert_eq!(entries[0].count, 1);
        assert_eq!(
            entries[1].file_name,
            "class_text_overrides_11_12_2.rs::lookup_class_rendered_width_px"
        );
        assert_eq!(entries[1].count, 2);
    }

    #[test]
    fn classifies_static_text_tables_as_lookup_entries() {
        let text = r#"
static TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &[
    (1100, "Task", 22.24853515625),
    (1100, "Task2", 27.796875),
];

pub fn lookup_task_text_bbox_width_px(font_size: f64, text: &str) -> Option<f64> {
    let _ = (font_size, text);
    None
}
"#;

        let entries =
            classify_generated_override_file("er_text_overrides_11_12_2.rs".to_string(), text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, OverrideCategory::TextLookup);
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[0].unit, "lookup entries");
    }

    #[test]
    fn classifies_c4_local_text_tables_as_lookup_entries() {
        let text = r#"
pub fn lookup_c4_text_width_px(
    font_key: &str,
    font_size_key: usize,
    font_weight: &str,
    text: &str,
) -> Option<f64> {
    match (font_key, font_size_key, font_weight) {
        ("opensans,sans-serif", 14000, "normal") => {
            let tbl: &[(&str, f64)] = &[
                ("Customer", 75.0),
                ("System", 47.0),
            ];
            lookup_in(tbl, text)
        }
        _ => None,
    }
}
"#;

        let entries =
            classify_generated_override_file("c4_text_overrides_11_12_2.rs".to_string(), text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, OverrideCategory::TextLookup);
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[0].unit, "lookup entries");
    }

    #[test]
    fn report_paths_are_stable_across_platforms() {
        assert_eq!(
            report_path_name(Path::new(
                r"svg\parity\flowchart\edge_geom\degenerate_path.rs"
            )),
            "svg/parity/flowchart/edge_geom/degenerate_path.rs"
        );
    }

    #[test]
    fn pinned_mermaid_baseline_label_reads_lockfile_ref() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "merman-report-overrides-test-{}-{unique}",
            std::process::id()
        ));
        let lock_dir = dir.join("tools").join("upstreams");
        fs::create_dir_all(&lock_dir).expect("lock dir");
        fs::write(
            lock_dir.join("REPOS.lock.json"),
            r#"{"repos":{"mermaid":{"ref":"mermaid@11.16.0"}}}"#,
        )
        .expect("lockfile");

        assert_eq!(pinned_mermaid_baseline_label(&dir), "@11.16.0");

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn generated_categories_report_removal_metadata() {
        for category in [
            OverrideCategory::RootViewport,
            OverrideCategory::TextLookup,
            OverrideCategory::SvgTextMetrics,
            OverrideCategory::FontMetrics,
            OverrideCategory::HandCuratedHelpers,
        ] {
            let metadata = category.metadata();
            assert!(!metadata.source.is_empty());
            assert!(!metadata.allowed_use.is_empty());
            assert!(!metadata.expected_removal.is_empty());
        }
    }

    #[test]
    fn override_growth_check_allows_current_budget() {
        let entries: Vec<_> = OverrideCategory::ALL
            .into_iter()
            .map(|category| OverrideFootprintEntry {
                file_name: category.heading().to_string(),
                category,
                count: category.no_growth_budget(),
                unit: category.total_unit(),
            })
            .collect();

        assert!(check_override_no_growth(&entries).is_ok());
    }

    #[test]
    fn override_growth_check_rejects_category_growth() {
        let entries = [OverrideFootprintEntry {
            file_name: "flowchart_root_overrides_11_12_2.rs".to_string(),
            category: OverrideCategory::RootViewport,
            count: OverrideCategory::RootViewport.no_growth_budget() + 1,
            unit: "entries",
        }];

        let err = check_override_no_growth(&entries).expect_err("growth should fail");
        let msg = err.to_string();
        assert!(msg.contains("Root viewport overrides grew"));
        assert!(msg.contains("budget 192"));
    }

    #[test]
    fn override_growth_check_rejects_manual_bridge_growth() {
        let entries = [OverrideFootprintEntry {
            file_name: "svg/parity/flowchart/edge_geom/degenerate_path.rs".to_string(),
            category: OverrideCategory::RawPathBridge,
            count: 1,
            unit: "bridge functions",
        }];

        let err = check_override_no_growth(&entries).expect_err("bridge growth should fail");
        let msg = err.to_string();
        assert!(msg.contains("Manual raw SVG/path bridges grew"));
        assert!(msg.contains("budget 0"));
    }

    #[test]
    fn root_viewport_router_accepts_typed_table_dispatch_and_explicit_none_arm() {
        let violations = find_root_viewport_router_violations(valid_root_viewport_router());
        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:#?}"
        );
    }

    #[test]
    fn root_viewport_router_accepts_block_arms_without_commas() {
        let router = valid_root_viewport_router().replace(
            "RenderFamilyKind::C4 => super::c4_root_overrides_11_12_2::lookup_c4_root_viewport_override(diagram_id),",
            "RenderFamilyKind::C4 => {\n                super::c4_root_overrides_11_12_2::lookup_c4_root_viewport_override(diagram_id)\n            }",
        );

        let violations = find_root_viewport_router_violations(&router);
        assert!(
            violations.is_empty(),
            "block match arms are valid Rust and must remain auditable: {violations:#?}"
        );
    }

    #[test]
    fn root_viewport_router_rejects_missing_table_route() {
        let router = valid_root_viewport_router().replace(
            "RenderFamilyKind::C4 => super::c4_root_overrides_11_12_2::lookup_c4_root_viewport_override(diagram_id),",
            "RenderFamilyKind::C4 => None,",
        );

        let violations = find_root_viewport_router_violations(&router);
        assert!(violations.iter().any(|violation| {
            violation.message.contains("RenderFamilyKind::C4")
                && violation.message.contains("c4_root_overrides_11_12_2")
        }));
    }

    #[test]
    fn root_viewport_router_rejects_wildcard_fallback() {
        let router =
            valid_root_viewport_router().replace("RenderFamilyKind::Error => None,", "_ => None,");

        let violations = find_root_viewport_router_violations(&router);
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains("wildcard"))
        );
    }

    #[test]
    fn root_viewport_modules_must_be_private() {
        let generated_mod = private_root_viewport_modules().replace(
            "mod c4_root_overrides_11_12_2;",
            "pub mod c4_root_overrides_11_12_2;",
        );

        let violations = find_root_viewport_module_violations(&generated_mod);
        assert!(violations.iter().any(|violation| {
            violation.message.contains("c4_root_overrides_11_12_2")
                && violation.message.contains("private")
        }));
    }

    #[test]
    fn root_viewport_renderer_rejects_direct_generated_lookup() {
        let text = r#"
fn apply(diagram_id: &str) {
    if let Some((viewbox, max_w)) =
        crate::generated::state_root_overrides_11_12_2::lookup_state_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_attr = max_w.to_string();
    }
}
"#;

        let violations = find_root_viewport_renderer_violations("state/render.rs", text);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].file_name, "state/render.rs");
        assert_eq!(violations[0].line_number, 4);
    }

    #[test]
    fn root_viewport_renderer_ignores_line_comments() {
        let text = r#"
fn apply() {
    // crate::generated::state_root_overrides_11_12_2::lookup_state_root_viewport_override
}
"#;

        assert!(find_root_viewport_renderer_violations("state/render.rs", text).is_empty());
    }

    fn valid_root_viewport_router() -> &'static str {
        r#"
use crate::family::RenderFamilyKind;

pub(crate) struct GeneratedRootViewport {
    pub(crate) view_box: &'static str,
    pub(crate) max_width: &'static str,
}

pub(crate) fn lookup_root_viewport_override(
    family: RenderFamilyKind,
    baseline_version: &str,
    diagram_id: &str,
) -> Option<GeneratedRootViewport> {
    if baseline_version != merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION {
        return None;
    }

    let raw = match family {
        RenderFamilyKind::C4 => super::c4_root_overrides_11_12_2::lookup_c4_root_viewport_override(diagram_id),
        RenderFamilyKind::Er => super::er_root_overrides_11_12_2::lookup_er_root_viewport_override(diagram_id),
        RenderFamilyKind::EventModeling => super::eventmodeling_root_overrides_11_15_0::lookup_eventmodeling_root_viewport_override(diagram_id),
        RenderFamilyKind::Flowchart => super::flowchart_root_overrides_11_12_2::lookup_flowchart_root_viewport_override(diagram_id),
        RenderFamilyKind::Mindmap => super::mindmap_root_overrides_11_12_2::lookup_mindmap_root_viewport_override(diagram_id),
        RenderFamilyKind::Pie => super::pie_root_overrides_11_12_2::lookup_pie_root_viewport_override(diagram_id),
        RenderFamilyKind::Sankey => super::sankey_root_overrides_11_12_2::lookup_sankey_root_viewport_override(diagram_id),
        RenderFamilyKind::Sequence => super::sequence_root_overrides_11_16_0::lookup_sequence_root_viewport_override(diagram_id),
        RenderFamilyKind::State => super::state_root_overrides_11_12_2::lookup_state_root_viewport_override(diagram_id),
        RenderFamilyKind::Timeline => super::timeline_root_overrides_11_12_2::lookup_timeline_root_viewport_override(diagram_id),
        RenderFamilyKind::Error => None,
    };

    raw.map(|(view_box, max_width)| GeneratedRootViewport {
        view_box,
        max_width,
    })
}
"#
    }

    fn private_root_viewport_modules() -> &'static str {
        r#"
mod c4_root_overrides_11_12_2;
mod er_root_overrides_11_12_2;
mod eventmodeling_root_overrides_11_15_0;
mod flowchart_root_overrides_11_12_2;
mod mindmap_root_overrides_11_12_2;
mod pie_root_overrides_11_12_2;
mod sankey_root_overrides_11_12_2;
mod sequence_root_overrides_11_16_0;
mod state_root_overrides_11_12_2;
mod timeline_root_overrides_11_12_2;
"#
    }
}
