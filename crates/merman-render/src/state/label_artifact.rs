//! Operation-local prepared text retained by one State family artifact.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagram_theme::LineHeight;
use crate::entities::decode_mermaid_entities_for_render_text;
use crate::text::{
    CatalogAdmittedTextStyle, PendingPreparedTextLabelLedgerEntry, PrepareTextRequest,
    PreparedText, PreparedTextLabelFamily, PreparedTextLabelId, PreparedTextLabelLedgerEntry,
    PreparedTextLayout, PreparedTextWrap, TextLayoutError, TextMeasurer, TextMetrics, WrapMode,
};

use super::{
    ResolvedLabelTypography, StateLabelMeasurement, measure_state_markdown_label,
    state_markdown_label_plain_text,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateLabelSourceKind {
    Markdown,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateLabelOwner<'a> {
    Node(&'a str),
    NodeTitle(&'a str),
    NodeDescription(&'a str),
    ClusterTitle(&'a str),
    Edge(&'a str),
}

#[derive(Clone, Copy)]
pub(crate) struct StateLabelMetricsRequest<'a> {
    pub(crate) owner: StateLabelOwner<'a>,
    pub(crate) text: &'a str,
    pub(crate) source_kind: StateLabelSourceKind,
    pub(crate) measurer: &'a dyn TextMeasurer,
    pub(crate) typography: &'a ResolvedLabelTypography,
    pub(crate) max_width_px: Option<f64>,
    pub(crate) wrap_mode: WrapMode,
    pub(crate) break_long_words: bool,
}

#[derive(Debug)]
pub(crate) struct PreparedStateLabel {
    semantic_source: Arc<str>,
    prepared: PreparedText,
    admitted_typography: CatalogAdmittedTextStyle,
    uses_html_wrapping_table: bool,
    pending_label_entry: Option<PendingPreparedTextLabelLedgerEntry>,
    label_entry: Option<PreparedTextLabelLedgerEntry>,
    label_consumed: Cell<bool>,
}

impl PreparedStateLabel {
    pub(crate) fn wrapped_lines(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.prepared.wrapped_lines()
    }

    pub(crate) const fn metrics(&self) -> TextMetrics {
        self.prepared.metrics()
    }

    pub(crate) const fn uses_html_wrapping_table(&self) -> bool {
        self.uses_html_wrapping_table
    }

    pub(crate) fn matches_semantic_source(&self, source: &str) -> bool {
        self.semantic_source.as_ref() == source
    }

    pub(crate) fn merge_emission_font_style(&self, existing: Option<&str>) -> String {
        self.admitted_typography.merge_emission_font_style(existing)
    }

    fn bind_label_id(&mut self, key: u32) -> bool {
        let Some(pending) = self.pending_label_entry.take() else {
            return false;
        };
        self.label_entry = Some(pending.bind(PreparedTextLabelId::new(
            PreparedTextLabelFamily::State,
            key,
        )));
        true
    }

    pub(crate) fn label_id_for_emission(&self) -> Option<PreparedTextLabelId> {
        let id = self
            .label_entry
            .as_ref()
            .map(PreparedTextLabelLedgerEntry::id);
        if id.is_some() {
            self.label_consumed.set(true);
        }
        id
    }

    fn consumed_label_entry(&self) -> Option<&PreparedTextLabelLedgerEntry> {
        self.label_consumed
            .get()
            .then_some(self.label_entry.as_ref())
            .flatten()
    }
}

#[derive(Debug)]
struct StateLabelSlots<T> {
    nodes: BTreeMap<Box<str>, T>,
    node_titles: BTreeMap<Box<str>, T>,
    node_descriptions: BTreeMap<Box<str>, T>,
    cluster_titles: BTreeMap<Box<str>, T>,
    edges: BTreeMap<Box<str>, T>,
}

impl<T> Default for StateLabelSlots<T> {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            node_titles: BTreeMap::new(),
            node_descriptions: BTreeMap::new(),
            cluster_titles: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }
}

impl<T> StateLabelSlots<T> {
    fn insert(&mut self, owner: StateLabelOwner<'_>, value: T) {
        match owner {
            StateLabelOwner::Node(id) => {
                self.nodes.insert(id.into(), value);
            }
            StateLabelOwner::NodeTitle(id) => {
                self.node_titles.insert(id.into(), value);
            }
            StateLabelOwner::NodeDescription(id) => {
                self.node_descriptions.insert(id.into(), value);
            }
            StateLabelOwner::ClusterTitle(id) => {
                self.cluster_titles.insert(id.into(), value);
            }
            StateLabelOwner::Edge(id) => {
                self.edges.insert(id.into(), value);
            }
        }
    }

    fn for_each_mut(&mut self, mut visit: impl FnMut(&mut T)) {
        for value in self.nodes.values_mut() {
            visit(value);
        }
        for value in self.node_titles.values_mut() {
            visit(value);
        }
        for value in self.node_descriptions.values_mut() {
            visit(value);
        }
        for value in self.cluster_titles.values_mut() {
            visit(value);
        }
        for value in self.edges.values_mut() {
            visit(value);
        }
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.nodes
            .values()
            .chain(self.node_titles.values())
            .chain(self.node_descriptions.values())
            .chain(self.cluster_titles.values())
            .chain(self.edges.values())
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateLabelSidecarBuilder {
    prepared_active: bool,
    prepared_text_layout: Option<PreparedTextLayout>,
    labels: RefCell<StateLabelSlots<PreparedStateLabel>>,
    prepared_error: RefCell<Option<TextLayoutError>>,
}

impl StateLabelSidecarBuilder {
    pub(crate) fn new(prepared_text_layout: Option<&PreparedTextLayout>) -> Self {
        Self {
            prepared_active: prepared_text_layout.is_some(),
            prepared_text_layout: prepared_text_layout.cloned(),
            ..Self::default()
        }
    }

    pub(crate) fn measure_for_layout(
        &self,
        request: StateLabelMetricsRequest<'_>,
    ) -> StateLabelMeasurement {
        let Some(layout) = self.prepared_text_layout.as_ref() else {
            return measure_without_prepared_text(request);
        };
        if self.prepared_error.borrow().is_some() {
            return failed_prepared_measurement();
        }

        match self.prepare_label(layout, request) {
            Ok(label) => {
                let measurement = StateLabelMeasurement {
                    metrics: label.metrics(),
                    uses_html_wrapping_table: label.uses_html_wrapping_table(),
                };
                self.labels.borrow_mut().insert(request.owner, label);
                measurement
            }
            Err(error) => {
                self.record_error(error);
                failed_prepared_measurement()
            }
        }
    }

    fn prepare_label(
        &self,
        layout: &PreparedTextLayout,
        request: StateLabelMetricsRequest<'_>,
    ) -> Result<PreparedStateLabel, TextLayoutError> {
        let prepared_source = match request.source_kind {
            StateLabelSourceKind::Markdown => state_markdown_label_plain_text(request.text).ok_or(
                TextLayoutError::UnsupportedPreparedTextPath("state_markdown_label"),
            )?,
            StateLabelSourceKind::Plain => decode_mermaid_entities_for_render_text(request.text),
        };
        let typography = request
            .typography
            .prepared_typography()
            .ok_or(TextLayoutError::UnsupportedPreparedTextPath(
                "structured_typography",
            ))?
            .clone()
            .with_line_height(match request.wrap_mode {
                WrapMode::HtmlLike => LineHeight::Multiplier(1.5),
                WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => LineHeight::Multiplier(1.1),
            })
            .map_err(|_| TextLayoutError::InvalidRequest("line_height"))?;
        let admitted_typography = layout.admit_typography_with_css_font_stack(
            &typography,
            request.typography.source_font_stack(),
        )?;
        debug_assert_eq!(
            admitted_typography.catalog_fingerprint(),
            layout.catalog_fingerprint(),
            "prepared State typography must remain bound to the active catalog"
        );
        let wrap = match request.wrap_mode {
            WrapMode::SvgLike => PreparedTextWrap::SvgLike {
                max_width_px: normalized_width(request.max_width_px),
                break_long_words: request.break_long_words,
            },
            WrapMode::SvgLikeSingleRun => PreparedTextWrap::SingleRun,
            WrapMode::HtmlLike => PreparedTextWrap::HtmlLike {
                max_width_px: normalized_width(request.max_width_px),
            },
        };
        let prepared = layout.prepare_text(
            &PrepareTextRequest::new(
                prepared_source.as_ref(),
                admitted_typography.typography().clone(),
            )
            .with_family_normalized_projection()
            .with_wrap(wrap),
        )?;
        let pending_label_entry = prepared
            .label_ledger_entry()
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        let metrics = prepared.metrics();
        let max_width = normalized_width(request.max_width_px);
        let uses_html_wrapping_table = request.wrap_mode == WrapMode::HtmlLike
            && max_width.is_some_and(|width| {
                prepared.raw_width_px().unwrap_or(metrics.width) >= width - 1e-9
            });
        Ok(PreparedStateLabel {
            semantic_source: Arc::from(request.text),
            prepared,
            admitted_typography,
            uses_html_wrapping_table,
            pending_label_entry: Some(pending_label_entry),
            label_entry: None,
            label_consumed: Cell::new(false),
        })
    }

    fn record_error(&self, error: TextLayoutError) {
        let mut prepared_error = self.prepared_error.borrow_mut();
        if prepared_error.is_none() {
            *prepared_error = Some(error);
        }
    }

    pub(crate) fn reject_unsupported(&self, path: &'static str) {
        if self.prepared_text_layout.is_some() {
            self.record_error(TextLayoutError::UnsupportedPreparedTextPath(path));
        }
    }

    pub(crate) fn finish(self) -> StateLabelSidecar {
        let mut labels = self.labels.into_inner();
        let mut next_key = 0usize;
        let mut key_overflow = false;
        labels.for_each_mut(|label| {
            let Ok(key) = u32::try_from(next_key) else {
                if label.pending_label_entry.is_some() {
                    key_overflow = true;
                }
                return;
            };
            if label.bind_label_id(key) {
                next_key = next_key.saturating_add(1);
            }
        });
        let mut prepared_error = self.prepared_error.into_inner();
        if key_overflow && prepared_error.is_none() {
            prepared_error = Some(TextLayoutError::LimitExceeded("prepared_label_ledger"));
        }
        StateLabelSidecar {
            prepared_active: self.prepared_active,
            labels,
            prepared_error,
        }
    }
}

fn failed_prepared_measurement() -> StateLabelMeasurement {
    StateLabelMeasurement {
        metrics: TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        },
        uses_html_wrapping_table: false,
    }
}

fn normalized_width(width: Option<f64>) -> Option<f64> {
    width.filter(|width| width.is_finite() && *width > 0.0)
}

fn measure_without_prepared_text(request: StateLabelMetricsRequest<'_>) -> StateLabelMeasurement {
    match request.source_kind {
        StateLabelSourceKind::Markdown => measure_state_markdown_label(
            request.text,
            request.measurer,
            request.typography.text_style(),
            request.max_width_px,
            request.wrap_mode,
        ),
        StateLabelSourceKind::Plain => {
            let text = decode_mermaid_entities_for_render_text(request.text);
            StateLabelMeasurement {
                metrics: request.measurer.measure_wrapped(
                    text.as_ref(),
                    request.typography.text_style(),
                    request.max_width_px,
                    request.wrap_mode,
                ),
                uses_html_wrapping_table: false,
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateLabelSidecar {
    prepared_active: bool,
    labels: StateLabelSlots<PreparedStateLabel>,
    prepared_error: Option<TextLayoutError>,
}

impl StateLabelSidecar {
    pub(crate) fn prepared_error(&self) -> Option<&TextLayoutError> {
        self.prepared_error.as_ref()
    }

    pub(crate) fn prepared_text_label_ledger(
        &self,
    ) -> impl Iterator<Item = &PreparedTextLabelLedgerEntry> {
        self.labels
            .iter()
            .filter_map(PreparedStateLabel::consumed_label_entry)
    }

    pub(crate) fn node(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.nodes.get(id)
    }

    pub(crate) fn node_title(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.node_titles.get(id)
    }

    pub(crate) fn node_description(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.node_descriptions.get(id)
    }

    pub(crate) fn cluster_title(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.cluster_titles.get(id)
    }

    pub(crate) fn edge(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.edges.get(id)
    }

    pub(crate) fn validate_for_render(
        &self,
        model: &merman_core::diagrams::state::StateDiagramRenderModel,
        layout: &crate::model::StateDiagramLayout,
    ) -> Result<(), TextLayoutError> {
        if !self.prepared_active {
            return Ok(());
        }

        for layout_node in &layout.nodes {
            if layout_node.is_cluster {
                continue;
            }
            let node = model
                .nodes
                .iter()
                .find(|node| node.id == layout_node.id)
                .ok_or(TextLayoutError::InvalidPreparedText)?;
            let source = node
                .label
                .as_ref()
                .map(super::state_value_to_label_text)
                .unwrap_or_else(|| node.id.clone());
            match node.shape.as_str() {
                "stateStart" | "stateEnd" | "choice" | "fork" | "join" => {}
                "rectWithTitle" => {
                    validate_source(self.node_title(&node.id), &source)?;
                    let description = node
                        .description
                        .as_ref()
                        .map(|parts| parts.join("\n"))
                        .unwrap_or_default();
                    validate_source(self.node_description(&node.id), &description)?;
                }
                _ => validate_source(self.node(&node.id), &source)?,
            }
        }

        for cluster in &layout.clusters {
            if cluster.title.trim().is_empty() {
                continue;
            }
            validate_source(self.cluster_title(&cluster.id), &cluster.title)?;
        }

        for layout_edge in &layout.edges {
            if layout_edge.label.is_none() {
                continue;
            }
            let edge = model
                .edges
                .iter()
                .find(|edge| edge.id == layout_edge.id)
                .ok_or(TextLayoutError::InvalidPreparedText)?;
            let source = edge.label.trim();
            if !source.is_empty() {
                validate_source(self.edge(&edge.id), source)?;
            }
        }
        Ok(())
    }
}

fn validate_source(
    prepared: Option<&PreparedStateLabel>,
    semantic_source: &str,
) -> Result<(), TextLayoutError> {
    let prepared = prepared.ok_or(TextLayoutError::InvalidPreparedText)?;
    if prepared.matches_semantic_source(semantic_source) {
        Ok(())
    } else {
        Err(TextLayoutError::RequestDigestMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec,
        FontSourcePolicy, FontStack, OrdinalSelector, ResolvedDiagramTheme, Specified,
        TextStylePatch, ThemeAssets, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTextStyle,
        TypographySpec,
    };
    use crate::render_family::RenderFamilyKind;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use crate::text::{
        DeterministicTextMeasurer, NativeTextLayoutBackend, PrepareCatalogRequest, TextStyle,
    };
    use merman_core::diagrams::state::{StateDiagramRenderModel, StateDiagramRenderNode};
    use serde_json::json;

    fn prepared_state_fixture() -> (
        PreparedTextLayout,
        ResolvedDiagramTheme,
        StateDiagramRenderModel,
    ) {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let catalog = FontCatalogSpec::new([FontAssetSpec::new("excalifont", bytes)]);
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::new(["Arial", "Excalifont"]).expect("fixture font stack should be valid"),
        );
        let ordinal = OrdinalSelector::exact(1).unwrap();
        let mut generic_text = TextStylePatch::default();
        generic_text.font_size_px = Specified::Value(19.0);
        let mut state_label = TextStylePatch::default();
        state_label.font_size_px = Specified::Value(23.0);
        let rules = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(
                    crate::diagram_theme::ThemeTarget::State,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    crate::diagram_theme::ThemeTarget::State,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#22c55e").unwrap()),
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    crate::diagram_theme::ThemeTarget::Text,
                    ThemeStylePatch {
                        typography: generic_text,
                        ..ThemeStylePatch::default()
                    },
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    crate::diagram_theme::ThemeTarget::StateLabel,
                    ThemeStylePatch {
                        typography: state_label,
                        ..ThemeStylePatch::default()
                    },
                )
                .with_ordinal(ordinal),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography))
                    .with_assets(ThemeAssets::default().with_font_catalog(catalog))
                    .with_styles(rules),
            )
            .expect("fixture theme should compile");
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                theme.font_catalog().clone(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let resolved = theme.resolve(RenderFamilyKind::State);
        let model = StateDiagramRenderModel {
            nodes: vec![StateDiagramRenderNode {
                id: "Ready".to_string(),
                label_style: String::new(),
                label: Some(json!("Ready")),
                description: None,
                dom_id: "state-Ready-0".to_string(),
                is_group: false,
                node_type: None,
                parent_id: None,
                css_classes: String::new(),
                css_compiled_styles: Vec::new(),
                css_styles: Vec::new(),
                dir: None,
                explicit_dir: None,
                padding: Some(8.0),
                rx: Some(0.0),
                ry: Some(0.0),
                shape: "rect".to_string(),
                position: None,
            }],
            ..StateDiagramRenderModel::default()
        };
        (prepared, resolved, model)
    }

    #[test]
    fn prepared_state_label_reuses_metered_plan_typography_without_theme_lookup() {
        let (prepared, resolved, model) = prepared_state_fixture();
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let (plan, _) = crate::state::StateStylePlan::resolve_with_evidence(
            &model,
            &json!({}),
            Some(&resolved),
            None,
            true,
            &work_meter,
        )
        .expect("resolve metered State style plan");
        assert_eq!(work_meter.used(), 4);
        let node = plan.node("Ready").expect("prepared State node style");
        let measurer = DeterministicTextMeasurer::default();
        let sidecar = StateLabelSidecarBuilder::new(Some(&prepared));

        let measurement = sidecar.measure_for_layout(StateLabelMetricsRequest {
            owner: StateLabelOwner::Node("Ready"),
            text: "Ready",
            source_kind: StateLabelSourceKind::Markdown,
            measurer: &measurer,
            typography: node.resolved_label_typography(),
            max_width_px: Some(180.0),
            wrap_mode: WrapMode::SvgLike,
            break_long_words: true,
        });
        let sidecar = sidecar.finish();

        assert!(measurement.metrics.width > 0.0);
        assert!(sidecar.prepared_error().is_none());
        let prepared_label = sidecar.node("Ready").expect("prepared State label");
        let emission_style =
            prepared_label.merge_emission_font_style(Some(node.label_style_attr()));
        assert!(emission_style.contains("font-family:\"Excalifont\" !important"));
        assert!(!emission_style.contains("Arial"));
        assert!(emission_style.contains("font-size:23px !important"));
        assert_eq!(work_meter.used(), 4);
    }

    #[test]
    fn prepared_state_label_without_planned_typography_remains_fail_closed() {
        let (prepared, resolved, model) = prepared_state_fixture();
        let measurer = DeterministicTextMeasurer::default();
        let typography = ResolvedLabelTypography::new(TextStyle::default(), None);
        let sidecar = StateLabelSidecarBuilder::new(Some(&prepared));

        let measurement = sidecar.measure_for_layout(StateLabelMetricsRequest {
            owner: StateLabelOwner::Node("Ready"),
            text: "Ready",
            source_kind: StateLabelSourceKind::Markdown,
            measurer: &measurer,
            typography: &typography,
            max_width_px: Some(180.0),
            wrap_mode: WrapMode::SvgLike,
            break_long_words: true,
        });
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let (plan, _) = crate::state::StateStylePlan::resolve_with_evidence(
            &model,
            &json!({}),
            Some(&resolved),
            None,
            true,
            &work_meter,
        )
        .expect("resolve metered State style plan");
        let node = plan.node("Ready").expect("prepared State node style");
        let skipped = sidecar.measure_for_layout(StateLabelMetricsRequest {
            owner: StateLabelOwner::NodeTitle("Ready"),
            text: "Not prepared after the terminal error",
            source_kind: StateLabelSourceKind::Plain,
            measurer: &measurer,
            typography: node.resolved_label_typography(),
            max_width_px: None,
            wrap_mode: WrapMode::SvgLikeSingleRun,
            break_long_words: false,
        });
        let sidecar = sidecar.finish();

        assert_eq!(measurement.metrics.width, 0.0);
        assert_eq!(skipped.metrics.width, 0.0);
        assert!(sidecar.node_title("Ready").is_none());
        assert!(matches!(
            sidecar.prepared_error(),
            Some(TextLayoutError::UnsupportedPreparedTextPath(
                "structured_typography"
            ))
        ));
    }
}
