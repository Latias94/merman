use crate::{Result, SourcePresentationMetadata};
use merman_model::families::zenuml::ZenumlDiagramModel;
use serde_json::Value;

pub(crate) fn render_model_to_compat_json(
    model: &ZenumlDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::zenuml::compat::project(model, &meta.compat_json_metadata())
        .map_err(Into::into)
}

#[cfg(feature = "editor-facts")]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &ZenumlDiagramModel,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Value>> {
    Ok(merman_model::families::zenuml::compat::project_controlled(
        model,
        &meta.compat_json_metadata(),
        control,
    )?
    .map_err(Into::into))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MermaidConfig, OperationControl};
    use merman_model::families::zenuml::{
        ZenumlGroup, ZenumlParticipant, ZenumlStatement, model_to_typed_json_controlled,
    };
    use serde::Serialize;
    use serde_json::json;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct LegacyModelWire<'a> {
        title: &'a Option<String>,
        starter: &'a Option<String>,
        participants: &'a [ZenumlParticipant],
        groups: &'a [ZenumlGroup],
        statements: &'a [ZenumlStatement],
    }

    #[test]
    fn iterative_projection_preserves_serde_fields_and_typed_wire_order() {
        let source = concat!(
            "zenuml\n@Actor A 120\n@Starter(A)\n",
            "A.call(){\n// closing comment\n}\nnew B\nreturn okay\n",
            "ref(Label,A,B)\n== Divider ==\nif(ok){B.call()}else{A.call()}\n",
        );
        let built = super::super::construct_semantic_source(
            source,
            #[cfg(feature = "editor-facts")]
            crate::family::EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(
            built.first_diagnostic.is_none(),
            "{:?}",
            built.first_diagnostic
        );
        let meta = SourcePresentationMetadata {
            diagram_type: "zenuml".into(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let legacy = LegacyModelWire {
            title: &built.model.title,
            starter: &built.model.starter,
            participants: &built.model.participants,
            groups: &built.model.groups,
            statements: &built.model.statements,
        };
        let mut expected = serde_json::to_value(&legacy).unwrap();
        let canonical =
            model_to_typed_json_controlled(&built.model, &crate::OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(canonical.as_value(), &expected);
        assert!(expected.get("type").is_none());
        assert_eq!(expected["participants"][0]["widthSource"], "120");
        assert!(expected["participants"][0].get("width").is_none());
        assert!(expected["statements"][0].get("explicit_from").is_some());
        for participant in expected["participants"].as_array_mut().unwrap() {
            let object = participant.as_object_mut().unwrap();
            let width = match object
                .remove("widthSource")
                .as_ref()
                .and_then(Value::as_str)
            {
                Some("120") => json!(120.0),
                None => Value::Null,
                other => panic!("unexpected fixture width {other:?}"),
            };
            object.insert("width".into(), width);
        }
        expected
            .as_object_mut()
            .unwrap()
            .insert("type".into(), json!("zenuml"));
        let projected = render_model_to_compat_json(&built.model, &meta).unwrap();
        assert_eq!(projected, expected);

        assert_eq!(
            serde_json::to_string(&built.model).unwrap(),
            serde_json::to_string(&legacy).unwrap(),
            "typed ZenUML serialization must retain legacy field order"
        );
    }
}
