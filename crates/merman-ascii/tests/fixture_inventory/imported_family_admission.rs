use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use merman_ascii::AsciiRenderOptions;
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::sequence::SequenceMessageKind;
use merman_core::{Engine, ParseOptions};

use super::support::render_model;

const IMPORTED_FAMILY_FIXTURE_COUNTS: &[(&str, &str, usize)] = &[
    ("sequence", "sequence", 322),
    ("class", "class", 251),
    ("er", "er", 101),
];

const IMPORTED_INTENTIONAL_EMPTY_FIXTURES: &[(&str, &str)] = &[
    (
        "class/upstream_pkgtests_classdiagram_spec_004.mmd",
        "accessibility-only Class input has no terminal-visible diagram facts",
    ),
    (
        "class/upstream_pkgtests_classdiagram_spec_005.mmd",
        "multiline accessibility-only Class input has no terminal-visible diagram facts",
    ),
    (
        "er/upstream_pkgtests_diagram_orchestration_spec_030.mmd",
        "an empty ER diagram has no terminal-visible diagram facts",
    ),
];

#[test]
fn imported_common_family_fixtures_preserve_primary_typed_facts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let engine = Engine::new();
    let options = AsciiRenderOptions::ascii();
    let mut failures = Vec::new();
    let mut intentional_empty_seen = BTreeSet::new();
    assert!(
        IMPORTED_INTENTIONAL_EMPTY_FIXTURES
            .iter()
            .all(|(path, reason)| !path.is_empty() && !reason.is_empty()),
        "every intentional-empty imported fixture must name a path and reason"
    );

    for (directory, expected_kind, expected_count) in IMPORTED_FAMILY_FIXTURE_COUNTS {
        let root = workspace_root.join("fixtures").join(directory);
        let mut fixtures = fs::read_dir(&root)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
            .map(|entry| entry.expect("fixture entry must be readable").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "mmd"))
            .collect::<Vec<_>>();
        fixtures.sort();

        assert_eq!(
            fixtures.len(),
            *expected_count,
            "imported fixture count drifted for {}",
            root.display()
        );

        let mut rendered_count = 0usize;
        let mut rejected_invalid_count = 0usize;
        for path in fixtures {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("fixture name must be UTF-8");
            if *directory == "sequence" && file_name == "stress_end_keyword_016.mmd" {
                assert!(
                    engine
                        .parse_diagram_for_render_model_sync(
                            &fs::read_to_string(&path).expect("fixture must be readable"),
                            ParseOptions::strict(),
                        )
                        .is_err(),
                    "the documented upstream-invalid local stress fixture must stay excluded"
                );
                rejected_invalid_count += 1;
                continue;
            }
            let fixture_key = format!("{directory}/{file_name}");
            let expected_empty = IMPORTED_INTENTIONAL_EMPTY_FIXTURES
                .iter()
                .any(|(path, _)| *path == fixture_key);

            let result = (|| -> std::result::Result<(), String> {
                let source = fs::read_to_string(&path)
                    .map_err(|error| format!("failed to read fixture: {error}"))?;
                let parsed = engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .map_err(|error| format!("strict parse failed: {error}"))?
                    .ok_or_else(|| "diagram type was not detected".to_string())?;
                let model = parsed.model();
                if model.kind() != *expected_kind {
                    return Err(format!(
                        "expected typed family `{expected_kind}`, got `{}`",
                        model.kind()
                    ));
                }
                // This corpus gate owns primary authored-text visibility. Focused family tests own
                // topology, marker, control-frame, and field-role semantics.
                let witnesses = primary_typed_semantic_witnesses(model);
                let rendered = render_model(model, &options)
                    .map_err(|error| format!("ASCII render failed: {error}"))?;
                if expected_empty {
                    if !rendered.trim().is_empty() {
                        return Err(format!(
                            "document has terminal-visible output despite its intentional-empty disposition:\n{rendered}"
                        ));
                    }
                } else if rendered.trim().is_empty() {
                    return Err("ASCII renderer returned an empty document".to_string());
                } else {
                    assert_primary_typed_semantic_witnesses(&rendered, &witnesses)?;
                }
                Ok(())
            })();

            match result {
                Ok(()) => {
                    rendered_count += 1;
                    if expected_empty {
                        intentional_empty_seen.insert(fixture_key);
                    }
                }
                Err(error) => {
                    let relative = path.strip_prefix(&workspace_root).unwrap_or(&path);
                    failures.push(format!("{}: {error}", relative.display()));
                }
            }
        }

        let expected_invalid_count = usize::from(*directory == "sequence");
        assert_eq!(
            rejected_invalid_count,
            expected_invalid_count,
            "intentional-invalid fixture count drifted for {}",
            root.display()
        );
        assert_eq!(
            rendered_count,
            expected_count - expected_invalid_count,
            "rendered fixture count drifted for {}:\n{}",
            root.display(),
            failures.join("\n")
        );
    }

    assert!(
        failures.is_empty(),
        "imported common-family admission failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(
        intentional_empty_seen,
        IMPORTED_INTENTIONAL_EMPTY_FIXTURES
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect(),
        "every empty imported render must have one explicit metadata-only disposition"
    );
}

