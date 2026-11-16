#![cfg(any(feature = "diagram-class", feature = "diagram-er"))]

use merman_ascii::{
    ASCII_OUTPUT_SCHEMA_VERSION, AsciiError, AsciiOutput, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy, AsciiTrimPolicy,
    AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::{
    OperationControl, ParsedSemanticDiagram, SemanticDiagramModel, runtime::RuntimePolicy,
};
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

fn parsed(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("the relation reading fixture should parse")
}

fn render(
    parsed: &ParsedSemanticDiagram,
    width: usize,
    options: AsciiRenderOptions,
    resources: AsciiResourcePolicy,
    control: &OperationControl,
) -> merman_ascii::Result<AsciiOutput> {
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?.render_parsed_report(
        parsed,
        AsciiViewportPolicy::with_max_width(width)
            .overflow(OverflowPolicy::Fallback)
            .trim(AsciiTrimPolicy::TrimTrailingSpaces),
        control,
        &operation,
        resources,
    )
}

fn fallback(
    parsed: &ParsedSemanticDiagram,
    width: usize,
    options: AsciiRenderOptions,
) -> AsciiOutput {
    let report = render(
        parsed,
        width,
        options,
        AsciiResourcePolicy::unbounded(),
        &OperationControl::new(),
    )
    .expect("the reading document should be complete and bounded");
    assert_eq!(report.schema_version, ASCII_OUTPUT_SCHEMA_VERSION);
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(report.projection, AsciiProjection::StructuredText);
    assert_eq!(report.lossiness.as_str(), "presentation_only");
    assert!(report.fallback.attempted);
    assert!(!report.trimmed, "authored trailing spaces must survive");
    assert!(report.primary_extent.width > width);
    assert_eq!(report.emitted_extent.height, report.text.lines().count());
    for line in report.text.lines() {
        let cells = match report.width_profile {
            TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(line),
            _ => UnicodeWidthStr::width(line),
        };
        assert!(cells <= width, "{cells} > {width}: {line:?}");
    }
    report
}

// Continuation indentation is renderer-owned. Quoted/structured values can themselves contain
// convincing record boundaries, so a new declaration is admitted only outside those fields.
fn record_open(record: &str) -> bool {
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0usize;
    for ch in record.chars() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else {
            match ch {
                '"' => quoted = true,
                '[' | '{' => depth += 1,
                ']' | '}' => depth = depth.checked_sub(1).expect("balanced reading records"),
                _ => {}
            }
        }
    }
    quoted || depth > 0
}

fn records(text: &str) -> Vec<String> {
    let mut records: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(continuation) = line.strip_prefix("  ") {
            records
                .last_mut()
                .expect("a preceding record")
                .push_str(continuation);
            continue;
        }
        let keyed = line.starts_with(['N', 'G', 'I', 'T', 'E'])
            && line[1..]
                .bytes()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit());
        let boundary = keyed
            || [
                "family: ",
                "direction: ",
                "title: ",
                "Keys: ",
                "Relationships ",
                "graph details:",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix));
        if records.is_empty() || (boundary && !record_open(records.last().unwrap())) {
            records.push(line.to_owned());
        } else {
            records.last_mut().unwrap().push_str(line);
        }
    }
    records
}

fn quoted_record(records: &[String], prefix: &str) -> String {
    let value = records
        .iter()
        .find_map(|record| record.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("missing {prefix:?}: {records:#?}"));
    serde_json::from_str(value)
        .unwrap_or_else(|error| panic!("invalid quoted field {value:?}: {error}"))
}

