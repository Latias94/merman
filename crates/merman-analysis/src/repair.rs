//! Bounded immutable repair facts; replacements belong to one diagnostic projection.

use crate::{
    AnalysisCancellationToken, AnalysisCancelled, DiagnosticFix, DiagnosticFixEdit, DiagnosticSpan,
    SourceMap,
};
use merman_core::{MermaidConfig, SourceSpan, preprocess::SourceConfigEvidence};
use std::collections::BTreeMap;
use std::mem::size_of;
use std::sync::Arc;

// Advisory repairs have independent bounds even when source admission has no configured limit.
pub(crate) const MAX_CONFIG_MIGRATION_FIX_WEIGHT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_FRONTMATTER_MIGRATION_INPUT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_FRONTMATTER_MIGRATION_MATERIALIZED_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_FRONTMATTER_MIGRATION_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_FRONTMATTER_MIGRATION_NESTING_DEPTH: usize = 64;
pub(crate) const MAX_CONFIG_MIGRATION_FIX_EDITS: usize = 128;

#[derive(Debug)]
pub(crate) struct MigrationFrontmatter {
    pub(crate) full_span: SourceSpan,
    pub(crate) body_span: SourceSpan,
    pub(crate) indent: String,
    pub(crate) has_config: bool,
    pub(crate) rewrite_safe: bool,
    pub(crate) fields: Option<MermaidConfig>,
}

#[derive(Debug)]
pub(crate) struct ConfigMigrationEvidence {
    pub(crate) source_map: SourceMap,
    pub(crate) authored_config: MermaidConfig,
    pub(crate) frontmatter: Option<MigrationFrontmatter>,
    pub(crate) config_insert_span: SourceSpan,
    pub(crate) removals: Vec<SourceSpan>,
}

