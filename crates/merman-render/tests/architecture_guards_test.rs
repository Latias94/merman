use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{
    BinOp, Expr, ExprBinary, FnArg, ImplItemFn, ItemFn, LitFloat, LitStr, Pat, Signature, Type,
};

const FORBIDDEN_CENTRAL_SYMBOLS: &[&str] = &[
    "LayoutDiagram",
    "LayoutedDiagram",
    "layout_parsed",
    "layout_json_by_type",
    "render_layouted_svg",
    "render_layout_svg_parts",
];

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, sources: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).expect("read Rust source directory") {
            let entry = entry.expect("read Rust source entry");
            let path = entry.path();
            if path.is_dir() {
                visit(&path, sources);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                sources.push(path);
            }
        }
    }

    let mut sources = Vec::new();
    visit(root, &mut sources);
    sources.sort();
    sources
}

fn is_json_value_reference(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => is_json_value_type(&reference.elem),
        Type::Group(group) => is_json_value_reference(&group.elem),
        Type::Paren(paren) => is_json_value_reference(&paren.elem),
        _ => false,
    }
}

fn is_json_value_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Value"),
        Type::Group(group) => is_json_value_type(&group.elem),
        Type::Paren(paren) => is_json_value_type(&paren.elem),
        _ => false,
    }
}

fn json_model_parameter(signature: &Signature) -> Option<&str> {
    signature.inputs.iter().find_map(|argument| {
        let FnArg::Typed(argument) = argument else {
            return None;
        };
        let Pat::Ident(name) = argument.pat.as_ref() else {
            return None;
        };
        matches!(name.ident.to_string().as_str(), "semantic" | "model")
            .then(|| is_json_value_reference(&argument.ty))
            .filter(|is_json| *is_json)
            .map(|_| {
                if name.ident == "semantic" {
                    "semantic"
                } else {
                    "model"
                }
            })
    })
}

struct ArchitectureGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

impl ArchitectureGuard<'_> {
    fn inspect_signature(&mut self, signature: &Signature) {
        let name = signature.ident.to_string();
        let Some(parameter) = json_model_parameter(signature) else {
            return;
        };

        if name.starts_with("layout_") {
            self.violations.push(format!(
                "{}: `{name}` accepts raw JSON `{parameter}` input",
                self.relative_path
            ));
        }
        if name.starts_with("render_") && name.contains("svg") {
            self.violations.push(format!(
                "{}: `{name}` accepts a raw JSON semantic model",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for ArchitectureGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        self.inspect_signature(&function.sig);
        visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast ImplItemFn) {
        self.inspect_signature(&function.sig);
        visit::visit_impl_item_fn(self, function);
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if FORBIDDEN_CENTRAL_SYMBOLS.contains(&ident.to_string().as_str()) {
            self.violations.push(format!(
                "{}: legacy central render symbol `{ident}` remains",
                self.relative_path
            ));
        }
    }
}

#[test]
fn built_in_rendering_has_no_json_input_or_central_dispatch_fallback() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for path in rust_sources(&source_root) {
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        let relative = path
            .strip_prefix(&source_root)
            .unwrap_or(&path)
            .to_string_lossy();
        ArchitectureGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "built-in rendering must stay typed from family semantics through SVG:\n{}",
        violations.join("\n")
    );
}

struct CanonicalFunctionNameGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

impl<'ast> Visit<'ast> for CanonicalFunctionNameGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if is_test_only(&function.attrs) {
            return;
        }

        let name = function.sig.ident.to_string();
        let is_canonical_entry = ["layout_", "render_", "try_render_", "build_", "debug_"]
            .iter()
            .any(|prefix| name.starts_with(prefix));
        let has_version_segment = name.split('_').any(|segment| segment == "v2");
        if is_canonical_entry && has_version_segment {
            self.violations.push(format!(
                "{}: canonical family function `{name}` carries a transition version",
                self.relative_path
            ));
        }

        visit::visit_item_fn(self, function);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }
}

#[test]
fn canonical_family_functions_are_versionless() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for path in rust_sources(&source_root) {
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        let relative = path
            .strip_prefix(&source_root)
            .unwrap_or(&path)
            .to_string_lossy();
        CanonicalFunctionNameGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "canonical family functions must not expose transition version names:\n{}",
        violations.join("\n")
    );
}

struct RootOwnershipGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

