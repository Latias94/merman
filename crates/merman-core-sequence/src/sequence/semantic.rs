use merman_model::{
    OperationControl, OperationControlResult, families::sequence::SequenceDiagramModel,
};

use super::{SequenceSyntax, SequenceSyntaxDiagnostic, ast::Action, db::SequenceDb};

/// A grammar or database failure in the preprocessed source coordinate space.
#[derive(Debug)]
pub enum SequenceModelError {
    Grammar(SequenceSyntaxDiagnostic),
    Db(String),
}

impl SequenceSyntax {
    /// Consumes the same syntax used by optional editor capture and builds the semantic model.
    pub fn into_model(
        self,
        wrap_enabled: Option<bool>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<SequenceDiagramModel, SequenceModelError>> {
        let actions = match self.into_actions(control)? {
            Ok(actions) => actions,
            Err(diagnostic) => return Ok(Err(SequenceModelError::Grammar(diagnostic))),
        };
        let db = match build_sequence_db(actions, wrap_enabled, control)? {
            Ok(db) => db,
            Err(message) => return Ok(Err(SequenceModelError::Db(message))),
        };
        control.checkpoint()?;
        Ok(Ok(db.into_model()))
    }
}

fn build_sequence_db(
    actions: Vec<Action>,
    wrap_enabled: Option<bool>,
    control: &OperationControl,
) -> OperationControlResult<Result<SequenceDb, String>> {
    let mut db = SequenceDb::new(wrap_enabled);
    for (index, action) in actions.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if let Err(error) = db.apply_controlled(action, control)? {
            return Ok(Err(error));
        }
    }
    Ok(Ok(db))
}

#[cfg(test)]
mod tests {
    use super::{SequenceSyntax, build_sequence_db};
    use merman_model::{CancelReason, OperationControl};

    #[test]
    fn semantic_replay_updates_duplicate_link_labels_without_moving_them() {
        let source = concat!(
            "sequenceDiagram\nparticipant A\n",
            r#"links A: {"Zebra":"https://example.test/old","Guide":"https://example.test/guide","Zebra":"https://example.test/new","Ignore":null}"#,
            "\nlink A: Help @ https://example.test/help\n",
        );
        let control = OperationControl::new();
        let model = SequenceSyntax::lex(source, &control)
            .unwrap()
            .into_model(None, &control)
            .unwrap()
            .unwrap();
        let links: Vec<_> = model.actors["A"]
            .links
            .iter()
            .map(|(label, url)| (label.as_str(), url.as_str().unwrap()))
            .collect();
        assert_eq!(
            links,
            [
                ("Zebra", "https://example.test/new"),
                ("Guide", "https://example.test/guide"),
                ("Help", "https://example.test/help"),
            ],
        );
    }

    #[test]
    fn cancelled_replay_releases_remaining_flat_events() {
        let source = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(256));
        let control = OperationControl::new();
        let actions = SequenceSyntax::lex(&source, &control)
            .unwrap()
            .into_actions(&control)
            .unwrap()
            .expect("Sequence parser accepts the workload");
        control.cancel_after_checkpoints(16);
        let Err(error) = build_sequence_db(actions, None, &control) else {
            panic!("replay returns cancellation through the outer result");
        };
        assert_eq!(error.reason, CancelReason::Requested);
    }
}
