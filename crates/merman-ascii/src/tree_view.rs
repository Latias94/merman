use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    BudgetedTextDocument, push_optional_document_field, push_wrapped_field, try_clone_layout_text,
    try_concat_layout_text, try_repeat_layout_char,
};
use crate::text::display_width_with_profile;
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};
use std::collections::HashSet;

const SUMMARY_WRAP_WIDTH: usize = 80;

#[derive(Clone, Copy)]
struct TreeViewChars {
    branch: &'static str,
    last_branch: &'static str,
    child_continue: &'static str,
    child_empty: &'static str,
}

impl TreeViewChars {
    fn from_options(options: &AsciiRenderOptions) -> Self {
        match options.structural_charset() {
            AsciiCharset::Ascii => Self {
                branch: "|-- ",
                last_branch: "\\-- ",
                child_continue: "|   ",
                child_empty: "    ",
            },
            AsciiCharset::Unicode => Self {
                branch: "├── ",
                last_branch: "└── ",
                child_continue: "│   ",
                child_empty: "    ",
            },
        }
    }
}

pub(super) fn render_tree_view_diagram(
    model: &TreeViewDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    validate_tree_view_model(&model.root, document.resources_mut())?;
    let chars = TreeViewChars::from_options(options);
    push_optional_document_field(&mut document, "title", model.title.as_deref())?;
    push_optional_document_field(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_document_field(&mut document, "accDescr", model.acc_descr.as_deref())?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    push_wrapped_node(&mut document, "", &model.root, options)?;

    let mut stack = Vec::new();
    for (index, child) in model.root.children.iter().enumerate().rev() {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        push_frame(
            &mut stack,
            TreeFrame {
                node: child,
                prefix: String::new(),
                is_last: index + 1 == model.root.children.len(),
                depth: 1,
            },
            document.resources_mut(),
        )?;
    }

    while let Some(frame) = stack.pop() {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        let TreeFrame {
            node,
            prefix,
            is_last,
            depth,
        } = frame;
        let branch = try_concat_layout_text(
            &prefix,
            if is_last {
                chars.last_branch
            } else {
                chars.branch
            },
            document.resources_mut(),
        )?;
        push_wrapped_node(&mut document, &branch, node, options)?;

        if node.children.is_empty() {
            continue;
        }
        let child_depth = depth.checked_add(1).ok_or_else(|| {
            document
                .resources_mut()
                .policy()
                .overflow(AsciiResourceLimitId::MaxNestingDepth)
        })?;
        document.resources_mut().check_nesting_depth(child_depth)?;
        let next_prefix = try_concat_layout_text(
            &prefix,
            if is_last {
                chars.child_empty
            } else {
                chars.child_continue
            },
            document.resources_mut(),
        )?;

        for (index, child) in node.children.iter().enumerate().rev() {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            push_frame(
                &mut stack,
                TreeFrame {
                    node: child,
                    prefix: try_clone_layout_text(&next_prefix, document.resources_mut())?,
                    is_last: index + 1 == node.children.len(),
                    depth: child_depth,
                },
                document.resources_mut(),
            )?;
        }
    }

    document.finish(options)
}

fn validate_tree_view_model(
    root: &TreeViewNodeRenderModel,
    resources: &mut ResourceContext,
) -> Result<()> {
    let mut seen_ids = HashSet::new();
    let mut stack = Vec::new();
    resources.charge_layout_work(1)?;
    stack
        .try_reserve(1)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    stack.push((root, 0usize));

    while let Some((node, depth)) = stack.pop() {
        resources.check_nesting_depth(depth)?;
        resources.charge_layout_work(1)?;
        seen_ids
            .try_reserve(1)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        if !seen_ids.insert(node.id) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "treeView",
                feature: "duplicate node ids",
            });
        }
        if !matches!(node.node_type.as_str(), "file" | "directory") {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "treeView",
                feature: "unknown node types",
            });
        }

        if node.children.is_empty() {
            continue;
        }
        let child_depth = depth.checked_add(1).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxNestingDepth)
        })?;
        resources.check_nesting_depth(child_depth)?;
        resources.charge_layout_work(node.children.len())?;
        stack
            .try_reserve(node.children.len())
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        stack.extend(node.children.iter().rev().map(|child| (child, child_depth)));
    }

    Ok(())
}

struct TreeFrame<'a> {
    node: &'a TreeViewNodeRenderModel,
    prefix: String,
    is_last: bool,
    depth: usize,
}