const ROOT_NUMERIC_POLICY_FILES: &[&str] = &[
    "root_svg.rs",
    "architecture/viewport.rs",
    "flowchart/document.rs",
    "timeline/render.rs",
    "sequence/root.rs",
    "state/render.rs",
    "er/render.rs",
    "gantt/render.rs",
    "gitgraph/render.rs",
];

const FORBIDDEN_FLOWCHART_LATTICE_HELPERS: &[&str] = &[
    "f32_dims",
    "maybe_snap_data_point_to_f32",
    "maybe_snap_shallow_basis_triplet_y_to_f32",
    "maybe_truncate_data_point",
];

struct RootNumericPolicyGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

struct FlowchartF64GeometryGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

struct FlowchartMeasurementAuthorityGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
    in_computed_length_owner: bool,
}

struct CreateTextBBoxYOffsetGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

struct StateSvgTopologyGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

struct StateCompensationGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

fn is_test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || (attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Meta>()
                    .is_ok_and(|meta| cfg_requires_test(&meta)))
    })
}

fn cfg_requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|items| items.iter().any(cfg_requires_test)),
        syn::Meta::List(list) if list.path.is_ident("any") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|items| !items.is_empty() && items.iter().all(cfg_requires_test)),
        syn::Meta::List(_) | syn::Meta::NameValue(_) => false,
    }
}

#[test]
fn test_only_cfg_detection_respects_boolean_structure() {
    fn classified_as_test_only(attribute: &str) -> bool {
        let function: ItemFn = syn::parse_str(&format!("{attribute}\nfn guarded() {{}}"))
            .expect("parse attributed function");
        is_test_only(&function.attrs)
    }

    assert!(classified_as_test_only("#[test]"));
    assert!(classified_as_test_only("#[cfg(test)]"));
    assert!(classified_as_test_only("#[cfg(all(test, unix))]"));
    assert!(!classified_as_test_only("#[cfg(not(test))]"));
    assert!(!classified_as_test_only("#[cfg(any(test, unix))]"));
}

