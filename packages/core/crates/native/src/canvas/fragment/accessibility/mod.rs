use std::collections::HashSet;

use accesskit::{Action, Node, NodeId, Rect as AkRect, Role, Tree, TreeId, TreeUpdate};

pub(crate) mod text;

use super::super::vello::peniko::kurbo::Rect as KurboRect;
use super::node::{FragmentData, FragmentNode, SemanticsData};
use super::tree::FragmentTree;
use super::types::FragmentId;
use text::{
    build_text_run_node, build_text_selection, compute_character_positions,
    compute_character_widths, text_run_node_id,
};

/// Virtual root node ID for the accesskit tree (window-level container).
/// Uses u64::MAX to avoid collision with fragment IDs.
const AK_ROOT_ID: NodeId = NodeId(u64::MAX);

fn frag_to_node_id(id: FragmentId) -> NodeId {
    NodeId(id.0 as u64)
}

fn to_ak_rect(r: &KurboRect) -> AkRect {
    AkRect {
        x0: r.x0,
        y0: r.y0,
        x1: r.x1,
        y1: r.y1,
    }
}

/// Whether a fragment node should appear in the accessibility tree by itself.
fn node_has_a11y(node: &FragmentNode) -> bool {
    node.semantics.is_some() || (node.props.visible && node.props.focusable)
}

/// Whether a fragment node should appear in the accessibility tree, either
/// because it has explicit semantics, can receive focus, or because it has
/// descendant(s) that do.
fn subtree_has_a11y(
    node: &FragmentNode,
    nodes: &std::collections::HashMap<FragmentId, FragmentNode>,
) -> bool {
    if node_has_a11y(node) {
        return true;
    }
    node.children.iter().any(|cid| {
        nodes
            .get(cid)
            .map_or(false, |child| subtree_has_a11y(child, nodes))
    })
}

/// Build an `accesskit::Node` from a `FragmentNode`.
///
/// Returns the main node plus any synthetic child nodes (TextRun children
/// for text-bearing fragments).
fn build_ak_node(frag: &FragmentNode, a11y_children: &[NodeId]) -> (Node, Vec<(NodeId, Node)>) {
    let role = frag.semantics.as_ref().map_or(Role::Group, |s| s.role);

    let mut ak = Node::new(role);
    let bounds = frag.world_aabb.map(|r| to_ak_rect(&r));

    // Attempt to create TextRun children for text-bearing fragments.
    let mut synthetic: Vec<(NodeId, Node)> = Vec::new();
    let mut extra_children: Vec<NodeId> = Vec::new();

    match &frag.kind {
        FragmentData::Text(t) if !t.text.is_empty() => {
            let run_id = text_run_node_id(frag.id);
            let run = build_text_run_node(&t.text, bounds, None, None);
            extra_children.push(run_id);
            synthetic.push((run_id, run));
        }
        FragmentData::TextInput(t) if !t.text.is_empty() => {
            let run_id = text_run_node_id(frag.id);

            // Derive character positions/widths from cursor_x_positions if available.
            let (positions, widths) = t
                .layout
                .as_ref()
                .filter(|l| !l.cursor_x_positions.is_empty())
                .map(|l| {
                    (
                        compute_character_positions(&l.cursor_x_positions),
                        compute_character_widths(&l.cursor_x_positions),
                    )
                })
                .unzip();

            let run = build_text_run_node(&t.text, bounds, positions, widths);
            extra_children.push(run_id);
            synthetic.push((run_id, run));

            // Text selection on the container.
            if let Some(sel) =
                build_text_selection(&t.text, t.cursor_pos, t.selection_anchor, run_id)
            {
                ak.set_text_selection(sel);
            }
            ak.add_action(Action::SetTextSelection);
        }
        _ => {}
    }

    if frag.props.visible && frag.props.focusable {
        ak.add_action(Action::Focus);
    }

    // Merge real a11y children + synthetic TextRun children.
    let mut all_children = a11y_children.to_vec();
    all_children.extend(extra_children);
    ak.set_children(all_children);

    if let Some(rect) = bounds {
        ak.set_bounds(rect);
    }

    if !frag.props.visible {
        ak.set_hidden();
    }

    if let Some(sem) = &frag.semantics {
        apply_semantics(&mut ak, sem, frag);
    } else {
        auto_infer_text(&mut ak, frag);
    }

    (ak, synthetic)
}

