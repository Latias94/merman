use merman_model::families::state::{
    StateDocument, StateDocumentBuilder, StateDocumentError, StateDocumentId, StateStatement,
};
use merman_model::{OperationControl, OperationControlResult};

/// Mutable grammar/DB construction before the completed tree enters the model.
#[derive(Debug, Default)]
pub(super) struct StateDocumentScratch {
    builder: StateDocumentBuilder,
    pub(super) root: Option<StateDocumentId>,
}

impl StateDocumentScratch {
    pub(super) fn from_parsed_parts(builder: StateDocumentBuilder, root: StateDocumentId) -> Self {
        Self {
            builder,
            root: Some(root),
        }
    }

    pub(super) fn push(&mut self, statements: Vec<StateStatement>) -> StateDocumentId {
        self.builder.append(statements)
    }

    pub(super) fn get(&self, id: StateDocumentId) -> Option<&[StateStatement]> {
        self.builder.get(id)
    }

    pub(super) fn get_mut(&mut self, id: StateDocumentId) -> &mut Vec<StateStatement> {
        self.builder
            .edit(id)
            .expect("scratch document id was allocated by this builder")
    }

    pub(super) fn root_statements(&self) -> &[StateStatement] {
        self.root.and_then(|id| self.get(id)).unwrap_or_default()
    }

    pub(super) fn finish_controlled(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<StateDocument, StateDocumentError>> {
        self.builder.build_controlled(self.root, control)
    }
}

pub(super) fn assign_divider_ids(
    document: &mut StateDocumentScratch,
    cnt: &mut usize,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let Some(root) = document.root else {
        return Ok(());
    };
    let mut stack = vec![(root, 0usize)];
    let mut inspected = 0usize;
    while let Some((doc, index)) = stack.last_mut() {
        if inspected.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let Some(stmt) = document.get_mut(*doc).get_mut(*index) else {
            stack.pop();
            continue;
        };
        *index += 1;
        let mut child = None;
        match stmt {
            StateStatement::State(st) => {
                if st.ty == "divider" && st.id == "__divider__" {
                    *cnt += 1;
                    st.id = format!("divider-id-{cnt}");
                }
                child = st.doc;
            }
            StateStatement::Relation(relation) => {
                for st in [&mut relation.state1, &mut relation.state2] {
                    if st.ty == "divider" && st.id == "__divider__" {
                        *cnt += 1;
                        st.id = format!("divider-id-{cnt}");
                    }
                }
            }
            _ => {}
        }
        if let Some(child) = child {
            stack.push((child, 0));
        }
        inspected = inspected.saturating_add(1);
    }
    control.checkpoint()
}

#[cfg(test)]
#[test]
fn state_divider_assignment_cancellation_releases_completed_grammar_documents() {
    let mut source = String::from("stateDiagram-v2\nstate Outer {\n");
    for index in 0..300 {
        source.push_str(&format!("S{index}\n--\n"));
    }
    source.push_str("}\n");
    let parse_control = OperationControl::new();
    let document = super::StateSyntax::lex(&source, &parse_control)
        .unwrap()
        .into_document(&parse_control)
        .unwrap()
        .unwrap();
    let mut document = StateDocumentScratch::from_parsed_parts(document.builder, document.root);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);
    let mut count = 0;
    let result = assign_divider_ids(&mut document, &mut count, &control);
    assert!(result.is_err());
    assert!(
        count > 0 && count < 300,
        "assignment had useful work before cancellation"
    );
    drop(document);
    let control = OperationControl::new();
    let model = super::StateSyntax::lex("stateDiagram-v2\nA\n", &control)
        .unwrap()
        .into_source(&control)
        .unwrap()
        .unwrap()
        .into_semantic_model_controlled(&merman_model::MermaidConfig::default(), &control)
        .unwrap()
        .unwrap();
    assert_eq!(model.nodes.len(), 1);
}