impl<'ast> Visit<'ast> for RootOwnershipGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let name = ident.to_string();
        let forbidden = matches!(
            name.as_str(),
            "push_svg_root_open"
                | "push_svg_root_open_with_viewport_plan"
                | "SvgRootAttrs"
                | "SvgRootWidth"
                | "apply_root_viewport_override"
                | "build_root_viewport_plan"
                | "fmt_max_width_px"
                | "fmt_max_width_px_into"
                | "VIEWBOX_PLACEHOLDER"
                | "MAX_WIDTH_PLACEHOLDER"
        ) || (name.starts_with("lookup_")
            && name.ends_with("_root_viewport_override"));
        if forbidden {
            self.violations.push(format!(
                "{}: family bypasses Root Viewport ownership through `{name}`",
                self.relative_path
            ));
        }
    }

    fn visit_lit_str(&mut self, literal: &'ast LitStr) {
        let value = literal.value();
        if value.contains("<svg id=")
            || value.contains("__MERMAN_ROOT_")
            || value.contains("__MERMAID_VIEWBOX__")
            || value.contains("__MERMAID_MAX_WIDTH__")
        {
            self.violations.push(format!(
                "{}: family contains a direct root SVG emitter or viewport placeholder",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for RootNumericPolicyGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        if ty
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "f32")
        {
            self.violations.push(format!(
                "{}: root viewport math narrows an SVG/JS number to f32",
                self.relative_path
            ));
        }
        visit::visit_type_path(self, ty);
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let name = ident.to_string();
        if matches!(
            name.as_str(),
            "next_up_f32" | "f32_round_up" | "title_bbox_w_bias"
        ) {
            self.violations.push(format!(
                "{}: root viewport retains float-lattice compensation `{name}`",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for FlowchartF64GeometryGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        if ty
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "f32")
        {
            self.violations.push(format!(
                "{}: Flowchart geometry narrows a Dagre/ELK/JS number to f32",
                self.relative_path
            ));
        }
        visit::visit_type_path(self, ty);
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let name = ident.to_string();
        if FORBIDDEN_FLOWCHART_LATTICE_HELPERS.contains(&name.as_str()) {
            self.violations.push(format!(
                "{}: Flowchart geometry retains lattice helper `{name}`",
                self.relative_path
            ));
        }
    }

    fn visit_lit_float(&mut self, literal: &'ast LitFloat) {
        let Ok(value) = literal.base10_parse::<f64>() else {
            return;
        };
        if value == 262_144.0 {
            self.violations.push(format!(
                "{}: Flowchart geometry retains the fixture-derived 2^18 lattice",
                self.relative_path
            ));
        }
        if self.relative_path == "flowchart/node.rs"
            && ((value > 2.0 && value < 2.1)
                || (value > 14.0 && value < 14.1)
                || (value > 60.0 && value < 60.1))
        {
            self.violations.push(format!(
                "{}: Flowchart geometry retains a browser-derived circle bbox constant",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for FlowchartMeasurementAuthorityGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if is_test_only(&function.attrs) {
            return;
        }

        let previous = self.in_computed_length_owner;
        self.in_computed_length_owner =
            function.sig.ident == "flowchart_svg_plain_computed_width_px";
        visit::visit_item_fn(self, function);
        self.in_computed_length_owner = previous;
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if self.in_computed_length_owner && ident == "round_to_1_64_px" {
            self.violations.push(format!(
                "{}: Flowchart requantizes an operation-owned computed text length",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for CreateTextBBoxYOffsetGuard<'_> {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if ident == "svg_create_text_bbox_y_offset_px"
            || ident == "svg_create_text_middle_bbox_y_offset_px"
            || ident == "svg_create_text_middle_baseline_shift_px"
        {
            self.violations.push(format!(
                "{}: family code bypasses an operation-owned createText bbox-y measurement",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for StateSvgTopologyGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_lit_str(&mut self, literal: &'ast LitStr) {
        if literal.value().contains("cyclic-special") {
            self.violations.push(format!(
                "{}: State SVG infers self-loop topology from an internal Dagre helper id",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for StateCompensationGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_lit_float(&mut self, literal: &'ast LitFloat) {
        let Ok(value) = literal.base10_parse::<f64>() else {
            return;
        };
        if value > 14.0 && value < 14.1 {
            self.violations.push(format!(
                "{}: State layout retains a browser-derived stateEnd bbox constant",
                self.relative_path
            ));
        }
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if matches!(
            self.relative_path,
            "state/layout.rs" | "svg/parity/state/node.rs"
        ) && ident == "round_to_1_64_px"
        {
            self.violations.push(format!(
                "{}: State family requantizes an operation-owned text measurement",
                self.relative_path
            ));
        }
    }

    fn visit_expr_binary(&mut self, expression: &'ast ExprBinary) {
        let shifts_placeholder_y = self.relative_path == "svg/parity/state/render.rs"
            && matches!(expression.op, BinOp::AddAssign(_))
            && matches!(
                expression.left.as_ref(),
                Expr::Path(path)
                    if path.path.segments.last().is_some_and(|segment| segment.ident == "y")
            );
        if shifts_placeholder_y {
            self.violations.push(format!(
                "{}: State SVG offsets placeholder geometry with a fixture-derived bbox delta",
                self.relative_path
            ));
        }
        visit::visit_expr_binary(self, expression);
    }
}

#[test]
fn root_ownership_guard_rejects_direct_svg_emission() {
    let syntax = syn::parse_file(
        r#"
        fn render_family_root() {
            let _root = "<svg id=\"bypass\">";
        }
        "#,
    )
    .expect("parse direct root emitter");
    let mut violations = Vec::new();

    RootOwnershipGuard {
        relative_path: "mutation.rs",
        violations: &mut violations,
    }
    .visit_file(&syntax);

    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("direct root SVG emitter"));
}

#[test]
fn root_numeric_policy_guard_rejects_lattice_compensation() {
    let syntax = syn::parse_file(
        r#"
        fn build_root(width: f64) {
            let width = (width as f32) as f64;
            let title_bbox_w_bias = 1.0 / 128.0;
        }
        "#,
    )
    .expect("parse root numeric policy bypass");
    let mut violations = Vec::new();

    RootNumericPolicyGuard {
        relative_path: "mutation.rs",
        violations: &mut violations,
    }
    .visit_file(&syntax);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("narrows an SVG/JS number to f32"))
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("title_bbox_w_bias"))
    );
}

#[test]
fn root_viewport_numeric_policy_stays_f64_and_source_backed() {
    let parity_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/svg/parity");
    let mut violations = Vec::new();

    for relative in ROOT_NUMERIC_POLICY_FILES {
        let path = parity_root.join(relative);
        let source = std::fs::read_to_string(&path).expect("read root viewport owner");
        let syntax = syn::parse_file(&source).expect("parse root viewport owner");
        RootNumericPolicyGuard {
            relative_path: relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "root viewport math must preserve Mermaid's f64/JS Number semantics:\n{}",
        violations.join("\n")
    );
}

#[test]
fn flowchart_f64_geometry_guard_rejects_lattice_compensation() {
    let syntax = syn::parse_file(
        r#"
        fn normalize_data_point(value: f64) -> f64 {
            let scale = 262_144.0;
            let browser_anchor_bbox = 2.01;
            let browser_stop_bbox = 14.02;
            let browser_crossed_bbox = 60.02;
            maybe_snap_data_point_to_f32((value as f32) as f64) * scale
        }
        "#,
    )
    .expect("parse Flowchart lattice compensation");
    let mut violations = Vec::new();

    FlowchartF64GeometryGuard {
        relative_path: "flowchart/node.rs",
        violations: &mut violations,
    }
    .visit_file(&syntax);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("narrows a Dagre/ELK/JS number to f32"))
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("maybe_snap_data_point_to_f32"))
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("fixture-derived 2^18 lattice"))
    );
    assert!(
        violations
            .iter()
            .filter(|violation| violation.contains("browser-derived circle bbox"))
            .count()
            == 3
    );
}

#[test]
fn flowchart_geometry_preserves_f64_layout_and_svg_numbers() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();
    let mut paths = vec![
        source_root.join("flowchart/node.rs"),
        source_root.join("svg/parity/flowchart/edge_geom.rs"),
    ];
    paths.extend(rust_sources(
        &source_root.join("svg/parity/flowchart/edge_geom"),
    ));

    for path in paths {
        let relative = path
            .strip_prefix(&source_root)
            .unwrap_or(&path)
            .to_string_lossy();
        let source = std::fs::read_to_string(&path).expect("read Flowchart geometry owner");
        let syntax = syn::parse_file(&source).expect("parse Flowchart geometry owner");
        FlowchartF64GeometryGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "Flowchart geometry must preserve Dagre/ELK and Mermaid JS Number precision:\n{}",
        violations.join("\n")
    );
}

#[test]
fn flowchart_measurement_authority_guard_rejects_computed_length_lattice() {
    let syntax = syn::parse_file(
        r#"
        fn flowchart_svg_plain_computed_width_px(measurer: &Measurer, style: &Style) -> f64 {
            let width = measurer.measure_svg_text_computed_length_px("label", style);
            round_to_1_64_px(width)
        }

        fn source_backed_shape_width(width: f64) -> f64 {
            (width + 16.0).min(200.0)
        }
        "#,
    )
    .expect("parse Flowchart computed-length compensation");
    let mut violations = Vec::new();

    FlowchartMeasurementAuthorityGuard {
        relative_path: "flowchart/layout.rs",
        violations: &mut violations,
        in_computed_length_owner: false,
    }
    .visit_file(&syntax);

    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("operation-owned computed text length"));
}

#[test]
fn flowchart_computed_length_preserves_operation_authority() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let relative = "flowchart/layout.rs";
    let source = std::fs::read_to_string(source_root.join(relative))
        .expect("read Flowchart measurement owner");
    let syntax = syn::parse_file(&source).expect("parse Flowchart measurement owner");
    let mut violations = Vec::new();

    FlowchartMeasurementAuthorityGuard {
        relative_path: relative,
        violations: &mut violations,
        in_computed_length_owner: false,
    }
    .visit_file(&syntax);

    assert!(
        violations.is_empty(),
        "Flowchart must preserve operation-owned computed text lengths without family compensation:\n{}",
        violations.join("\n")
    );
}

#[test]
fn create_text_bbox_y_offsets_are_operation_owned_outside_the_text_fallbacks() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for path in rust_sources(&source_root) {
        let relative = path.strip_prefix(&source_root).unwrap_or(&path);
        if relative == Path::new("text.rs") || relative.starts_with("text") {
            continue;
        }
        let relative_display = relative.to_string_lossy();
        let source = std::fs::read_to_string(&path).expect("read render source");
        let syntax = syn::parse_file(&source).expect("parse render source");
        CreateTextBBoxYOffsetGuard {
            relative_path: &relative_display,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "createText bbox-y operations must route through TextMeasurer so browser hosts remain authoritative:\n{}",
        violations.join("\n")
    );
}

#[test]
fn state_svg_topology_guard_rejects_helper_id_matching() {
    let syntax = syn::parse_file(
        r#"
        fn render_segment(id: &str) -> bool {
            id.ends_with("-cyclic-special-2")
        }
        "#,
    )
    .expect("parse State SVG helper-id match");
    let mut violations = Vec::new();

    StateSvgTopologyGuard {
        relative_path: "mutation.rs",
        violations: &mut violations,
    }
    .visit_file(&syntax);

    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("internal Dagre helper id"));
}

#[test]
fn state_svg_does_not_parse_internal_dagre_helper_ids() {
    let state_svg_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/svg/parity/state");
    let mut violations = Vec::new();

    for path in rust_sources(&state_svg_root) {
        let relative = path
            .strip_prefix(&state_svg_root)
            .unwrap_or(&path)
            .to_string_lossy();
        let source = std::fs::read_to_string(&path).expect("read State SVG source");
        let syntax = syn::parse_file(&source).expect("parse State SVG source");
        StateSvgTopologyGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "State SVG must consume logical layout edges instead of parsing Dagre helper ids:\n{}",
        violations.join("\n")
    );
}

#[test]
fn state_compensation_guard_rejects_browser_bbox_and_measurement_lattices() {
    let state_layout = syn::parse_file(
        r#"
        const STATE_END_WIDTH: f64 = 14.02;
        fn quantize_host_width(width: f64) -> f64 {
            round_to_1_64_px(width)
        }
        "#,
    )
    .expect("parse State stateEnd bbox compensation");
    let state_svg = syn::parse_file(
        r#"
        fn place_placeholder(mut y: f64) {
            y += 0.025;
        }
        "#,
    )
    .expect("parse State placeholder bbox compensation");
    let mut violations = Vec::new();

    StateCompensationGuard {
        relative_path: "state/layout.rs",
        violations: &mut violations,
    }
    .visit_file(&state_layout);
    StateCompensationGuard {
        relative_path: "svg/parity/state/render.rs",
        violations: &mut violations,
    }
    .visit_file(&state_svg);

    assert_eq!(violations.len(), 3);
    assert!(violations[0].contains("stateEnd bbox constant"));
    assert!(violations[1].contains("requantizes an operation-owned text measurement"));
    assert!(violations[2].contains("placeholder geometry"));
}

#[test]
fn state_geometry_and_measurement_preserve_source_and_host_authority() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for relative in [
        "state.rs",
        "state/layout.rs",
        "svg/parity/state/node.rs",
        "svg/parity/state/render.rs",
    ] {
        let source =
            std::fs::read_to_string(source_root.join(relative)).expect("read State geometry owner");
        let syntax = syn::parse_file(&source).expect("parse State geometry owner");
        StateCompensationGuard {
            relative_path: relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "State must preserve source geometry and operation-owned text measurements without family compensation:\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_family_delegates_root_viewport_and_emission() {
    let parity_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/svg/parity");
    let root_module = parity_root.join("root_svg.rs");
    let mut violations = Vec::new();

    for path in rust_sources(&parity_root) {
        if path == root_module {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        let relative = path
            .strip_prefix(&parity_root)
            .unwrap_or(&path)
            .to_string_lossy();
        RootOwnershipGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "built-in families must delegate root sizing and emission to root_svg:\n{}",
        violations.join("\n")
    );
}

struct FixtureOverrideGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

impl<'ast> Visit<'ast> for FixtureOverrideGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let name = ident.to_string();
        let forbidden = matches!(
            name.as_str(),
            "RootViewportOverridePolicy"
                | "lookup_root_viewport_override"
                | "lookup_sequence_svg_override_em"
                | "html_overrides"
                | "svg_overrides"
        ) || (name.starts_with("lookup_")
            && name.ends_with("_root_viewport_override"));
        if forbidden {
            self.violations.push(format!(
                "{}: production rendering retains fixture or exact-text override symbol `{name}`",
                self.relative_path
            ));
        }
    }
}

#[test]
fn production_rendering_contains_no_fixture_or_exact_text_overrides() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let generated_root = source_root.join("generated");
    let mut violations = Vec::new();

    for path in rust_sources(&source_root) {
        let relative = path
            .strip_prefix(&source_root)
            .unwrap_or(&path)
            .to_string_lossy();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if path.starts_with(&generated_root)
            && (file_name.contains("_overrides_") || file_name == "root_viewports.rs")
        {
            violations.push(format!(
                "{relative}: generated fixture override module remains in production"
            ));
        }
        if relative == "text/overrides.rs" {
            violations.push(format!(
                "{relative}: exact-text override seam remains in production"
            ));
        }

        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        FixtureOverrideGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "production rendering must use computed bounds and generalized measurement facts:\n{}",
        violations.join("\n")
    );
}