fn apply_semantics(ak: &mut Node, sem: &SemanticsData, frag: &FragmentNode) {
    if let Some(ref label) = sem.label {
        ak.set_label(label.clone());
    }
    if let Some(ref value) = sem.value {
        ak.set_value(value.clone());
    }
    if let Some(ref desc) = sem.description {
        ak.set_description(desc.clone());
    }
    if let Some(live) = sem.live {
        ak.set_live(live);
    }
    if let Some(toggled) = sem.checked {
        ak.set_toggled(toggled);
    }
    if let Some(expanded) = sem.expanded {
        ak.set_expanded(expanded);
    }
    if let Some(selected) = sem.selected {
        ak.set_selected(selected);
    }
    if sem.disabled {
        ak.set_disabled();
    }

    // Auto-infer label/value from text content when semantics doesn't provide one.
    auto_infer_text_with_semantics(ak, sem, frag);
}

fn auto_infer_text_with_semantics(ak: &mut Node, sem: &SemanticsData, frag: &FragmentNode) {
    match &frag.kind {
        FragmentData::Text(t) if sem.label.is_none() && !t.text.is_empty() => {
            ak.set_label(t.text.clone());
        }
        FragmentData::TextInput(t) if sem.value.is_none() && !t.text.is_empty() => {
            ak.set_value(t.text.clone());
        }
        _ => {}
    }
}

fn auto_infer_text(ak: &mut Node, frag: &FragmentNode) {
    match &frag.kind {
        FragmentData::Text(t) if !t.text.is_empty() => {
            ak.set_label(t.text.clone());
        }
        FragmentData::TextInput(t) if !t.text.is_empty() => {
            ak.set_value(t.text.clone());
        }
        _ => {}
    }
}