fn detail_fields(records: &[String], scope: &str) -> BTreeMap<String, Value> {
    let prefix = format!("{scope} details: {{");
    let Some(mut input) = records
        .iter()
        .find_map(|record| record.strip_prefix(&prefix))
    else {
        return BTreeMap::new();
    };
    let mut fields = BTreeMap::new();
    while input != "}" {
        let (field, rest) = input.split_once(':').expect("model-owned detail key");
        let mut values = serde_json::Deserializer::from_str(rest).into_iter::<&RawValue>();
        let raw = values
            .next()
            .unwrap()
            .expect("a complete opaque detail value");
        fields.insert(field.to_owned(), serde_json::from_str(raw.get()).unwrap());
        input = &rest[values.byte_offset()..];
        if let Some(rest) = input.strip_prefix(',') {
            input = rest;
        } else {
            assert_eq!(input, "}");
        }
    }
    fields
}

fn key_for(records: &[String], kind: &str, id: &str) -> String {
    let suffix = format!(" [{kind}]: {}", serde_json::to_string(id).unwrap());
    records
        .iter()
        .find_map(|record| record.strip_suffix(&suffix))
        .unwrap_or_else(|| panic!("missing {kind} identity {id:?}: {records:#?}"))
        .to_owned()
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_reading_keeps_independent_markers_labels_and_identity_in_each_direction() {
    for direction in ["TB", "BT", "LR", "RL"] {
        let mut parsed = parsed(&format!(
            "classDiagram\ndirection {direction}\nAccountRepositoryService \"1\" o--* \"0..*\" TransactionAuditPublisher : owns\n"
        ));
        let SemanticDiagramModel::Class(model) = &mut parsed.model else {
            unreachable!()
        };
        if direction == "RL" {
            // Detached models can carry a nonstandard vocabulary table. Preserve it and use
            // its values, rather than treating every retained integer as a parser default.
            model.constants.relation_type.aggregation = 17;
            model.constants.line_type.line = 23;
            model.relations[0].relation.type1 = 17;
            model.relations[0].relation.line_type = 23;
        }
        let report = fallback(&parsed, 24, AsciiRenderOptions::ascii());
        let rows = records(&report.text);
        let account = key_for(&rows, "class", "AccountRepositoryService");
        let audit = key_for(&rows, "class", "TransactionAuditPublisher");
        assert_eq!(account, "N1");
        assert_eq!(audit, "N2");
        assert_eq!(quoted_record(&rows, "direction: "), direction);
        assert!(
            rows.contains(&format!(
                "E1: {account} [aggregation] --solid-- {audit} [composition]"
            )),
            "{rows:#?}"
        );
        assert_eq!(quoted_record(&rows, "E1 source-label: "), "1");
        assert_eq!(quoted_record(&rows, "E1 target-label: "), "0..*");
        assert_eq!(quoted_record(&rows, "E1 label: "), "owns");
        let graph = detail_fields(&rows, "graph");
        assert!(!graph.contains_key("topology"));
        if direction == "RL" {
            assert_eq!(graph["constants"]["relationType"]["aggregation"], 17);
            assert_eq!(graph["constants"]["lineType"]["line"], 23);
        } else {
            assert!(!graph.contains_key("constants"));
        }
        assert!(!report.text.contains("model.classes."));
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_reading_retains_namespace_members_facts_and_opaque_metadata_with_their_owner() {
    let mut parsed = parsed(concat!(
        "classDiagram\ndirection LR\n",
        "namespace Domain {\n",
        "  class Account {\n    +String id\n    +save(String value) bool\n  }\n",
        "  note for Account \"retained note\"\n}\n",
        "class Audit\nAccount --> Audit : records\n",
    ));
    parsed.metadata.title = Some("Authored title [N1]: {key,value}  ".into());
    parsed.metadata.config = merman_model::MermaidConfig::from_value(json!({
        "class": {"hierarchicalNamespaces": false},
        "retained": {"empty": {}, "items": [], "value": "config: [E1] {two,fields}  "}
    }));
    let SemanticDiagramModel::Class(model) = &mut parsed.model else {
        unreachable!()
    };
    model.acc_title = Some("Accessible account API".into());
    model.acc_descr = Some("A complete retained description".into());
    let account = model.classes.get_mut("Account").unwrap();
    account.css_classes = "custom primary".into();
    account.styles = vec!["fill:#123456".into()];
    account.link = Some("https://example.com/Account?query=[one,two]".into());
    account.tooltip = Some("Tooltip: \"quoted\" {owner: N2}".into());
    account.callback = Some(serde_json::Map::from_iter([
        ("empty".into(), json!({})),
        ("payload".into(), json!({"value": "callback [E1]"})),
    ]));
    account.have_callback = true;
    account.callback_effective = true;
    account.link_target = Some(String::new());
    account.annotations = vec!["interface".into(), "retained second annotation".into()];
    account.members[0].css_style = "font-style:italic".into();
    account.members[0].parameters = "raw: [one,two]".into();
    let expected_members = account.members.clone();
    let expected_methods = account.methods.clone();
    model.classes.get_mut("Audit").unwrap().callback = Some(serde_json::Map::new());
    model.style_precedence_facts = serde_json::from_value(json!({
        "inline_paint": {"target": "Account", "styles": ["fill:#123456"], "style_event_ordinal": 7}
    }))
    .unwrap();
    let expected_style_facts = serde_json::to_value(&model.style_precedence_facts).unwrap();
    let report = fallback(&parsed, 20, AsciiRenderOptions::unicode());
    let rows = records(&report.text);
    let account_key = key_for(&rows, "class", "Account");
    let namespace = key_for(&rows, "namespace", "Domain");
    assert!(
        rows.contains(&format!("{account_key} namespace: {namespace}")),
        "{rows:#?}"
    );
    assert!(
        rows.contains(&format!("{namespace} classes: [{account_key}]")),
        "{rows:#?}"
    );
    let note = key_for(&rows, "note", "note0");
    assert!(rows.contains(&format!("{note} class: {account_key}")));
    assert_eq!(
        quoted_record(&rows, &format!("{note} text: ")),
        "retained note"
    );
    for (kind, members) in [("member", expected_members), ("method", expected_methods)] {
        assert!(!members.is_empty());
        for (index, member) in members.iter().enumerate() {
            let scope = format!("{account_key} {kind} {}", index + 1);
            assert_eq!(
                quoted_record(&rows, &format!("{scope}: ")),
                member.display_text
            );
            let fields = detail_fields(&rows, &scope);
            assert_eq!(fields["id"], member.id);
            assert!(!fields.contains_key("memberType"));
            for (field, value) in [
                ("classifier", &member.classifier),
                ("cssStyle", &member.css_style),
                ("parameters", &member.parameters),
                ("returnType", &member.return_type),
                ("visibility", &member.visibility),
            ] {
                if value.is_empty() {
                    assert!(!fields.contains_key(field));
                } else {
                    assert_eq!(&fields[field], value);
                }
            }
        }
    }
    let fields = detail_fields(&rows, &account_key);
    assert_eq!(fields["cssClasses"], "custom primary");
    assert_eq!(fields["styles"], json!(["fill:#123456"]));
    assert_eq!(fields["callback"]["empty"], json!({}));
    assert_eq!(fields["callback"]["payload"]["value"], "callback [E1]");
    assert_eq!(fields["haveCallback"], true);
    assert_eq!(fields["callbackEffective"], true);
    assert_eq!(fields["linkTarget"], "");
    assert_eq!(fields["tooltip"], "Tooltip: \"quoted\" {owner: N2}");
    assert_eq!(
        quoted_record(&rows, &format!("{account_key} annotation 2: ")),
        "retained second annotation"
    );
    let graph = detail_fields(&rows, "graph");
    assert_eq!(graph["accTitle"], "Accessible account API");
    assert_eq!(graph["accDescr"], "A complete retained description");
    assert_eq!(&graph["sourceConfig"], parsed.metadata.config.as_value());
    assert!(!graph.contains_key("topology"));
    assert!(!graph.contains_key("constants"));
    assert_eq!(graph["stylePrecedenceFacts"], expected_style_facts);
    let audit = key_for(&rows, "class", "Audit");
    assert_eq!(detail_fields(&rows, &audit)["callback"], json!({}));
    assert_eq!(
        quoted_record(&rows, "title: "),
        parsed.metadata.title.as_deref().unwrap()
    );
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_interface_markers_select_the_interface_identity_domain_when_ids_collide() {
    use merman_model::families::class::{ClassInterface, REL_LOLLIPOP};
    let mut parsed = parsed("classDiagram\ndirection LR\nA --> B : first\nA --> B : second\n");
    let SemanticDiagramModel::Class(model) = &mut parsed.model else {
        unreachable!()
    };
    model.interfaces.push(ClassInterface {
        id: "A".into(),
        label: "Shared identity".into(),
        class_id: "B".into(),
    });
    model.relations[0].relation.type1 = REL_LOLLIPOP;
    let report = fallback(&parsed, 8, AsciiRenderOptions::ascii());
    let rows = records(&report.text);
    let class = key_for(&rows, "class", "A");
    let interface = key_for(&rows, "interface", "A");
    let target = key_for(&rows, "class", "B");
    assert_ne!(class, interface);
    assert!(
        rows.contains(&format!(
            "E1: {interface} [lollipop] --solid-- {target} [arrow]"
        )),
        "{rows:#?}"
    );
    assert!(
        rows.contains(&format!("E2: {class} [none] --solid-- {target} [arrow]")),
        "{rows:#?}"
    );
    assert_eq!(
        quoted_record(&rows, &format!("{interface} label: ")),
        "Shared identity"
    );
}

#[cfg(feature = "diagram-class")]
#[test]
fn qualified_class_references_keep_the_real_namespace_owner_and_authored_spelling() {
    use merman_model::families::class::ClassInterface;
    let mut parsed = parsed(
        "classDiagram\nnamespace Domain {\n class Service\n}\nclass Client\nClient --> Domain.Service\nnote for Service \"a fact\"\n",
    );
    let SemanticDiagramModel::Class(model) = &mut parsed.model else {
        unreachable!()
    };
    assert_eq!(model.namespace_facade_aliases["Domain.Service"], "Service");
    model.interfaces.push(ClassInterface {
        id: "Api".into(),
        label: "API".into(),
        class_id: "Domain.Service".into(),
    });
    model.notes[0].class_id = Some("Domain.Service".into());
    let report = fallback(&parsed, 24, AsciiRenderOptions::ascii());
    let rows = records(&report.text);
    let client = key_for(&rows, "class", "Client");
    let service = key_for(&rows, "class", "Service");
    assert!(
        rows.contains(&format!("E1: {client} [none] --solid-- {service} [arrow]")),
        "{rows:#?}"
    );
    assert_eq!(quoted_record(&rows, "E1 target-ref: "), "Domain.Service");
    assert!(rows.contains(&format!("I1 class: {service}")), "{rows:#?}");
    assert_eq!(quoted_record(&rows, "I1 class-ref: "), "Domain.Service");
    assert!(rows.contains(&format!("T1 class: {service}")), "{rows:#?}");
    assert_eq!(quoted_record(&rows, "T1 class-ref: "), "Domain.Service");
    let facade = key_for(&rows, "class alias", "Domain.Service");
    assert!(
        rows.contains(&format!("{facade} alias-of: {service}")),
        "{rows:#?}"
    );
}

#[cfg(feature = "diagram-class")]
#[test]
fn duplicate_interface_references_follow_the_primary_topology() {
    use merman_model::families::class::{ClassInterface, REL_LOLLIPOP};
    // A repeated ordinary ID selects the last remap. An ID equal to the second declaration's
    // generated ID leaves the earlier remap in force. The reading document must follow both.
    for (authored_id, selected) in [("X", "I2"), ("interface1", "I1")] {
        let mut parsed = parsed("classDiagram\ndirection LR\nA --> B : calls\n");
        let SemanticDiagramModel::Class(model) = &mut parsed.model else {
            unreachable!()
        };
        for label in ["First", "Second"] {
            model.interfaces.push(ClassInterface {
                id: authored_id.into(),
                label: label.into(),
                class_id: "B".into(),
            });
        }
        model.relations[0].id1 = authored_id.into();
        model.relations[0].relation.type1 = REL_LOLLIPOP;
        let report = fallback(&parsed, 24, AsciiRenderOptions::ascii());
        let rows = records(&report.text);
        let target = key_for(&rows, "class", "B");
        assert_eq!(quoted_record(&rows, "I1 label: "), "First");
        assert_eq!(quoted_record(&rows, "I2 label: "), "Second");
        assert!(
            rows.contains(&format!(
                "E1: {selected} [lollipop] --solid-- {target} [arrow]"
            )),
            "{rows:#?}"
        );
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn interface_references_retain_authored_and_direct_generated_id_spellings() {
    use merman_model::families::class::{ClassInterface, REL_LOLLIPOP};
    for reference in ["X", "interface0"] {
        let mut parsed = parsed("classDiagram\ndirection LR\nA --> B : calls\n");
        let SemanticDiagramModel::Class(model) = &mut parsed.model else {
            unreachable!()
        };
        model.interfaces.push(ClassInterface {
            id: "X".into(),
            label: "API".into(),
            class_id: "B".into(),
        });
        model.relations[0].id1 = reference.into();
        model.relations[0].relation.type1 = REL_LOLLIPOP;
        let report = fallback(&parsed, 24, AsciiRenderOptions::ascii());
        let rows = records(&report.text);
        let target = key_for(&rows, "class", "B");
        assert!(
            rows.contains(&format!("E1: I1 [lollipop] --solid-- {target} [arrow]")),
            "{rows:#?}"
        );
        assert_eq!(quoted_record(&rows, "E1 source-ref: "), reference);
    }
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_reading_resolves_generated_ids_and_attaches_each_cardinality_to_its_actual_endpoint() {
    let parsed = parsed(concat!(
        "erDiagram\ndirection LR\n",
        "CUSTOMER ||..o{ ORDER : places\n",
        "ORDER }|--o| LINE_ITEM : contains\n",
    ));
    let report = fallback(&parsed, 24, AsciiRenderOptions::ascii());
    let rows = records(&report.text);
    let customer = key_for(&rows, "entity", "CUSTOMER");
    let order = key_for(&rows, "entity", "ORDER");
    let line = key_for(&rows, "entity", "LINE_ITEM");
    assert!(
        rows.contains(&format!("E1: {customer} [1] --dotted-- {order} [0..*]")),
        "{rows:#?}"
    );
    assert!(
        rows.contains(&format!("E2: {order} [1..*] --solid-- {line} [0..1]")),
        "{rows:#?}"
    );
    assert_eq!(quoted_record(&rows, "E1 label: "), "places");
    assert_eq!(quoted_record(&rows, "E2 label: "), "contains");
    let SemanticDiagramModel::Er(model) = &parsed.model else {
        unreachable!()
    };
    for authored in model.entities.keys() {
        let key = key_for(&rows, "entity", authored);
        assert!(!detail_fields(&rows, &key).contains_key("id"));
    }
    assert!(!report.text.contains("model.entities."));
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_reading_retains_attribute_keys_comments_aliases_styles_and_source_metadata() {
    let mut parsed = parsed(concat!(
        "erDiagram\ndirection LR\n",
        "ACCOUNT {\n  string id PK\n  string owner FK \"owner id\"\n}\n",
        "ACCOUNT ||--o{ AUDIT : records\n",
    ));
    parsed.metadata.title = Some("Attributes: [all, fields]".into());
    parsed.metadata.config = merman_model::MermaidConfig::from_value(json!({
        "retained": {"empty": {}, "items": [], "message": "metadata {N1: E2}"}
    }));
    let SemanticDiagramModel::Er(model) = &mut parsed.model else {
        unreachable!()
    };
    model.acc_title = Some("Accessible entities".into());
    let account = model.entities.get_mut("ACCOUNT").unwrap();
    account.alias = "Same [entity]".into();
    account.css_styles = vec!["stroke:#123456".into()];
    account.attributes[0].keys = vec!["PK".into(), "FK".into(), "UK".into()];
    account.attributes[0].comment = "all retained keys, not a legend".into();
    let attributes = account.attributes.clone();
    account.id = "custom wire [account]".into();
    model.relationships[0].entity_a = "custom wire [account]".into();
    model.entities.get_mut("AUDIT").unwrap().alias = "Same [entity]".into();
    let report = fallback(&parsed, 20, AsciiRenderOptions::unicode());
    let rows = records(&report.text);
    let account = key_for(&rows, "entity", "ACCOUNT");
    let audit = key_for(&rows, "entity", "AUDIT");
    assert_ne!(account, audit);
    for key in [&account, &audit] {
        assert_eq!(
            quoted_record(&rows, &format!("{key} alias: ")),
            "Same [entity]"
        );
    }
    for (index, attribute) in attributes.iter().enumerate() {
        let scope = format!("{account} attribute {}", index + 1);
        assert!(
            rows.contains(&format!(
                "{scope}: {} {}",
                json!(attribute.ty),
                json!(attribute.name)
            )),
            "{rows:#?}"
        );
        let fields = detail_fields(&rows, &scope);
        assert_eq!(fields["keys"], json!(attribute.keys));
        if attribute.comment.is_empty() {
            assert!(!fields.contains_key("comment"));
        } else {
            assert_eq!(fields["comment"], attribute.comment);
        }
    }
    assert_eq!(
        detail_fields(&rows, &account)["cssStyles"],
        json!(["stroke:#123456"])
    );
    assert_eq!(
        detail_fields(&rows, &account)["id"],
        "custom wire [account]"
    );
    let graph = detail_fields(&rows, "graph");
    assert_eq!(&graph["sourceConfig"], parsed.metadata.config.as_value());
    assert_eq!(graph["accTitle"], "Accessible entities");
    assert_eq!(
        quoted_record(&rows, "title: "),
        parsed.metadata.title.as_deref().unwrap()
    );
}

#[test]
fn long_cjk_and_delimiter_rich_retained_fields_survive_wrapping_without_rebinding_endpoints() {
    let label = "客户  [N2]: {E1, \"value\"} \\ path\n第二行  ";
    let relation_label = "关联\" [E2]: {source: N9, target: G8} \\  ";
    let mut fixtures = Vec::new();
    #[cfg(feature = "diagram-class")]
    {
        let mut parsed = parsed("classDiagram\ndirection LR\nA --> B : owns\n");
        let SemanticDiagramModel::Class(model) = &mut parsed.model else {
            unreachable!()
        };
        model.classes.get_mut("A").unwrap().text = label.into();
        model.classes.get_mut("B").unwrap().text = label.into();
        model.relations[0].title = relation_label.into();
        model.relations[0].relation_title_1 = Some("none".into());
        model.relations[0].relation_title_2 = Some("* [N1]".into());
        fixtures.push(parsed);
    }
    #[cfg(feature = "diagram-er")]
    {
        let mut parsed = parsed("erDiagram\ndirection LR\nA ||--o{ B : owns\n");
        let SemanticDiagramModel::Er(model) = &mut parsed.model else {
            unreachable!()
        };
        model.entities.get_mut("A").unwrap().alias = label.into();
        model.entities.get_mut("B").unwrap().alias = label.into();
        model.relationships[0].role_a = relation_label.into();
        fixtures.push(parsed);
    }
    for parsed in fixtures {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let report = fallback(&parsed, 24, options.with_terminal_width_profile(profile));
                let rows = records(&report.text);
                let field = match &parsed.model {
                    #[cfg(feature = "diagram-class")]
                    SemanticDiagramModel::Class(_) => "label",
                    _ => "alias",
                };
                assert_eq!(quoted_record(&rows, &format!("N1 {field}: ")), label);
                assert_eq!(quoted_record(&rows, &format!("N2 {field}: ")), label);
                assert_eq!(quoted_record(&rows, "E1 label: "), relation_label);
                assert_eq!(rows.iter().filter(|row| row.starts_with("E1: ")).count(), 1);
                assert!(!rows.iter().any(|row| row.starts_with("E2: ")));
                if field == "label" {
                    assert_eq!(quoted_record(&rows, "E1 source-label: "), "none");
                    assert_eq!(quoted_record(&rows, "E1 target-label: "), "* [N1]");
                }
            }
        }
    }
}

fn small_fixtures() -> Vec<ParsedSemanticDiagram> {
    vec![
        #[cfg(feature = "diagram-class")]
        parsed("classDiagram\ndirection LR\nA --> B : owns\n"),
        #[cfg(feature = "diagram-er")]
        parsed("erDiagram\ndirection LR\nA ||--o{ B : owns\n"),
    ]
}

#[test]
fn reading_fallback_preserves_tiny_viewports_and_exact_document_and_output_boundaries() {
    for mut parsed in small_fixtures() {
        for width in [1, 2, 4] {
            fallback(&parsed, width, AsciiRenderOptions::ascii());
        }
        // Make the emitted-byte boundary dominate the admitted serialization as well. The
        // retained config payload wraps once per cell without relying on serializer internals.
        parsed.metadata.config = merman_model::MermaidConfig::from_value(json!({
            "retained": "budgeted source data ".repeat(128)
        }));
        let width = 1;
        let baseline = fallback(&parsed, width, AsciiRenderOptions::ascii());
        let cells: usize = baseline.text.lines().map(UnicodeWidthStr::width).sum();
        for (id, exact) in [
            (AsciiResourceLimitId::MaxDocumentCells, cells),
            (AsciiResourceLimitId::MaxOutputBytes, baseline.text.len()),
        ] {
            let resources = AsciiResourcePolicy::unbounded()
                .with_limit(id, exact)
                .unwrap();
            let report = render(
                &parsed,
                width,
                AsciiRenderOptions::ascii(),
                resources,
                &OperationControl::new(),
            )
            .expect("the exact measured boundary should succeed");
            assert_eq!(report.text, baseline.text);
            let resources = AsciiResourcePolicy::unbounded()
                .with_limit(id, exact - 1)
                .unwrap();
            let error = render(
                &parsed,
                width,
                AsciiRenderOptions::ascii(),
                resources,
                &OperationControl::new(),
            )
            .expect_err("a complete document must fail rather than truncate at N-1");
            assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
                if details.limit == id && details.actual == exact && details.max == exact - 1));
        }
    }
}

#[test]
fn reading_fallback_observes_late_cancellation_instead_of_returning_a_partial_document() {
    for parsed in small_fixtures() {
        let baseline = fallback(&parsed, 8, AsciiRenderOptions::ascii());
        let attempt = |checkpoints| {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(checkpoints);
            render(
                &parsed,
                8,
                AsciiRenderOptions::ascii(),
                AsciiResourcePolicy::unbounded(),
                &control,
            )
        };
        let mut rejected = 0;
        let mut accepted = 1usize;
        loop {
            match attempt(accepted) {
                Ok(_) => break,
                Err(AsciiError::Cancelled(_)) => {
                    rejected = accepted;
                    accepted = accepted.checked_mul(2).unwrap();
                }
                Err(error) => panic!("unexpected cancellation probe error: {error}"),
            }
        }
        while accepted - rejected > 1 {
            let middle = rejected + (accepted - rejected) / 2;
            match attempt(middle) {
                Ok(_) => accepted = middle,
                Err(AsciiError::Cancelled(_)) => rejected = middle,
                Err(error) => panic!("unexpected cancellation probe error: {error}"),
            }
        }
        assert!(
            matches!(attempt(accepted - 1), Err(AsciiError::Cancelled(cancelled))
            if cancelled.phase == merman_model::OperationPhase::Emit)
        );
        assert_eq!(attempt(accepted).unwrap().text, baseline.text);
    }
}