#[derive(Debug)]
struct SemanticWitness<'model> {
    role: &'static str,
    text: &'model str,
    tokens: Vec<String>,
}

fn primary_typed_semantic_witnesses(model: &RenderSemanticModel) -> Vec<SemanticWitness<'_>> {
    let mut witnesses = Vec::new();
    match model {
        RenderSemanticModel::Sequence(model) => {
            if let Some(title) = &model.title {
                push_semantic_witness(&mut witnesses, "sequence title", title);
            }
            for actor_id in &model.actor_order {
                let Some(actor) = model.actors.get(actor_id) else {
                    continue;
                };
                let visible = if actor.description.is_empty() {
                    actor_id
                } else {
                    actor.description.as_str()
                };
                push_semantic_witness(&mut witnesses, "sequence participant", visible);
            }
            for sequence_box in &model.boxes {
                if let Some(name) = &sequence_box.name {
                    push_semantic_witness(&mut witnesses, "sequence box", name);
                }
            }
            for note in &model.notes {
                push_semantic_witness(&mut witnesses, "sequence note", &note.message);
            }
            for message in &model.messages {
                if matches!(
                    message.semantic_kind(),
                    SequenceMessageKind::Signal | SequenceMessageKind::Note
                ) {
                    push_semantic_witness(
                        &mut witnesses,
                        "sequence message",
                        message.message_text(),
                    );
                }
            }
        }
        RenderSemanticModel::Class(model) => {
            for class in model.classes.values() {
                let visible = if class.label.is_empty() {
                    class.id.as_str()
                } else {
                    class.label.as_str()
                };
                push_semantic_witness(&mut witnesses, "class identity", visible);
                for annotation in &class.annotations {
                    push_semantic_witness(&mut witnesses, "class annotation", annotation);
                }
                for member in class.members.iter().chain(&class.methods) {
                    let visible = if member.display_text.is_empty() {
                        member.id.as_str()
                    } else {
                        member.display_text.as_str()
                    };
                    push_semantic_witness(&mut witnesses, "class member", visible);
                }
            }
            for namespace in model.namespaces.values() {
                let visible = if namespace.label.is_empty() {
                    namespace.id.rsplit('.').next().unwrap_or(&namespace.id)
                } else {
                    namespace.label.as_str()
                };
                push_semantic_witness(&mut witnesses, "class namespace", visible);
            }
            for note in &model.notes {
                push_semantic_witness(&mut witnesses, "class note", &note.text);
            }
            for relation in &model.relations {
                push_semantic_witness(&mut witnesses, "class relation", &relation.title);
                for (role, label) in [
                    ("class source endpoint label", &relation.relation_title_1),
                    ("class target endpoint label", &relation.relation_title_2),
                ] {
                    // Core projects an absent endpoint label as this sentinel.
                    if !label.eq_ignore_ascii_case("none") {
                        push_semantic_witness(&mut witnesses, role, label);
                    }
                }
            }
        }
        RenderSemanticModel::Er(model) => {
            for entity in model.entities.values() {
                let visible = if entity.alias.is_empty() {
                    entity.label.as_str()
                } else {
                    entity.alias.as_str()
                };
                push_semantic_witness(&mut witnesses, "ER entity", visible);
                for attribute in &entity.attributes {
                    push_semantic_witness(&mut witnesses, "ER attribute type", &attribute.ty);
                    push_semantic_witness(&mut witnesses, "ER attribute name", &attribute.name);
                    for key in &attribute.keys {
                        push_semantic_witness(&mut witnesses, "ER attribute key", key);
                    }
                    push_semantic_witness(
                        &mut witnesses,
                        "ER attribute comment",
                        &attribute.comment,
                    );
                }
            }
            for relationship in &model.relationships {
                push_semantic_witness(
                    &mut witnesses,
                    "ER relationship label",
                    &relationship.role_a,
                );
            }
        }
        other => panic!(
            "imported common-family witness extraction does not support `{}`",
            other.kind()
        ),
    }
    witnesses
}

