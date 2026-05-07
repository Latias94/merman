use super::model::SequenceSvgModel;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub(super) struct AltSection {
    pub(super) raw_label: String,
    pub(super) message_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) enum SequenceBlock {
    Alt {
        sections: Vec<AltSection>,
    },
    Opt {
        raw_label: String,
        message_ids: Vec<String>,
    },
    Break {
        raw_label: String,
        message_ids: Vec<String>,
    },
    Par {
        sections: Vec<AltSection>,
    },
    Loop {
        raw_label: String,
        message_ids: Vec<String>,
    },
    Critical {
        sections: Vec<AltSection>,
    },
}

#[derive(Debug, Clone)]
enum BlockStackEntry {
    Alt {
        raw_labels: Vec<String>,
        sections: Vec<Vec<String>>,
    },
    Loop {
        raw_label: String,
        messages: Vec<String>,
    },
    Opt {
        raw_label: String,
        messages: Vec<String>,
    },
    Break {
        raw_label: String,
        messages: Vec<String>,
    },
    Par {
        raw_labels: Vec<String>,
        sections: Vec<Vec<String>>,
    },
    Critical {
        raw_labels: Vec<String>,
        sections: Vec<Vec<String>>,
    },
}

pub(super) fn collect_sequence_blocks(
    model: &SequenceSvgModel,
) -> (FxHashMap<String, Vec<usize>>, Vec<SequenceBlock>) {
    // Mermaid renders block frames (`alt`, `loop`, ...) as `<g>` elements before message lines.
    // Use layout-derived message y-coordinates for separator placement to avoid visual artifacts
    // like dashed lines ending in a gap right before the frame border.
    let mut blocks_by_end_id: FxHashMap<String, Vec<usize>> =
        FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());
    let mut blocks: Vec<SequenceBlock> = Vec::new();
    let mut stack: Vec<BlockStackEntry> = Vec::new();

    for msg in &model.messages {
        let raw_label = msg.message_text();
        match msg.message_type {
            // notes
            2 => {
                // Notes inside blocks must contribute to block frame bounds and section separators.
                // Track them in the active block scopes, similar to message edges.
                for entry in stack.iter_mut() {
                    push_item_to_block_stack_entry(entry, &msg.id);
                }
                continue;
            }
            // loop start/end
            10 => stack.push(BlockStackEntry::Loop {
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            11 => {
                if let Some(BlockStackEntry::Loop {
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    push_block(
                        &mut blocks_by_end_id,
                        &mut blocks,
                        msg.id.clone(),
                        SequenceBlock::Loop {
                            raw_label,
                            message_ids: messages,
                        },
                    );
                }
            }
            // opt start/end
            15 => stack.push(BlockStackEntry::Opt {
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            16 => {
                if let Some(BlockStackEntry::Opt {
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    push_block(
                        &mut blocks_by_end_id,
                        &mut blocks,
                        msg.id.clone(),
                        SequenceBlock::Opt {
                            raw_label,
                            message_ids: messages,
                        },
                    );
                }
            }
            // break start/end
            30 => stack.push(BlockStackEntry::Break {
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            31 => {
                if let Some(BlockStackEntry::Break {
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    push_block(
                        &mut blocks_by_end_id,
                        &mut blocks,
                        msg.id.clone(),
                        SequenceBlock::Break {
                            raw_label,
                            message_ids: messages,
                        },
                    );
                }
            }
            // alt start/else/end
            12 => stack.push(BlockStackEntry::Alt {
                raw_labels: vec![raw_label.to_string()],
                sections: vec![Vec::new()],
            }),
            13 => {
                if let Some(BlockStackEntry::Alt {
                    raw_labels,
                    sections,
                }) = stack.last_mut()
                {
                    raw_labels.push(raw_label.to_string());
                    sections.push(Vec::new());
                }
            }
            14 => {
                if let Some(BlockStackEntry::Alt {
                    raw_labels,
                    sections,
                }) = stack.pop()
                {
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Alt {
                        sections: into_alt_sections(raw_labels, sections),
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            // par start/and/end
            19 | 32 => stack.push(BlockStackEntry::Par {
                raw_labels: vec![raw_label.to_string()],
                sections: vec![Vec::new()],
            }),
            20 => {
                if let Some(BlockStackEntry::Par {
                    raw_labels,
                    sections,
                }) = stack.last_mut()
                {
                    raw_labels.push(raw_label.to_string());
                    sections.push(Vec::new());
                }
            }
            21 => {
                if let Some(BlockStackEntry::Par {
                    raw_labels,
                    sections,
                }) = stack.pop()
                {
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Par {
                        sections: into_alt_sections(raw_labels, sections),
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            // critical start/option/end
            27 => stack.push(BlockStackEntry::Critical {
                raw_labels: vec![raw_label.to_string()],
                sections: vec![Vec::new()],
            }),
            28 => {
                if let Some(BlockStackEntry::Critical {
                    raw_labels,
                    sections,
                }) = stack.last_mut()
                {
                    raw_labels.push(raw_label.to_string());
                    sections.push(Vec::new());
                }
            }
            29 => {
                if let Some(BlockStackEntry::Critical {
                    raw_labels,
                    sections,
                }) = stack.pop()
                {
                    let idx = blocks.len();
                    blocks.push(SequenceBlock::Critical {
                        sections: into_alt_sections(raw_labels, sections),
                    });
                    blocks_by_end_id
                        .entry(msg.id.clone())
                        .or_default()
                        .push(idx);
                }
            }
            _ => {
                // If this is a "real" message edge, attach it to all active block scopes.
                if msg.from.is_some() && msg.to.is_some() {
                    for entry in stack.iter_mut() {
                        push_item_to_block_stack_entry(entry, &msg.id);
                    }
                }
            }
        }
    }

    (blocks_by_end_id, blocks)
}

fn push_block(
    blocks_by_end_id: &mut FxHashMap<String, Vec<usize>>,
    blocks: &mut Vec<SequenceBlock>,
    end_id: String,
    block: SequenceBlock,
) {
    let idx = blocks.len();
    blocks.push(block);
    blocks_by_end_id.entry(end_id).or_default().push(idx);
}

fn push_item_to_block_stack_entry(entry: &mut BlockStackEntry, item_id: &str) {
    match entry {
        BlockStackEntry::Alt { sections, .. }
        | BlockStackEntry::Par { sections, .. }
        | BlockStackEntry::Critical { sections, .. } => {
            if let Some(cur) = sections.last_mut() {
                cur.push(item_id.to_string());
            }
        }
        BlockStackEntry::Loop { messages, .. }
        | BlockStackEntry::Opt { messages, .. }
        | BlockStackEntry::Break { messages, .. } => {
            messages.push(item_id.to_string());
        }
    }
}

fn into_alt_sections(raw_labels: Vec<String>, sections: Vec<Vec<String>>) -> Vec<AltSection> {
    let mut out_sections = Vec::new();
    for (i, raw_label) in raw_labels.into_iter().enumerate() {
        let message_ids = sections.get(i).cloned().unwrap_or_default();
        out_sections.push(AltSection {
            raw_label,
            message_ids,
        });
    }
    out_sections
}
