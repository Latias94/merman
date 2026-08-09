use crate::Result;
use crate::error::AsciiError;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::BudgetedTextDocument;
use crate::text::display_width_with_profile;
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

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

pub fn render_tree_view_diagram(
    model: &TreeViewDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    let chars = TreeViewChars::from_options(options);
    document.push_optional_line(model.title.as_deref())?;
    document.push_optional_prefixed_line("accTitle: ", model.acc_title.as_deref())?;
    document.push_optional_prefixed_line("accDescr: ", model.acc_descr.as_deref())?;

    let mut stack = Vec::new();
    for (index, child) in model.root.children.iter().enumerate().rev() {
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
        let TreeFrame {
            node,
            prefix,
            is_last,
            depth,
        } = frame;
        let branch = if prefix.is_empty() {
            if is_last {
                chars.last_branch.to_string()
            } else {
                chars.branch.to_string()
            }
        } else if is_last {
            format!("{prefix}{}", chars.last_branch)
        } else {
            format!("{prefix}{}", chars.branch)
        };
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
        let next_prefix = if prefix.is_empty() {
            if is_last {
                chars.child_empty.to_string()
            } else {
                chars.child_continue.to_string()
            }
        } else if is_last {
            format!("{prefix}{}", chars.child_empty)
        } else {
            format!("{prefix}{}", chars.child_continue)
        };

        for (index, child) in node.children.iter().enumerate().rev() {
            document
                .resources_mut()
                .charge_layout_work(next_prefix.len())?;
            push_frame(
                &mut stack,
                TreeFrame {
                    node: child,
                    prefix: next_prefix.clone(),
                    is_last: index + 1 == node.children.len(),
                    depth: child_depth,
                },
                document.resources_mut(),
            )?;
        }
    }

    document.finish(options)
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
    document
        .resources_mut()
        .charge_layout_work(continuation_width)?;
    let continuation_prefix = " ".repeat(continuation_width);
    document.push_wrapped_prefixed_line_with(
        prefix,
        &continuation_prefix,
        SUMMARY_WRAP_WIDTH,
        |line| {
            line.push_str(&node.name)?;
            match node.node_type.as_str() {
                "directory" if !node.name.ends_with('/') => line.push_str("/")?,
                "file" | "directory" => {}
                node_type => {
                    line.push_str(" [type=")?;
                    line.push_str(node_type)?;
                    line.push_str("]")?;
                }
            }

            let metadata = [
                ("icon=", node.icon.as_deref()),
                ("class=", node.css_class.as_deref()),
            ];
            let mut emitted = false;
            for (key, value) in metadata
                .into_iter()
                .filter_map(|(key, value)| value.map(|value| (key, value)))
            {
                line.push_str(if emitted { ", " } else { " [" })?;
                line.push_str(key)?;
                line.push_str(value)?;
                emitted = true;
            }
            if emitted {
                line.push_str("]")?;
            }
            if let Some(description) = node.description.as_deref() {
                line.push_str(" -- ")?;
                line.push_str(description)?;
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
        let rendered = render_tree_view_diagram(
            &chain(DEEP_NESTING),
            &options_with_nesting_limit(DEEP_NESTING),
        )
        .expect("deep nesting equal to the limit should render iteratively");

        assert!(rendered.contains("leaf"));
    }

    #[test]
    fn tree_view_rejects_limit_minus_one_before_descending() {
        let error = render_tree_view_diagram(
            &chain(DEEP_NESTING),
            &options_with_nesting_limit(DEEP_NESTING - 1),
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
}
