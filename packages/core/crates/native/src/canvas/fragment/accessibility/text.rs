use accesskit::{Node, NodeId, Role, TextPosition, TextSelection};
use unicode_segmentation::UnicodeSegmentation;

use super::super::types::FragmentId;

// ---------------------------------------------------------------------------
// NodeId namespace for synthetic TextRun nodes
// ---------------------------------------------------------------------------

/// Derive a NodeId for a virtual TextRun child from its parent fragment id.
/// Upper bit acts as a namespace to avoid collision with real fragment ids.
pub(crate) fn text_run_node_id(parent: FragmentId) -> NodeId {
    NodeId((parent.0 as u64) | (1u64 << 32))
}

// ---------------------------------------------------------------------------
// Grapheme-cluster helpers
// ---------------------------------------------------------------------------

/// UTF-8 byte length of each grapheme cluster in `text`.
pub(crate) fn compute_character_lengths(text: &str) -> Vec<u8> {
    text.graphemes(true).map(|g| g.len() as u8).collect()
}

/// Character indices (grapheme cluster indices) where words begin.
pub(crate) fn compute_word_starts(text: &str) -> Vec<u8> {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let mut result = Vec::new();
    let mut char_idx: usize = 0;
    let mut byte_offset: usize = 0;

    for (word_byte_start, _) in text.unicode_word_indices() {
        while byte_offset < word_byte_start && char_idx < graphemes.len() {
            byte_offset += graphemes[char_idx].len();
            char_idx += 1;
        }
        if byte_offset == word_byte_start {
            result.push(char_idx as u8);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Cursor position conversion (TextInput)
// ---------------------------------------------------------------------------

/// Character positions (x coords) from `cursor_x_positions`.
pub(crate) fn compute_character_positions(cursor_positions: &[f64]) -> Vec<f32> {
    cursor_positions.iter().map(|&p| p as f32).collect()
}

/// Character widths derived from adjacent cursor positions.
pub(crate) fn compute_character_widths(cursor_positions: &[f64]) -> Vec<f32> {
    cursor_positions
        .windows(2)
        .map(|w| (w[1] - w[0]) as f32)
        .chain(std::iter::once(0.0f32))
        .collect()
}

/// Convert a UTF-16 code-unit offset to a grapheme cluster index.
pub(crate) fn utf16_offset_to_char_index(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0usize;
    for (idx, grapheme) in text.graphemes(true).enumerate() {
        if utf16_count >= utf16_offset {
            return idx;
        }
        utf16_count += grapheme.chars().map(|c| c.len_utf16()).sum::<usize>();
    }
    text.graphemes(true).count()
}

/// Convert a grapheme cluster index back to a UTF-16 code-unit offset.
pub(crate) fn char_index_to_utf16_offset(text: &str, char_index: usize) -> usize {
    text.graphemes(true)
        .take(char_index)
        .flat_map(|g| g.chars())
        .map(|c| c.len_utf16())
        .sum()
}

/// Extract the parent FragmentId from a NodeId, handling both real fragment
/// nodes and synthetic TextRun nodes (upper bit namespace).
pub(crate) fn node_id_to_fragment_id(node_id: NodeId) -> Option<FragmentId> {
    let raw = node_id.0;
    if raw == u64::MAX {
        return None; // virtual root
    }
    // Strip TextRun namespace bit if present
    let frag_raw = (raw & 0xFFFF_FFFF) as u32;
    Some(FragmentId(frag_raw))
}

// ---------------------------------------------------------------------------
// TextRun node builder
// ---------------------------------------------------------------------------

/// Build a `Role::TextRun` accesskit node for a text string.
///
/// `character_positions` and `character_widths` are optional — only available
/// for TextInput (from cursor_x_positions). TextFragment skips them.
pub(crate) fn build_text_run_node(
    text: &str,
    bounds: Option<accesskit::Rect>,
    character_positions: Option<Vec<f32>>,
    character_widths: Option<Vec<f32>>,
) -> Node {
    let mut node = Node::new(Role::TextRun);
    node.set_value(text.to_owned());
    node.set_character_lengths(compute_character_lengths(text));
    node.set_word_starts(compute_word_starts(text));

    if let Some(rect) = bounds {
        node.set_bounds(rect);
    }
    if let Some(positions) = character_positions {
        node.set_character_positions(positions);
    }
    if let Some(widths) = character_widths {
        node.set_character_widths(widths);
    }

    node
}

/// Build `TextSelection` for a TextInput given cursor_pos and selection_anchor
/// (both in UTF-16 code units). Returns `None` when text is empty.
pub(crate) fn build_text_selection(
    text: &str,
    cursor_pos: f64,
    selection_anchor: f64,
    text_run_id: NodeId,
) -> Option<TextSelection> {
    if text.is_empty() {
        return None;
    }

    let focus_idx = utf16_offset_to_char_index(text, cursor_pos as usize);
    let anchor_idx = if selection_anchor < 0.0 {
        focus_idx
    } else {
        utf16_offset_to_char_index(text, selection_anchor as usize)
    };

    Some(TextSelection {
        anchor: TextPosition {
            node: text_run_id,
            character_index: anchor_idx,
        },
        focus: TextPosition {
            node: text_run_id,
            character_index: focus_idx,
        },
    })
}
