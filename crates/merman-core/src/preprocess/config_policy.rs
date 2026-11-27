//! Core policy for decoded frontmatter and directives.

mod grid;
mod sanitize;

use super::PreprocessedSource;
#[cfg(feature = "editor-facts")]
use super::rewrite_order::FrontmatterInsertOrder;
#[cfg(feature = "editor-facts")]
use super::source_config::{SourceAppearanceCapture, SourceAppearanceOwner};
#[cfg(feature = "editor-facts")]
use super::{
    FrontmatterSourceEvidence, LocalConfigKeyEvidence, SourceConfigCaptureMode,
    SourceConfigEvidence, SourceConfigKeyEvidence, SourceConfigOrigin, SourceDirectiveEvidence,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, Result, SourceSpan,
    diagram::CapturedPanic,
};
pub(super) use grid::GridSource;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
pub(crate) use grid::validate_configured_grid;
pub(crate) use grid::validate_grid_for_parser;
pub(super) use grid::{merge_source_grid, sanitize_section as sanitize_grid_section};
#[cfg(feature = "editor-facts")]
use merman_core_preprocess::directives::decode_directive_with_evidence_controlled;
use merman_core_preprocess::directives::{
    DirectiveBlock, decode_directive_controlled, remove_directive_blocks_controlled,
};
#[cfg(feature = "editor-facts")]
use merman_core_preprocess::frontmatter::decode_frontmatter_with_evidence_controlled;
use merman_core_preprocess::frontmatter::{
    FrontmatterByteSpan, decode_frontmatter_controlled, frontmatter_config_routes,
};
use merman_core_preprocess::yaml::GridPlacementsSource;
#[cfg(feature = "editor-facts")]
use merman_core_preprocess::yaml::YamlConfigKeyEvidence;
#[cfg(test)]
pub(super) use sanitize::sanitize_directive;
#[cfg(feature = "editor-facts")]
pub(super) use sanitize::sanitize_directive_controlled;
#[cfg(not(feature = "editor-facts"))]
use sanitize::sanitize_directive_controlled;
pub(super) use sanitize::sanitize_grid_config_controlled;
use serde_json::Value;

pub(super) type DiagramTypeDetector<'a> =
    dyn Fn(&str, &OperationControl) -> OperationControlResult<Result<&'static str>> + 'a;

#[derive(Clone, Copy)]
pub(super) enum GridInitDetection {
    KnownType,
    Detect { frontmatter_has_placements: bool },
}

#[cfg(feature = "editor-facts")]
pub(super) fn append_directive_evidence(
    source: &PreprocessedSource,
    captured: &ProcessedDirectives,
    evidence: &mut SourceConfigEvidence,
    control: &OperationControl,
) -> OperationControlResult<()> {
    for (index, directive) in captured.evidence.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let Some(full_span) = source.try_map_span(directive.full_span) else {
            evidence.mark_rewrite_unsafe();
            continue;
        };
        let Some(keyword_span) = source.try_map_span(directive.keyword_span) else {
            evidence.mark_rewrite_unsafe();
            continue;
        };
        let directive_index = evidence.push_directive(SourceDirectiveEvidence::new(
            directive.keyword.clone(),
            full_span,
            keyword_span,
            evidence.directives().len(),
            directive.complete,
            directive.rewrite_safe,
        ));
        for key in &directive.keys {
            let Some(local_span) = key.span else {
                continue;
            };
            let exact_span = source.try_map_span(local_span);
            let Some(span) = exact_span.or_else(|| source.try_map_enclosing_span(local_span))
            else {
                continue;
            };
            evidence.push_key(SourceConfigKeyEvidence::new(
                SourceConfigOrigin::Directive { directive_index },
                key.path.clone(),
                span,
                evidence.keys().len(),
                key.rewrite_safe && exact_span.is_some(),
            ));
        }
    }
    evidence.set_legacy_init_namespace(captured.legacy_init_namespace);
    if captured.recovered_incomplete_directive {
        evidence.mark_rewrite_unsafe();
    }
    control.checkpoint()
}