fn push_frame<'a>(
    stack: &mut Vec<TreeFrame<'a>>,
    frame: TreeFrame<'a>,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.check_nesting_depth(frame.depth)?;
    resources.charge_layout_work(1)?;
    stack
        .try_reserve(1)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    stack.push(frame);
    Ok(())
}

fn push_wrapped_node(
    document: &mut BudgetedTextDocument,
    prefix: &str,
    node: &TreeViewNodeRenderModel,
    options: &AsciiRenderOptions,
) -> Result<()> {
    let continuation_width = display_width_with_profile(prefix, options.terminal_width_profile);
    let continuation_prefix =
        try_repeat_layout_char(' ', continuation_width, document.resources_mut())?;
    document.push_wrapped_prefixed_line_with(
        prefix,
        &continuation_prefix,
        SUMMARY_WRAP_WIDTH,
        |line| {
            match node.node_type.as_str() {
                "file" => line.push_str("[file] ")?,
                "directory" => line.push_str("[directory] ")?,
                _ => {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "treeView",
                        feature: "unknown node types",
                    });
                }
            }
            push_wrapped_field(line, "", "name", &node.name)?;
            if node.node_type == "directory" && !node.name.ends_with('/') {
                line.push_str("/")?;
            }

            line.push_str(" [id=")?;
            line.write_fmt(format_args!("{}", node.id))?;
            line.push_str(", level=")?;
            line.write_fmt(format_args!("{}", node.level))?;

            let metadata = [
                ("icon", node.icon.as_deref()),
                ("class", node.css_class.as_deref()),
            ];
            for (key, value) in metadata
                .into_iter()
                .filter_map(|(key, value)| value.map(|value| (key, value)))
            {
                push_wrapped_field(line, ", ", key, value)?;
            }
            line.push_str("]")?;
            if let Some(description) = node.description.as_deref() {
                push_wrapped_field(line, " ", "description", description)?;
            }
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;

    const DEEP_NESTING: usize = 256;

    fn node(
        id: i64,
        name: &str,
        children: Vec<TreeViewNodeRenderModel>,
    ) -> TreeViewNodeRenderModel {
        TreeViewNodeRenderModel {
            id,
            level: id - 1,
            name: name.to_string(),
            children,
            ..Default::default()
        }
    }

    fn chain(depth: usize) -> TreeViewDiagramRenderModel {
        let mut current = node(depth as i64, "leaf", Vec::new());
        for level in (1..depth).rev() {
            current = node(level as i64, &format!("level-{level}"), vec![current]);
        }
        TreeViewDiagramRenderModel {
            root: node(0, "/", vec![current]),
            ..Default::default()
        }
    }

    fn options_with_nesting_limit(limit: usize) -> AsciiRenderOptions {
        let resources = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, limit)
            .expect("positive nesting limit");
        AsciiRenderOptions::ascii().with_resource_policy(resources)
    }

    #[test]
    fn tree_view_accepts_exact_nesting_limit() {
        let options = options_with_nesting_limit(DEEP_NESTING);
        let rendered = render_tree_view_diagram(
            &chain(DEEP_NESTING),
            &options,
            AsciiExecution::standalone(&options.resources),
        )
        .expect("deep nesting equal to the limit should render iteratively");

        assert!(rendered.contains("leaf"));
    }

    #[test]
    fn tree_view_rejects_limit_minus_one_before_descending() {
        let options = options_with_nesting_limit(DEEP_NESTING - 1);
        let error = render_tree_view_diagram(
            &chain(DEEP_NESTING),
            &options,
            AsciiExecution::standalone(&options.resources),
        )
        .expect_err("deep nesting above the limit should fail before the final descent");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == DEEP_NESTING
                    && details.max == DEEP_NESTING - 1
        ));
    }

    #[test]
    fn tree_view_rejects_duplicate_node_ids_before_rendering() {
        let model = TreeViewDiagramRenderModel {
            root: node(
                0,
                "/",
                vec![node(1, "first", Vec::new()), node(1, "second", Vec::new())],
            ),
            ..Default::default()
        };

        let options = AsciiRenderOptions::ascii();
        let error = render_tree_view_diagram(
            &model,
            &options,
            AsciiExecution::standalone(&options.resources),
        )
        .expect_err("duplicate public node identities must not be rendered ambiguously");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "treeView",
                feature: "duplicate node ids",
            }
        );
    }
}