impl ConfigMigrationEvidence {
    /// Copies only bounded authored values and repair locations from this same parse.
    pub(crate) fn capture(
        source_map: &SourceMap,
        authored_config: &MermaidConfig,
        source_config: &SourceConfigEvidence,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Option<Arc<Self>>, AnalysisCancelled> {
        cancellation.checkpoint()?;
        if !source_config.rewrite_safe() {
            return Ok(None);
        }
        let source = source_map.source();
        let mut removals = Vec::new();
        for (index, directive) in source_config.directives().iter().enumerate() {
            if index.is_multiple_of(128) {
                cancellation.checkpoint()?;
            }
            if !directive.complete() || !matches!(directive.keyword(), "init" | "initialize") {
                continue;
            }
            if removals.len() == MAX_CONFIG_MIGRATION_FIX_EDITS {
                return Ok(None);
            }
            let span = directive.full_span();
            if source.get(span.start..span.end).is_none() {
                return Ok(None);
            }
            removals.push(
                crate::source_config_rewrite::directive_removal_span_cancellable(
                    source,
                    span,
                    cancellation,
                )?,
            );
        }
        let Some(config) = authored_config
            .clone_value_bounded_controlled(
                MAX_CONFIG_MIGRATION_FIX_WEIGHT_BYTES,
                MAX_FRONTMATTER_MIGRATION_NESTING_DEPTH,
                cancellation.operation_control(),
            )
            .map_err(|_| AnalysisCancelled)?
        else {
            return Ok(None);
        };
        let authored_config = MermaidConfig::from_value(config);
        if authored_config.is_empty_object() {
            return Ok(None);
        }
        let frontmatter = match source_config.frontmatter() {
            None => None,
            Some(frontmatter) => {
                let full_span = frontmatter.full_span();
                let body_span = frontmatter.body_span();
                if source.get(full_span.start..full_span.end).is_none()
                    || source.get(body_span.start..body_span.end).is_none()
                    || full_span.end.saturating_sub(full_span.start)
                        > MAX_FRONTMATTER_MIGRATION_INPUT_BYTES
                    || frontmatter.indent().len() > MAX_FRONTMATTER_MIGRATION_INPUT_BYTES
                {
                    return Ok(None);
                }
                let fields = if frontmatter.has_config() {
                    let Some(fields) = frontmatter.fields() else {
                        return Ok(None);
                    };
                    let Some(mut fields) = fields
                        .clone_value_bounded_controlled(
                            MAX_FRONTMATTER_MIGRATION_MATERIALIZED_BYTES,
                            MAX_FRONTMATTER_MIGRATION_NESTING_DEPTH,
                            cancellation.operation_control(),
                        )
                        .map_err(|_| AnalysisCancelled)?
                    else {
                        return Ok(None);
                    };
                    // Preserve this key's position without retaining the config that projection
                    // replaces with the captured authored overrides.
                    if let Some(config) = fields.get_mut("config") {
                        drop(MermaidConfig::from_value(std::mem::replace(
                            config,
                            serde_json::Value::Null,
                        )));
                    }
                    Some(MermaidConfig::from_value(fields))
                } else {
                    None
                };
                Some(MigrationFrontmatter {
                    full_span,
                    body_span,
                    indent: frontmatter.indent().to_owned(),
                    has_config: frontmatter.has_config(),
                    rewrite_safe: frontmatter.rewrite_safe(),
                    fields,
                })
            }
        };
        cancellation.checkpoint()?;
        Ok(Some(Arc::new(Self {
            source_map: source_map.clone(),
            authored_config,
            frontmatter,
            config_insert_span: source_config.config_insert_span(),
            removals,
        })))
    }

    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        let mut weight = crate::retained_weight::RetainedWeight::new(
            crate::retained_weight::ARC_ALLOCATION_OVERHEAD.saturating_add(size_of::<Self>()),
        );
        weight.add(crate::retained_weight::ARC_ALLOCATION_OVERHEAD);
        weight.add(self.authored_config.estimated_owned_heap_bytes());
        weight.add(
            self.source_map
                .estimated_owned_heap_bytes_excluding_source(),
        );
        weight.add_array::<SourceSpan>(self.removals.capacity());
        if let Some(frontmatter) = &self.frontmatter {
            weight.add_string(&frontmatter.indent);
            if let Some(fields) = &frontmatter.fields {
                weight.add(crate::retained_weight::ARC_ALLOCATION_OVERHEAD);
                weight.add(fields.estimated_owned_heap_bytes());
            }
        }
        weight.finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RepairEvidence {
    Replacement {
        span: DiagnosticSpan,
        title: &'static str,
        replacement: &'static str,
    },
    ConfigMigration {
        evidence: Arc<ConfigMigrationEvidence>,
        document: Option<(SourceMap, usize)>,
    },
}

impl RepairEvidence {
    pub(crate) fn config_migration(evidence: Arc<ConfigMigrationEvidence>) -> Self {
        Self::ConfigMigration {
            evidence,
            document: None,
        }
    }

    /// Source spans remain original diagram coordinates until the enclosing document is known.
    pub(crate) fn map_to_document(
        self,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Option<Self>, AnalysisCancelled> {
        cancellation.checkpoint()?;
        Ok(match self {
            Self::Replacement {
                span,
                title,
                replacement,
            } => map_span(source_map, body_start, span, cancellation)?.map(|span| {
                Self::Replacement {
                    span,
                    title,
                    replacement,
                }
            }),
            Self::ConfigMigration { evidence, .. } => Some(Self::ConfigMigration {
                evidence,
                document: Some((source_map.clone(), body_start)),
            }),
        })
    }

    pub(crate) fn retained_allocation(&self) -> Option<(usize, &ConfigMigrationEvidence)> {
        match self {
            Self::Replacement { .. } => None,
            Self::ConfigMigration { evidence, .. } => {
                Some((Arc::as_ptr(evidence) as usize, evidence))
            }
        }
    }
}

/// Temporary aggregate edit ownership for one projection, never stored in a generation.
#[derive(Default)]
pub(crate) struct FixProjection {
    migrations: BTreeMap<(usize, usize), Option<DiagnosticFix>>,
}

impl FixProjection {
    pub(crate) fn materialize(
        &mut self,
        repair: &RepairEvidence,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Option<DiagnosticFix>, AnalysisCancelled> {
        cancellation.checkpoint()?;
        match repair {
            RepairEvidence::Replacement {
                span,
                title,
                replacement,
            } => {
                #[cfg(test)]
                crate::test_support::record_fix_materialization();
                Ok(Some(
                    DiagnosticFix::new(*title, vec![DiagnosticFixEdit::new(*span, *replacement)])
                        .preferred(),
                ))
            }
            RepairEvidence::ConfigMigration { evidence, document } => {
                let key = (
                    Arc::as_ptr(evidence) as usize,
                    document.as_ref().map_or(0, |(_, start)| *start),
                );
                if let Some(fix) = self.migrations.get(&key) {
                    return Ok(fix.clone());
                }
                #[cfg(test)]
                crate::test_support::record_fix_materialization();
                let mut fix =
                    crate::source_config_rewrite::materialize_config_migration_cancellable(
                        evidence,
                        cancellation,
                    )?;
                if let (Some(current), Some((source_map, body_start))) = (&mut fix, document) {
                    let mut edits = Vec::with_capacity(current.edits.len());
                    for edit in current.edits.iter() {
                        cancellation.checkpoint()?;
                        let Some(span) =
                            map_span(source_map, *body_start, edit.span, cancellation)?
                        else {
                            edits.clear();
                            break;
                        };
                        edits.push(DiagnosticFixEdit::new(span, edit.replacement.clone()));
                    }
                    if edits.is_empty() {
                        fix = None;
                    } else {
                        current.edits = Arc::from(edits);
                    }
                }
                self.migrations.insert(key, fix.clone());
                cancellation.checkpoint()?;
                Ok(fix)
            }
        }
    }
}

fn map_span(
    source_map: &SourceMap,
    body_start: usize,
    span: DiagnosticSpan,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<DiagnosticSpan>, AnalysisCancelled> {
    let Some(start) = body_start.checked_add(span.byte_start) else {
        return Ok(None);
    };
    let Some(end) = body_start.checked_add(span.byte_end) else {
        return Ok(None);
    };
    Ok(source_map.span_cancellable(start, end, cancellation)?.ok())
}