pub(super) struct ProcessedDirectives {
    pub(super) config: MermaidConfig,
    pub(super) grid_source: GridSource,
    pub(super) removals: Vec<std::ops::Range<usize>>,
    #[cfg(feature = "editor-facts")]
    pub(super) editor_prefixes: Vec<String>,
    #[cfg(feature = "editor-facts")]
    pub(super) recovered_incomplete_directive: bool,
    #[cfg(feature = "editor-facts")]
    evidence: Vec<LocalDirectiveEvidence>,
    #[cfg(feature = "editor-facts")]
    legacy_init_namespace: Option<&'static str>,
    pub(super) error: Option<Error>,
    pub(super) panic: Option<CapturedPanic>,
    #[cfg(feature = "editor-facts")]
    pub(super) appearance_capture: SourceAppearanceCapture,
    #[cfg(feature = "editor-facts")]
    pub(super) appearance_complete: bool,
}

#[cfg(feature = "editor-facts")]
struct LocalDirectiveEvidence {
    keyword: String,
    full_span: SourceSpan,
    keyword_span: SourceSpan,
    complete: bool,
    rewrite_safe: bool,
    keys: Vec<LocalConfigKeyEvidence>,
}

pub(super) fn process_directives_controlled(
    source: &PreprocessedSource,
    detector: &DiagramTypeDetector<'_>,
    blocks: Vec<DirectiveBlock>,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    evaluate_config: bool,
    grid_detection: GridInitDetection,
    control: &OperationControl,
) -> OperationControlResult<ProcessedDirectives> {
    control.checkpoint()?;
    let input = source.text();
    #[cfg(feature = "editor-facts")]
    let mut appearance_capture = SourceAppearanceCapture::default();
    #[cfg(feature = "editor-facts")]
    let mut legacy_init_namespace = None;
    #[cfg(feature = "editor-facts")]
    let mut appearance_complete = true;
    if blocks.is_empty() {
        return Ok(ProcessedDirectives {
            config: MermaidConfig::empty_object(),
            grid_source: GridSource::default(),
            removals: Vec::new(),
            #[cfg(feature = "editor-facts")]
            editor_prefixes: Vec::new(),
            #[cfg(feature = "editor-facts")]
            recovered_incomplete_directive: false,
            #[cfg(feature = "editor-facts")]
            evidence: Vec::new(),
            #[cfg(feature = "editor-facts")]
            legacy_init_namespace: None,
            error: None,
            panic: None,
            #[cfg(feature = "editor-facts")]
            appearance_capture,
            #[cfg(feature = "editor-facts")]
            appearance_complete,
        });
    }
    #[cfg(feature = "editor-facts")]
    let recovered_incomplete_directive = blocks.iter().any(|block| !block.complete);
    let mut directives = Vec::new();
    #[cfg(feature = "editor-facts")]
    let mut evidence = Vec::new();
    let mut first_error = None;
    for (index, block) in blocks.iter().enumerate() {
        if index % 32 == 0 {
            control.checkpoint()?;
        }
        if !block.complete {
            #[cfg(feature = "editor-facts")]
            if !capture_mode.collects() {
                continue;
            }
            #[cfg(not(feature = "editor-facts"))]
            continue;
        }
        let raw = &input[block.raw.clone()];
        #[cfg(not(feature = "editor-facts"))]
        let parsed = decode_directive_controlled(raw, control)?;
        #[cfg(feature = "editor-facts")]
        let parsed = if capture_mode.collects() {
            decode_directive_with_evidence_controlled(raw, block.complete, control)?
        } else {
            decode_directive_controlled(raw, control)?
        };
        if let Some(directive) = parsed.directive {
            #[cfg(feature = "editor-facts")]
            if matches!(directive.ty.as_str(), "init" | "initialize") {
                appearance_complete &= directive.arguments_complete;
            }
            #[cfg(feature = "editor-facts")]
            let appearance_owner = if capture_mode.collects() {
                let owner = SourceAppearanceOwner {
                    origin: SourceConfigOrigin::Directive {
                        directive_index: evidence.len(),
                    },
                    construct_span: SourceSpan::new(block.range.start, block.range.end),
                };
                let raw_start = block.raw.start;
                let keyword_span = SourceSpan::new(
                    raw_start.saturating_add(directive.keyword_span.start),
                    raw_start.saturating_add(directive.keyword_span.end),
                );
                let keys = directive
                    .config_keys
                    .iter()
                    .map(|key| LocalConfigKeyEvidence {
                        path: key.path.clone(),
                        span: key.span.as_ref().map(|span| {
                            SourceSpan::new(
                                raw_start.saturating_add(span.start),
                                raw_start.saturating_add(span.end),
                            )
                        }),
                        rewrite_safe: key.rewrite_safe,
                    })
                    .collect();
                evidence.push(LocalDirectiveEvidence {
                    keyword: directive.ty.clone(),
                    full_span: SourceSpan::new(block.range.start, block.range.end),
                    keyword_span,
                    complete: block.complete,
                    rewrite_safe: block.complete && directive.rewrite_safe,
                    keys,
                });
                Some(owner)
            } else {
                None
            };
            if block.complete && parsed.error.is_none() {
                directives.push(Directive {
                    ty: directive.ty,
                    args: directive.args,
                    grid_source: GridSource::capture(
                        source,
                        directive.grid_placements,
                        block.raw.start,
                        SourceSpan::new(block.range.start, block.range.end),
                        control,
                    )?,
                    #[cfg(feature = "editor-facts")]
                    appearance_owner,
                });
            }
        }
        if block.complete && first_error.is_none() {
            first_error = parsed
                .error
                .map(|message| Error::InvalidDirectiveJson { message });
        }
        if first_error.is_some() {
            #[cfg(feature = "editor-facts")]
            if capture_mode.collects() {
                continue;
            }
            return Ok(ProcessedDirectives {
                config: MermaidConfig::empty_object(),
                grid_source: GridSource::default(),
                removals: Vec::new(),
                #[cfg(feature = "editor-facts")]
                editor_prefixes: Vec::new(),
                #[cfg(feature = "editor-facts")]
                recovered_incomplete_directive,
                #[cfg(feature = "editor-facts")]
                evidence,
                #[cfg(feature = "editor-facts")]
                legacy_init_namespace: None,
                error: first_error,
                panic: None,
                #[cfg(feature = "editor-facts")]
                appearance_capture,
                #[cfg(feature = "editor-facts")]
                appearance_complete: false,
            });
        }
    }
    control.checkpoint()?;
    let input_without_directives = remove_directive_blocks_controlled(input, &blocks, control)?;
    let wrap = directives.iter().any(|d| d.ty == "wrap");
    #[cfg(feature = "editor-facts")]
    let mut editor_prefixes = Vec::new();
    #[cfg(feature = "editor-facts")]
    if capture_mode.collects_editor() {
        for (index, directive) in directives.iter().enumerate() {
            if index % 32 == 0 {
                control.checkpoint()?;
            }
            if matches!(directive.ty.as_str(), "init" | "initialize" | "wrap")
                && !editor_prefixes.contains(&directive.ty)
            {
                editor_prefixes.push(directive.ty.clone());
            }
        }
    }

    let detected_init = if !evaluate_config || first_error.is_some() {
        Ok((MermaidConfig::empty_object(), GridSource::default()))
    } else {
        let detect_init = || {
            detect_init_controlled(
                directives,
                input_without_directives.as_ref(),
                detector,
                grid_detection,
                #[cfg(feature = "editor-facts")]
                &mut appearance_capture,
                #[cfg(feature = "editor-facts")]
                &mut legacy_init_namespace,
                control,
            )
        };
        #[cfg(feature = "editor-facts")]
        {
            if capture_mode.collects() {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(detect_init)) {
                    Ok(result) => result?,
                    Err(payload) => {
                        return Ok(ProcessedDirectives {
                            config: MermaidConfig::empty_object(),
                            grid_source: GridSource::default(),
                            removals: blocks.into_iter().map(|block| block.range).collect(),
                            #[cfg(feature = "editor-facts")]
                            editor_prefixes,
                            #[cfg(feature = "editor-facts")]
                            recovered_incomplete_directive,
                            #[cfg(feature = "editor-facts")]
                            evidence,
                            #[cfg(feature = "editor-facts")]
                            legacy_init_namespace: None,
                            error: first_error,
                            panic: Some(CapturedPanic::from_payload(payload)),
                            #[cfg(feature = "editor-facts")]
                            appearance_capture,
                            #[cfg(feature = "editor-facts")]
                            appearance_complete: false,
                        });
                    }
                }
            } else {
                detect_init()?
            }
        }
        #[cfg(not(feature = "editor-facts"))]
        {
            detect_init()?
        }
    };
    let (mut merged, grid_source) = match detected_init {
        Ok(init) => init,
        Err(error) => {
            if first_error.is_none() {
                first_error = Some(error);
            }
            (MermaidConfig::empty_object(), GridSource::default())
        }
    };
    if wrap {
        merged.set_value("wrap", Value::Bool(true));
    }

    control.checkpoint()?;
    Ok(ProcessedDirectives {
        config: merged,
        grid_source,
        removals: blocks.into_iter().map(|block| block.range).collect(),
        #[cfg(feature = "editor-facts")]
        editor_prefixes,
        #[cfg(feature = "editor-facts")]
        recovered_incomplete_directive,
        #[cfg(feature = "editor-facts")]
        evidence,
        #[cfg(feature = "editor-facts")]
        legacy_init_namespace,
        error: first_error,
        panic: None,
        #[cfg(feature = "editor-facts")]
        appearance_capture,
        #[cfg(feature = "editor-facts")]
        appearance_complete,
    })
}