fn push_semantic_witness<'model>(
    witnesses: &mut Vec<SemanticWitness<'model>>,
    role: &'static str,
    text: &'model str,
) {
    let tokens = semantic_witness_tokens(text);
    if !tokens.is_empty() {
        witnesses.push(SemanticWitness { role, text, tokens });
    }
}

fn assert_primary_typed_semantic_witnesses(
    rendered: &str,
    witnesses: &[SemanticWitness<'_>],
) -> std::result::Result<(), String> {
    let rendered_tokens = semantic_tokens(rendered);
    for witness in witnesses {
        for token in &witness.tokens {
            if visible_token_occurrences(&rendered_tokens, token) == 0 {
                return Err(format!(
                    "{} fact {:?} lost visible token {:?}:\n{rendered}",
                    witness.role, witness.text, token
                ));
            }
        }
    }
    Ok(())
}

fn semantic_witness_tokens(text: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(text.len());
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(after_break) = remaining.strip_prefix("\\n") {
            normalized.push(' ');
            remaining = after_break;
            continue;
        }
        if let Some(after_open) = remaining.strip_prefix("#lt;")
            && let Some(end) = after_open.find("#gt;")
        {
            normalized.push(' ');
            remaining = &after_open[end + "#gt;".len()..];
            continue;
        }
        if let Some(after_open) = remaining.strip_prefix('<')
            && let Some(end) = after_open.find('>')
        {
            let tag = after_open[..end].trim();
            if tag
                .as_bytes()
                .get(..2)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"br"))
                && tag[2..]
                    .chars()
                    .all(|character| character.is_ascii_whitespace() || character == '/')
            {
                normalized.push(' ');
                remaining = &after_open[end + 1..];
                continue;
            }
        }

        let character = remaining
            .chars()
            .next()
            .expect("non-empty semantic text must contain a character");
        normalized.push(character);
        remaining = &remaining[character.len_utf8()..];
    }

    semantic_tokens(&normalized)
}

fn semantic_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let flush = |token: &mut String, tokens: &mut Vec<String>| {
        if !token.is_empty() {
            tokens.push(std::mem::take(token));
        }
    };

    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            token.push(character);
        } else {
            flush(&mut token, &mut tokens);
        }
    }
    flush(&mut token, &mut tokens);
    tokens
}

fn visible_token_occurrences(tokens: &[String], expected: &str) -> usize {
    let mut occurrences = 0usize;
    for start in 0..tokens.len() {
        let mut joined = String::new();
        for token in &tokens[start..] {
            joined.push_str(token);
            if joined == expected {
                occurrences += 1;
                break;
            }
            if joined.len() >= expected.len() || !expected.starts_with(&joined) {
                break;
            }
        }
    }
    occurrences
}
