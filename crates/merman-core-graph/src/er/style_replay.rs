use super::{ErClassDef, ErDb, ErFailure, ErSyntaxDiagnostic};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

pub(super) const STYLE_REPLAY_LIMIT: StyleReplayCost = StyleReplayCost {
    string_entries: 200_000,
    text_bytes: 16 * 1024 * 1024,
};

const TEXT_STYLE_MATCH: &str = "color";
const FILL_MATCH: &str = "fill";
const TEXT_STYLE_FILL: &str = "bgFill";

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StyleReplayCost {
    pub(super) string_entries: usize,
    pub(super) text_bytes: usize,
}

impl StyleReplayCost {
    fn add(self, other: Self) -> Self {
        Self {
            string_entries: self.string_entries.saturating_add(other.string_entries),
            text_bytes: self.text_bytes.saturating_add(other.text_bytes),
        }
    }

    fn repeat(self, copies: usize) -> Self {
        Self {
            string_entries: self.string_entries.saturating_mul(copies),
            text_bytes: self.text_bytes.saturating_mul(copies),
        }
    }

    fn record_style(&mut self, bytes: usize, text_fill_matches: Option<usize>) {
        let mut cost = Self {
            string_entries: 1,
            text_bytes: bytes,
        };
        if let Some(matches) = text_fill_matches {
            cost.string_entries += 1;
            cost.text_bytes = cost
                .text_bytes
                .saturating_add(bytes)
                .saturating_add(matches.saturating_mul(TEXT_STYLE_FILL.len() - FILL_MATCH.len()));
        }
        *self = self.add(cost);
    }
}

pub(super) struct StyleReplay<'a> {
    control: &'a OperationControl,
    limit: StyleReplayCost,
    used: StyleReplayCost,
}

impl<'a> StyleReplay<'a> {
    pub(super) fn new(control: &'a OperationControl, limit: StyleReplayCost) -> Self {
        Self {
            control,
            limit,
            used: StyleReplayCost::default(),
        }
    }

    pub(super) fn set_class(
        &mut self,
        db: &mut ErDb,
        entities: &[String],
        classes: &[String],
        span: SourceSpan,
    ) -> Result<(), ErFailure> {
        let (nodes, subgraphs) = self.count_targets(db, entities)?;
        if nodes == 0 && subgraphs == 0 {
            return Ok(());
        }
        let mut class_bytes = 0usize;
        for (index, class) in classes.iter().enumerate() {
            self.checkpoint_items(index)?;
            class_bytes = class_bytes.saturating_add(class.len());
        }
        self.admit(
            StyleReplayCost {
                string_entries: subgraphs.saturating_mul(classes.len()),
                text_bytes: nodes
                    .saturating_mul(class_bytes.saturating_add(classes.len()))
                    .saturating_add(subgraphs.saturating_mul(class_bytes)),
            },
            span,
        )?;
        for (index, id) in entities.iter().enumerate() {
            self.checkpoint_items(index)?;
            if let Some(node) = db.entities.get_mut(id) {
                for (index, class) in classes.iter().enumerate() {
                    self.checkpoint_items(index)?;
                    node.css_classes.push(' ');
                    node.css_classes.push_str(class);
                }
            }
            if let Some(index) = db.subgraph_lookup.get(id).copied()
                && let Some(subgraph) = db.subgraphs.get_mut(index)
            {
                for (index, class) in classes.iter().enumerate() {
                    self.checkpoint_items(index)?;
                    subgraph.classes.push(class.clone());
                }
            }
        }
        Ok(())
    }

    pub(super) fn add_class_def(
        &mut self,
        db: &mut ErDb,
        classes: Vec<String>,
        raw: &str,
        span: SourceSpan,
    ) -> Result<(), ErFailure> {
        let cost = self.style_cost(raw, true)?.repeat(classes.len());
        self.admit(cost, span)?;
        for (index, id) in classes.into_iter().enumerate() {
            self.checkpoint_items(index)?;
            let entry = db.classes.entry(id).or_insert_with_key(|id| ErClassDef {
                id: id.clone(),
                ..Default::default()
            });
            self.write_styles(raw, &mut entry.styles, Some(&mut entry.text_styles))?;
        }
        Ok(())
    }