fn detect_init_controlled(
    directives: Vec<Directive>,
    input: &str,
    detector: &DiagramTypeDetector<'_>,
    grid_detection: GridInitDetection,
    #[cfg(feature = "editor-facts")] appearance_capture: &mut SourceAppearanceCapture,
    #[cfg(feature = "editor-facts")] legacy_init_namespace: &mut Option<&'static str>,
    control: &OperationControl,
) -> OperationControlResult<Result<(MermaidConfig, GridSource)>> {
    control.checkpoint()?;
    let mut merged = MermaidConfig::empty_object();
    let mut grid_source = GridSource::default();
    let multiple_inits = directives
        .iter()
        .filter(|directive| matches!(directive.ty.as_str(), "init" | "initialize"))
        .take(2)
        .count()
        > 1;
    let detect_grid = matches!(grid_detection, GridInitDetection::Detect { .. });
    let mut has_grid_placements = matches!(
        grid_detection,
        GridInitDetection::Detect {
            frontmatter_has_placements: true
        }
    );

    for (index, d) in directives.into_iter().enumerate() {
        if index % 16 == 0 {
            control.checkpoint()?;
        }
        if d.ty != "init" && d.ty != "initialize" {
            continue;
        }

        let has_arguments = d.args.as_ref().is_some_and(|args| !args.is_null());
        let mut args = d.args.unwrap_or_else(|| Value::Object(Default::default()));
        has_grid_placements |= args.pointer("/grid/placements").is_some();
        let remaps_legacy = !multiple_inits && args.get("config").is_some();
        let detect_for_grid = detect_grid && has_grid_placements && has_arguments;
        let diagram_type = if !multiple_inits && (remaps_legacy || detect_for_grid) {
            match detector(input, control)? {
                Ok(ty) => Some(ty),
                Err(error) if detect_for_grid => return Ok(Err(error)),
                Err(_) => None,
            }
        } else {
            None
        };
        // Mermaid sanitizes multiple init arguments before merging, so their legacy `config`
        // key is filtered out. A single init remaps that key before source sanitization.
        if !multiple_inits
            && let Some(diagram_specific_value) = args
                .as_object_mut()
                .and_then(|object| object.remove("config"))
        {
            // Legacy init uses the source detector ID, even when a parser type was supplied.
            // Mermaid only remaps flowchart-v2; other IDs still pass through source admission.
            if let Some(ty) = diagram_type {
                let key = if ty == "flowchart-v2" {
                    "flowchart"
                } else {
                    ty
                };
                if let Value::Object(obj) = &mut args {
                    if let Some(old) = obj.insert(key.to_string(), diagram_specific_value) {
                        crate::json_value::drop_value_nonrecursive(old);
                    }
                    #[cfg(feature = "editor-facts")]
                    {
                        *legacy_init_namespace = Some(key);
                    }
                }
            } else {
                crate::json_value::drop_value_nonrecursive(diagram_specific_value);
            }
        }
        // A single init's Grid section is sanitized after its merge with frontmatter. Keep
        // its explicit nulls through that merge, where they remove old option values.
        let single_grid = if multiple_inits {
            None
        } else {
            args.as_object_mut().and_then(|args| args.remove("grid"))
        };
        sanitize_directive_controlled(&mut args, control)?;
        if multiple_inits {
            if let Err(error) = sanitize_grid_section(&mut args, &d.grid_source, control)? {
                return Ok(Err(error));
            }
            grid_source.replace_init_entries(&args, d.grid_source, control)?;
        } else {
            grid_source = d.grid_source;
        }
        crate::config::mirror_legacy_font_family_into_theme_variables_value(&mut args);

        #[cfg(feature = "editor-facts")]
        merged.merge_with_mutation_observer(&args, &mut |path| {
            if let Some(owner) = d.appearance_owner {
                appearance_capture.record(path, owner);
            }
        });
        #[cfg(not(feature = "editor-facts"))]
        merged.deep_merge(&args);
        if let Some(grid) = single_grid {
            merged.set_value("grid", grid);
        }
    }

    control.checkpoint()?;
    if multiple_inits
        && detect_grid
        && has_grid_placements
        && let Err(error) = detector(input, control)?
    {
        return Ok(Err(error));
    }
    Ok(Ok((merged, grid_source)))
}

