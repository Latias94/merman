#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::super::*;
    use crate::resources::ModelComplexity;
    use crate::{MermaidConfig, OperationControl};

    #[test]
    fn state_document_counter_preserves_the_legacy_shallow_shape() {
        let source = concat!(
            "stateDiagram-v2\ndirection LR\naccTitle: title\naccDescr: description\n",
            "state Outer {\nstate \"Label\" as A\nA --> B: transition\n}\n",
            "classDef active fill:red\nclass A active\nstyle A color:blue\n",
            "click A href \"https://example.com\"\n",
        );
        let model = super::super::parse::parse_state_semantic_model_controlled(
            source,
            "stateDiagram-v2",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            crate::family::EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        let scalars = [
            StateStatement::Noop,
            StateStatement::AccTitle("title".to_string()),
            StateStatement::AccDescr("description".to_string()),
        ];
        for statement in model
            .document
            .iter()
            .flat_map(|(_, statements)| statements)
            .chain(&scalars)
        {
            let expected = super::super::db::stmt_to_json_shallow(statement, None);
            let fields = statement.compatibility_fields();
            assert_eq!(serde_json::to_value(&fields).unwrap(), *expected.as_value());
            assert_eq!(
                ModelComplexity::from_serializable(&fields),
                ModelComplexity::from_json(expected.as_value()),
            );
        }
    }
}