impl FragmentTree {
    /// Build a full accesskit `TreeUpdate` from the current fragment tree state.
    /// Used for initial tree and full rebuilds.
    pub fn build_full_accesskit_update(&self) -> TreeUpdate {
        let mut out: Vec<(NodeId, Node)> = Vec::new();

        // Collect a11y-relevant root children.
        let root_a11y_children: Vec<NodeId> = self
            .root_children
            .iter()
            .filter(|id| {
                self.nodes
                    .get(id)
                    .map_or(false, |n| subtree_has_a11y(n, &self.nodes))
            })
            .map(|id| frag_to_node_id(*id))
            .collect();

        // Virtual root node.
        let mut root_node = Node::new(Role::Window);
        root_node.set_children(root_a11y_children.clone());
        out.push((AK_ROOT_ID, root_node));

        // Walk all relevant subtrees.
        for id in &self.root_children {
            if let Some(node) = self.nodes.get(id) {
                if subtree_has_a11y(node, &self.nodes) {
                    self.collect_a11y_nodes(*id, &mut out);
                }
            }
        }

        let focus = self.accesskit_focus();

        TreeUpdate {
            nodes: out,
            tree: Some(Tree::new(AK_ROOT_ID)),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    /// Build an incremental accesskit `TreeUpdate` containing only nodes
    /// whose semantics or bounds changed.
    pub fn build_incremental_accesskit_update(
        &self,
        dirty_ids: &HashSet<FragmentId>,
    ) -> Option<TreeUpdate> {
        if dirty_ids.is_empty() {
            return None;
        }

        let mut affected: HashSet<FragmentId> = HashSet::new();
        let mut root_touched = false;

        for &id in dirty_ids {
            affected.insert(id);
            // Include parent so its children list is up-to-date.
            if let Some(node) = self.nodes.get(&id) {
                if let Some(parent) = node.parent {
                    affected.insert(parent);
                } else {
                    root_touched = true;
                }
            } else {
                // Node removed — parent is affected.
                root_touched = true;
            }
        }

        let mut out: Vec<(NodeId, Node)> = Vec::new();

        if root_touched {
            let root_a11y_children: Vec<NodeId> = self
                .root_children
                .iter()
                .filter(|id| {
                    self.nodes
                        .get(id)
                        .map_or(false, |n| subtree_has_a11y(n, &self.nodes))
                })
                .map(|id| frag_to_node_id(*id))
                .collect();

            let mut root_node = Node::new(Role::Window);
            root_node.set_children(root_a11y_children);
            out.push((AK_ROOT_ID, root_node));
        }

        for &id in &affected {
            if let Some(frag) = self.nodes.get(&id) {
                let a11y_children = self.a11y_children_of(frag);
                let (ak, synthetic) = build_ak_node(frag, &a11y_children);
                out.push((frag_to_node_id(id), ak));
                out.extend(synthetic);
            }
        }

        if out.is_empty() {
            return None;
        }

        let focus = self.accesskit_focus();

        Some(TreeUpdate {
            nodes: out,
            tree: None,
            tree_id: TreeId::ROOT,
            focus,
        })
    }

    // -- private helpers -----------------------------------------------------

    /// Recursively collect all a11y-relevant nodes under `id` into `out`.
    fn collect_a11y_nodes(&self, id: FragmentId, out: &mut Vec<(NodeId, Node)>) {
        let Some(frag) = self.nodes.get(&id) else {
            return;
        };

        let a11y_children = self.a11y_children_of(frag);
        let (ak, synthetic) = build_ak_node(frag, &a11y_children);
        out.push((frag_to_node_id(id), ak));
        out.extend(synthetic);

        for &child_id in &frag.children {
            if let Some(child) = self.nodes.get(&child_id) {
                if subtree_has_a11y(child, &self.nodes) {
                    self.collect_a11y_nodes(child_id, out);
                }
            }
        }
    }

    /// Return the NodeId list of `frag`'s children that participate in a11y.
    fn a11y_children_of(&self, frag: &FragmentNode) -> Vec<NodeId> {
        frag.children
            .iter()
            .filter(|cid| {
                self.nodes
                    .get(cid)
                    .map_or(false, |child| subtree_has_a11y(child, &self.nodes))
            })
            .map(|cid| frag_to_node_id(*cid))
            .collect()
    }

    fn accesskit_focus(&self) -> NodeId {
        let Some(focused) = self.focused else {
            return AK_ROOT_ID;
        };
        if self
            .root_children
            .iter()
            .any(|id| self.subtree_contains_a11y_node(*id, focused))
        {
            frag_to_node_id(focused)
        } else {
            AK_ROOT_ID
        }
    }

    fn subtree_contains_a11y_node(&self, id: FragmentId, target: FragmentId) -> bool {
        let Some(node) = self.nodes.get(&id) else {
            return false;
        };
        if id == target {
            return node_has_a11y(node);
        }
        node.children
            .iter()
            .any(|child_id| self.subtree_contains_a11y_node(*child_id, target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::fragment::{FragmentData, FragmentTree};

    fn node_ids(update: &TreeUpdate) -> Vec<NodeId> {
        update.nodes.iter().map(|(id, _)| *id).collect()
    }

    #[test]
    fn full_update_includes_focused_focusable_node_without_semantics() {
        let mut tree = FragmentTree::new();
        let id = tree.create_node(FragmentData::Rect(Default::default()));
        tree.nodes.get_mut(&id).unwrap().props.focusable = true;
        tree.insert_child(None, id, None);
        tree.focused = Some(id);

        let update = tree.build_full_accesskit_update();
        let ids = node_ids(&update);

        assert!(ids.contains(&AK_ROOT_ID));
        assert!(ids.contains(&frag_to_node_id(id)));
        assert_eq!(update.focus, frag_to_node_id(id));
    }

    #[test]
    fn full_update_falls_back_to_root_when_focus_is_detached() {
        let mut tree = FragmentTree::new();
        let id = tree.create_node(FragmentData::Rect(Default::default()));
        tree.nodes.get_mut(&id).unwrap().props.focusable = true;
        tree.insert_child(None, id, None);
        tree.focused = Some(id);

        tree.detach_child(None, id);

        let update = tree.build_full_accesskit_update();
        let ids = node_ids(&update);

        assert!(ids.contains(&AK_ROOT_ID));
        assert!(!ids.contains(&frag_to_node_id(id)));
        assert_eq!(update.focus, AK_ROOT_ID);
    }

    #[test]
    fn full_update_falls_back_to_root_when_focus_is_removed() {
        let mut tree = FragmentTree::new();
        let id = tree.create_node(FragmentData::Rect(Default::default()));
        tree.nodes.get_mut(&id).unwrap().props.focusable = true;
        tree.insert_child(None, id, None);
        tree.focused = Some(id);

        tree.remove(id);

        let update = tree.build_full_accesskit_update();
        let ids = node_ids(&update);

        assert!(ids.contains(&AK_ROOT_ID));
        assert!(!ids.contains(&frag_to_node_id(id)));
        assert_eq!(update.focus, AK_ROOT_ID);
    }
}