struct Directive {
    ty: String,
    args: Option<Value>,
    grid_source: GridSource,
    #[cfg(feature = "editor-facts")]
    appearance_owner: Option<SourceAppearanceOwner>,
}

#[cfg(feature = "editor-facts")]
pub(super) fn append_frontmatter_evidence(
    source: &PreprocessedSource,
    captured: &FrontmatterCapture,
    evidence: &mut SourceConfigEvidence,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let frontmatter = captured
        .full
        .zip(captured.body)
        .and_then(|(full, local_body)| {
            let full = source.try_map_span(SourceSpan::new(full.start, full.end))?;
            let body = source.try_map_span(SourceSpan::new(local_body.start, local_body.end))?;
            let insert = SourceSpan::new(body.end, body.end);
            Some(FrontmatterSourceEvidence::new(
                full,
                body,
                captured.indent.clone(),
                captured.has_config,
                captured.rewrite_safe,
                captured.fields.clone(),
                captured.insert_order.clone(),
            ))
            .map(|frontmatter| (frontmatter, insert))
        });
    if let Some((frontmatter, insert)) = frontmatter {
        evidence.set_config_insert_span(insert);
        evidence.set_frontmatter(frontmatter);
    } else if captured.full.is_some() {
        evidence.mark_rewrite_unsafe();
    }

    for (index, key) in captured.keys.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let Some(local_span) = &key.span else {
            continue;
        };
        let local_span = SourceSpan::new(local_span.start, local_span.end);
        let exact_span = source.try_map_span(local_span);
        let Some(span) = exact_span.or_else(|| source.try_map_enclosing_span(local_span)) else {
            continue;
        };
        evidence.push_key(SourceConfigKeyEvidence::new(
            SourceConfigOrigin::Frontmatter,
            key.path.clone(),
            span,
            evidence.keys().len(),
            key.rewrite_safe && exact_span.is_some(),
        ));
    }
    control.checkpoint()
}