    pub(super) fn add_css_styles(
        &mut self,
        db: &mut ErDb,
        entities: &[String],
        raw: &str,
        span: SourceSpan,
    ) -> Result<(), ErFailure> {
        let (nodes, subgraphs) = self.count_targets(db, entities)?;
        let copies = nodes.saturating_add(subgraphs);
        if copies == 0 {
            return Ok(());
        }
        let cost = self.style_cost(raw, false)?.repeat(copies);
        self.admit(cost, span)?;
        for (index, id) in entities.iter().enumerate() {
            self.checkpoint_items(index)?;
            if let Some(entity) = db.entities.get_mut(id) {
                self.write_styles(raw, &mut entity.css_styles, None)?;
            }
            if let Some(index) = db.subgraph_lookup.get(id).copied()
                && let Some(subgraph) = db.subgraphs.get_mut(index)
            {
                self.write_styles(raw, &mut subgraph.css_styles, None)?;
            }
        }
        Ok(())
    }

    fn count_targets(
        &self,
        db: &ErDb,
        entities: &[String],
    ) -> OperationControlResult<(usize, usize)> {
        let mut nodes = 0;
        let mut subgraphs = 0;
        for (index, id) in entities.iter().enumerate() {
            self.checkpoint_items(index)?;
            nodes += usize::from(db.entities.contains_key(id));
            subgraphs += usize::from(
                db.subgraph_lookup
                    .get(id)
                    .is_some_and(|&index| index < db.subgraphs.len()),
            );
        }
        Ok((nodes, subgraphs))
    }

    fn style_cost(&self, raw: &str, derive_text: bool) -> OperationControlResult<StyleReplayCost> {
        let mut cost = StyleReplayCost::default();
        let mut start = 0;
        let mut text_style = false;
        let mut fill_matches = 0;
        let bytes = raw.as_bytes();
        for (offset, &byte) in bytes.iter().enumerate() {
            if offset.is_multiple_of(4096) {
                self.control.checkpoint()?;
            }
            match byte {
                b',' => {
                    cost.record_style(offset - start, text_style.then_some(fill_matches));
                    start = offset + 1;
                    text_style = false;
                    fill_matches = 0;
                }
                b'c' if derive_text && !text_style => {
                    text_style = bytes[offset..].starts_with(TEXT_STYLE_MATCH.as_bytes());
                }
                b'f' if derive_text && bytes[offset..].starts_with(FILL_MATCH.as_bytes()) => {
                    fill_matches += 1;
                }
                _ => {}
            }
        }
        cost.record_style(bytes.len() - start, text_style.then_some(fill_matches));
        Ok(cost)
    }

    fn write_styles(
        &self,
        raw: &str,
        styles: &mut Vec<String>,
        mut text_styles: Option<&mut Vec<String>>,
    ) -> OperationControlResult<()> {
        for (index, style) in raw.split(',').enumerate() {
            self.checkpoint_items(index)?;
            if let Some(text_styles) = text_styles.as_deref_mut()
                && style.contains(TEXT_STYLE_MATCH)
            {
                text_styles.push(style.replace(FILL_MATCH, TEXT_STYLE_FILL));
            }
            styles.push(style.to_owned());
        }
        Ok(())
    }

    fn admit(&mut self, cost: StyleReplayCost, span: SourceSpan) -> Result<(), ErFailure> {
        self.control.checkpoint()?;
        let total = self.used.add(cost);
        for (unit, actual, maximum) in [
            (
                "string entries",
                total.string_entries,
                self.limit.string_entries,
            ),
            ("UTF-8 bytes", total.text_bytes, self.limit.text_bytes),
        ] {
            if actual > maximum {
                return Err(ErFailure::Syntax(ErSyntaxDiagnostic::StyleExpansionLimit {
                    span,
                    unit,
                    actual,
                    maximum,
                }));
            }
        }
        self.used = total;
        Ok(())
    }

    fn checkpoint_items(&self, index: usize) -> OperationControlResult<()> {
        if index.is_multiple_of(128) {
            self.control.checkpoint()?;
        }
        Ok(())
    }
}
