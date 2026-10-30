pub use merman_model::families::state::{
    StateDocument, StateDocumentId, StateStatement, StateStatementClick, StateStatementNote,
    StateStatementRelation, StateStatementState,
};
use merman_model::families::state::{StateDocumentBuilder, StateDocumentError};

/// Mutable grammar/DB construction; only a validated completed tree crosses into model data.
#[derive(Debug, Default)]
pub(crate) struct StateDocumentScratch {
    builder: StateDocumentBuilder,
    pub(super) root: Option<StateDocumentId>,
}

impl StateDocumentScratch {
    pub(crate) fn push(&mut self, statements: Vec<StateStatement>) -> StateDocumentId {
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
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Result<StateDocument, StateDocumentError>> {
        self.builder.build_controlled(self.root, control)
    }
}