pub(super) struct ProcessedFrontmatter {
    pub(super) title: Option<String>,
    pub(super) config: MermaidConfig,
    pub(super) grid_placements: Option<GridPlacementsSource>,
}

pub(super) struct FrontmatterCapture {
    pub(super) result: Result<ProcessedFrontmatter>,
    pub(super) full: Option<FrontmatterByteSpan>,
    #[cfg(feature = "editor-facts")]
    body: Option<FrontmatterByteSpan>,
    #[cfg(feature = "editor-facts")]
    indent: String,
    #[cfg(feature = "editor-facts")]
    pub(super) keys: Vec<YamlConfigKeyEvidence>,
    #[cfg(feature = "editor-facts")]
    has_config: bool,
    #[cfg(feature = "editor-facts")]
    rewrite_safe: bool,
    #[cfg(feature = "editor-facts")]
    fields: Option<MermaidConfig>,
    #[cfg(feature = "editor-facts")]
    insert_order: Option<FrontmatterInsertOrder>,
}

pub(super) fn process_frontmatter_controlled(
    input: &str,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    control: &OperationControl,
) -> OperationControlResult<FrontmatterCapture> {
    #[cfg(not(feature = "editor-facts"))]
    let decoded = decode_frontmatter_controlled(input, control)?;
    #[cfg(feature = "editor-facts")]
    let decoded = if capture_mode.collects() {
        decode_frontmatter_with_evidence_controlled(input, control)?
    } else {
        decode_frontmatter_controlled(input, control)?
    };
    let Some(decoded) = decoded else {
        return Ok(FrontmatterCapture {
            result: Ok(ProcessedFrontmatter {
                title: None,
                config: MermaidConfig::empty_object(),
                grid_placements: None,
            }),
            full: None,
            #[cfg(feature = "editor-facts")]
            body: None,
            #[cfg(feature = "editor-facts")]
            indent: String::new(),
            #[cfg(feature = "editor-facts")]
            keys: Vec::new(),
            #[cfg(feature = "editor-facts")]
            has_config: false,
            #[cfg(feature = "editor-facts")]
            rewrite_safe: true,
            #[cfg(feature = "editor-facts")]
            fields: None,
            #[cfg(feature = "editor-facts")]
            insert_order: None,
        });
    };
    let parsed = match decoded.result {
        Ok(parsed) => parsed,
        Err(message) => {
            return Ok(FrontmatterCapture {
                result: Err(Error::InvalidFrontMatterYaml { message }),
                full: Some(decoded.full),
                #[cfg(feature = "editor-facts")]
                body: Some(decoded.body),
                #[cfg(feature = "editor-facts")]
                indent: decoded.indent.to_string(),
                #[cfg(feature = "editor-facts")]
                has_config: decoded
                    .keys
                    .iter()
                    .any(|key| key.path.first() == Some("config")),
                #[cfg(feature = "editor-facts")]
                keys: decoded.keys,
                #[cfg(feature = "editor-facts")]
                rewrite_safe: false,
                #[cfg(feature = "editor-facts")]
                fields: None,
                #[cfg(feature = "editor-facts")]
                insert_order: None,
            });
        }
    };
    #[cfg(feature = "editor-facts")]
    let insert_order = decoded.insert_order.and_then(|order| {
        FrontmatterInsertOrder::from_yaml_capture(order, parsed.display_mode.as_deref())
    });

    let mut config = MermaidConfig::empty_object();
    merge_top_level_frontmatter_diagram_configs(&parsed.fields, &mut config);
    if let Some(v) = parsed.config() {
        config.deep_merge(v);
    }
    crate::config::mirror_legacy_font_family_into_theme_variables(&mut config);
    if let Some(dm) = parsed.display_mode {
        config.set_value("gantt.displayMode", Value::String(dm));
    }

    #[cfg(feature = "editor-facts")]
    let has_config = parsed.fields.contains_key("config");
    #[cfg(feature = "editor-facts")]
    let fields = if capture_mode.collects() && has_config && decoded.rewrite_safe {
        Some(MermaidConfig::from_value(Value::Object(parsed.fields)))
    } else {
        crate::json_value::drop_value_nonrecursive(Value::Object(parsed.fields));
        None
    };
    #[cfg(not(feature = "editor-facts"))]
    crate::json_value::drop_value_nonrecursive(Value::Object(parsed.fields));
    control.checkpoint()?;
    Ok(FrontmatterCapture {
        result: Ok(ProcessedFrontmatter {
            title: parsed.title,
            config,
            grid_placements: decoded.grid_placements,
        }),
        full: Some(decoded.full),
        #[cfg(feature = "editor-facts")]
        body: Some(decoded.body),
        #[cfg(feature = "editor-facts")]
        indent: decoded.indent.to_string(),
        #[cfg(feature = "editor-facts")]
        keys: decoded.keys,
        #[cfg(feature = "editor-facts")]
        has_config,
        #[cfg(feature = "editor-facts")]
        rewrite_safe: decoded.rewrite_safe,
        #[cfg(feature = "editor-facts")]
        fields,
        #[cfg(feature = "editor-facts")]
        insert_order,
    })
}

fn merge_top_level_frontmatter_diagram_configs(
    parsed_obj: &serde_json::Map<String, Value>,
    config: &mut MermaidConfig,
) {
    // Mermaid upstream only consumes `config`, but users commonly read docs examples as allowing
    // diagram config namespaces at the YAML root. Keep this compatibility narrow and explicit.
    for &(key, namespace) in frontmatter_config_routes() {
        if let Some(value) = parsed_obj.get(key) {
            config.set_value(
                namespace,
                crate::json_value::clone_value_nonrecursive(value),
            );
        }
    }
}
