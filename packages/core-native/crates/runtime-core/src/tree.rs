use qt_solid_widget_core::decl::NodeClass;

#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub class: NodeClass,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct NodeTree {
    nodes: std::collections::HashMap<u32, NodeRecord>,
}

impl NodeTree {
    pub fn with_root(root_id: u32) -> Self {
        let mut nodes = std::collections::HashMap::new();
        nodes.insert(
            root_id,
            NodeRecord {
                class: NodeClass::Root,
                parent: None,
                children: Vec::new(),
            },
        );
        Self { nodes }
    }

    pub fn reset_with_root(&mut self, root_id: u32) {
        *self = Self::with_root(root_id);
    }

    pub fn register(&mut self, handle: u32, class: NodeClass) -> Result<(), String> {
        if self.nodes.contains_key(&handle) {
            return Err(format!("node {handle} already registered"));
        }

        self.nodes.insert(
            handle,
            NodeRecord {
                class,
                parent: None,
                children: Vec::new(),
            },
        );
        Ok(())
    }

    pub fn contains(&self, handle: u32) -> bool {
        self.nodes.contains_key(&handle)
    }

    pub fn class(&self, handle: u32) -> Option<NodeClass> {
        self.nodes.get(&handle).map(|node| node.class)
    }

    pub fn children(&self, handle: u32) -> Option<&[u32]> {
        self.nodes.get(&handle).map(|node| node.children.as_slice())
    }

    pub fn all_handles(&self) -> Vec<u32> {
        let mut handles: Vec<u32> = self.nodes.keys().copied().collect();
        handles.sort_unstable();
        handles
    }

    pub fn get_parent(&self, handle: u32) -> Option<u32> {
        self.nodes.get(&handle).and_then(|node| node.parent)
    }

    pub fn get_first_child(&self, handle: u32) -> Option<u32> {
        self.nodes
            .get(&handle)
            .and_then(|node| node.children.first().copied())
    }

    pub fn get_next_sibling(&self, handle: u32) -> Option<u32> {
        let parent = self.get_parent(handle)?;
        let siblings = &self.nodes.get(&parent)?.children;
        let index = siblings.iter().position(|child| *child == handle)?;
        siblings.get(index + 1).copied()
    }

    pub fn insert_child(
        &mut self,
        parent: u32,
        child: u32,
        anchor: Option<u32>,
    ) -> Result<(), String> {
        if !self.nodes.contains_key(&parent) {
            return Err(format!("parent {parent} not found"));
        }
        if !self.nodes.contains_key(&child) {
            return Err(format!("child {child} not found"));
        }

        if let Some(anchor_id) = anchor {
            let parent_record = self
                .nodes
                .get(&parent)
                .ok_or_else(|| format!("parent {parent} not found"))?;
            if !parent_record.children.contains(&anchor_id) {
                return Err(format!(
                    "anchor {anchor_id} is not attached to parent {parent}"
                ));
            }
        }

        if parent == child {
            return Err("cannot insert a node into itself".to_owned());
        }

        if self.subtree_handles(child)?.contains(&parent) {
            return Err(format!(
                "cannot insert parent {parent} into descendant subtree of child {child}"
            ));
        }

        if let Some(old_parent) = self.get_parent(child) {
            if let Some(old_parent_record) = self.nodes.get_mut(&old_parent) {
                old_parent_record
                    .children
                    .retain(|candidate| *candidate != child);
            }
        }

        let insert_at = {
            let parent_record = self
                .nodes
                .get(&parent)
                .ok_or_else(|| format!("parent {parent} not found"))?;
            anchor
                .and_then(|anchor_id| {
                    parent_record
                        .children
                        .iter()
                        .position(|id| *id == anchor_id)
                })
                .unwrap_or(parent_record.children.len())
        };

        let parent_record = self
            .nodes
            .get_mut(&parent)
            .ok_or_else(|| format!("parent {parent} not found"))?;
        parent_record.children.insert(insert_at, child);

        let child_record = self
            .nodes
            .get_mut(&child)
            .ok_or_else(|| format!("child {child} not found"))?;
        child_record.parent = Some(parent);
        Ok(())
    }

    pub fn remove_child(&mut self, parent: u32, child: u32) -> Result<(), String> {
        let parent_record = self
            .nodes
            .get_mut(&parent)
            .ok_or_else(|| format!("parent {parent} not found"))?;

        let before_len = parent_record.children.len();
        parent_record
            .children
            .retain(|candidate| *candidate != child);
        if parent_record.children.len() == before_len {
            return Err(format!("child {child} not attached to parent {parent}"));
        }

        let child_record = self
            .nodes
            .get_mut(&child)
            .ok_or_else(|| format!("child {child} not found"))?;
        child_record.parent = None;
        Ok(())
    }

    pub fn subtree_handles(&self, handle: u32) -> Result<Vec<u32>, String> {
        if !self.nodes.contains_key(&handle) {
            return Err(format!("node {handle} not found"));
        }

        let mut ordered = Vec::new();
        self.collect_subtree(handle, &mut ordered)?;
        Ok(ordered)
    }

    pub fn remove_subtree(&mut self, handle: u32) -> Result<Vec<u32>, String> {
        let ordered = self.subtree_handles(handle)?;

        if let Some(parent) = self.get_parent(handle) {
            let parent_record = self
                .nodes
                .get_mut(&parent)
                .ok_or_else(|| format!("parent {parent} not found"))?;
            parent_record
                .children
                .retain(|candidate| *candidate != handle);
        }

        for id in &ordered {
            self.nodes.remove(id);
        }

        Ok(ordered)
    }

    fn collect_subtree(&self, handle: u32, ordered: &mut Vec<u32>) -> Result<(), String> {
        let node = self
            .nodes
            .get(&handle)
            .ok_or_else(|| format!("node {handle} not found"))?;

        ordered.push(handle);
        for child in &node.children {
            self.collect_subtree(*child, ordered)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use qt_solid_widget_core::decl::{NodeClass, WidgetTypeId};

    use super::NodeTree;

    #[test]
    fn insert_before_anchor_updates_sibling_chain() {
        let mut tree = NodeTree::with_root(1);
        tree.register(2, NodeClass::Widget(WidgetTypeId::new(1)))
            .unwrap();
        tree.register(3, NodeClass::Widget(WidgetTypeId::new(1)))
            .unwrap();
        tree.register(4, NodeClass::Widget(WidgetTypeId::new(1)))
            .unwrap();

        tree.insert_child(1, 2, None).unwrap();
        tree.insert_child(1, 4, None).unwrap();
        tree.insert_child(1, 3, Some(4)).unwrap();

        assert_eq!(tree.get_first_child(1), Some(2));
        assert_eq!(tree.get_next_sibling(2), Some(3));
        assert_eq!(tree.get_next_sibling(3), Some(4));
        assert_eq!(tree.get_next_sibling(4), None);
    }
}
