use crate::{Error, ParseMetadata, Result};
use serde_json::Value;

use super::db::{SequenceDb, fast_parse_sequence_signals_only};
use super::lexer::Lexer;
use super::sequence_grammar;

pub fn parse_sequence(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let wrap_enabled = meta
        .effective_config
        .as_value()
        .get("wrap")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            meta.effective_config
                .as_value()
                .get("sequence")
                .and_then(|v| v.get("wrap"))
                .and_then(|v| v.as_bool())
        });

    if let Some(v) = fast_parse_sequence_signals_only(code, wrap_enabled, meta) {
        return Ok(v);
    }

    let actions = sequence_grammar::ActionsParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut db = SequenceDb::new(wrap_enabled);
    for a in actions {
        db.apply(a).map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    }

    Ok(db.into_model(meta))
}
