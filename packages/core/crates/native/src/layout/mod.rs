pub mod ffi;
pub mod registry_ffi;
pub mod state;

use ffi::bridge::TaffyRect;
use taffy::geometry::{Rect, Size};
use taffy::prelude::*;
use taffy::style::{
    AlignItems, AlignSelf, Dimension, FlexDirection, FlexWrap, JustifyContent, LengthPercentage,
    LengthPercentageAuto, Position,
};

pub struct TaffyEngine {
    tree: TaffyTree<Size<f32>>,
    nodes: Vec<Option<NodeId>>,
    free_slots: Vec<u32>,
}

impl TaffyEngine {
    pub fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            nodes: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    fn alloc_slot(&mut self, node_id: NodeId) -> u32 {
        if let Some(slot) = self.free_slots.pop() {
            self.nodes[slot as usize] = Some(node_id);
            slot
        } else {
            let slot = self.nodes.len() as u32;
            self.nodes.push(Some(node_id));
            slot
        }
    }

    fn resolve(&self, handle: u32) -> NodeId {
        self.nodes[handle as usize].expect("invalid taffy node handle")
    }

    fn with_style_mut(&mut self, handle: u32, f: impl FnOnce(&mut Style)) {
        let node = self.resolve(handle);
        let mut style = self.tree.style(node).unwrap().clone();
        f(&mut style);
        self.tree.set_style(node, style).unwrap();
    }

    pub fn create_node(&mut self) -> u32 {
        let node_id = self
            .tree
            .new_leaf(Style {
                flex_shrink: 0.0,
                ..Style::default()
            })
            .unwrap();
        self.alloc_slot(node_id)
    }

    pub fn remove_node(&mut self, handle: u32) {
        let node_id = self.resolve(handle);
        self.tree.remove(node_id).ok();
        self.nodes[handle as usize] = None;
        self.free_slots.push(handle);
    }

    pub fn set_children(&mut self, parent: u32, children: &[u32]) {
        let parent_id = self.resolve(parent);
        let child_ids: Vec<NodeId> = children.iter().map(|&h| self.resolve(h)).collect();
        self.tree.set_children(parent_id, &child_ids).unwrap();
    }

    pub fn set_display(&mut self, handle: u32, tag: u8) {
        self.with_style_mut(handle, |s| {
            s.display = match tag {
                0 => Display::Flex,
                1 => Display::None,
                _ => Display::Flex,
            };
        });
    }

    pub fn set_flex_direction(&mut self, handle: u32, tag: u8) {
        self.with_style_mut(handle, |s| {
            s.flex_direction = match tag {
                1 => FlexDirection::Column,
                2 => FlexDirection::Row,
                _ => FlexDirection::Column,
            };
        });
    }

    pub fn set_flex_wrap(&mut self, handle: u32, tag: u8) {
        self.with_style_mut(handle, |s| {
            s.flex_wrap = match tag {
                1 => FlexWrap::NoWrap,
                2 => FlexWrap::Wrap,
                3 => FlexWrap::WrapReverse,
                _ => FlexWrap::NoWrap,
            };
        });
    }

    pub fn set_flex_grow(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.flex_grow = value);
    }

    pub fn set_flex_shrink(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.flex_shrink = value);
    }

    pub fn set_flex_basis_px(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.flex_basis = Dimension::length(value));
    }

    pub fn set_flex_basis_percent(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.flex_basis = Dimension::percent(value));
    }

    pub fn set_flex_basis_auto(&mut self, handle: u32) {
        self.with_style_mut(handle, |s| s.flex_basis = Dimension::auto());
    }

    pub fn set_width_px(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.size.width = Dimension::length(value));
    }

    pub fn set_width_percent(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.size.width = Dimension::percent(value));
    }

    pub fn set_width_auto(&mut self, handle: u32) {
        self.with_style_mut(handle, |s| s.size.width = Dimension::auto());
    }

    pub fn set_height_px(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.size.height = Dimension::length(value));
    }

    pub fn set_height_percent(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.size.height = Dimension::percent(value));
    }

    pub fn set_height_auto(&mut self, handle: u32) {
        self.with_style_mut(handle, |s| s.size.height = Dimension::auto());
    }

    pub fn set_min_width_px(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.min_size.width = Dimension::length(value));
    }

    pub fn set_min_height_px(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.min_size.height = Dimension::length(value));
    }

    pub fn set_max_width_px(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.max_size.width = Dimension::length(value));
    }

    pub fn set_max_height_px(&mut self, handle: u32, value: f32) {
        self.with_style_mut(handle, |s| s.max_size.height = Dimension::length(value));
    }

    pub fn set_align_self(&mut self, handle: u32, tag: u8) {
        self.with_style_mut(handle, |s| {
            s.align_self = match tag {
                1 => None, // Auto
                2 => Some(AlignSelf::FlexStart),
                3 => Some(AlignSelf::FlexEnd),
                4 => Some(AlignSelf::Center),
                5 => Some(AlignSelf::Stretch),
                _ => None,
            };
        });
    }

    pub fn set_align_items(&mut self, handle: u32, tag: u8) {
        self.with_style_mut(handle, |s| {
            s.align_items = match tag {
                1 => Some(AlignItems::FlexStart),
                2 => Some(AlignItems::Center),
                3 => Some(AlignItems::FlexEnd),
                4 => Some(AlignItems::Stretch),
                _ => None,
            };
        });
    }

    pub fn set_justify_content(&mut self, handle: u32, tag: u8) {
        self.with_style_mut(handle, |s| {
            s.justify_content = match tag {
                1 => Some(JustifyContent::FlexStart),
                2 => Some(JustifyContent::Center),
                3 => Some(JustifyContent::FlexEnd),
                4 => Some(JustifyContent::SpaceBetween),
                5 => Some(JustifyContent::SpaceAround),
                6 => Some(JustifyContent::SpaceEvenly),
                _ => None,
            };
        });
    }

    pub fn set_gap_px(&mut self, handle: u32, row: f32, column: f32) {
        self.with_style_mut(handle, |s| {
            s.gap = Size {
                width: LengthPercentage::length(column),
                height: LengthPercentage::length(row),
            };
        });
    }

    pub fn set_padding_px(&mut self, handle: u32, top: f32, right: f32, bottom: f32, left: f32) {
        self.with_style_mut(handle, |s| {
            s.padding = Rect {
                top: LengthPercentage::length(top),
                right: LengthPercentage::length(right),
                bottom: LengthPercentage::length(bottom),
                left: LengthPercentage::length(left),
            };
        });
    }

    pub fn set_margin_px(&mut self, handle: u32, top: f32, right: f32, bottom: f32, left: f32) {
        self.with_style_mut(handle, |s| {
            s.margin = Rect {
                top: LengthPercentageAuto::length(top),
                right: LengthPercentageAuto::length(right),
                bottom: LengthPercentageAuto::length(bottom),
                left: LengthPercentageAuto::length(left),
            };
        });
    }

    pub fn set_margin_auto(&mut self, handle: u32) {
        self.with_style_mut(handle, |s| {
            s.margin = Rect {
                top: LengthPercentageAuto::auto(),
                right: LengthPercentageAuto::auto(),
                bottom: LengthPercentageAuto::auto(),
                left: LengthPercentageAuto::auto(),
            };
        });
    }

    pub fn set_position_type(&mut self, handle: u32, tag: u8) {
        self.with_style_mut(handle, |s| {
            s.position = match tag {
                0 => Position::Relative,
                1 => Position::Absolute,
                _ => Position::Relative,
            };
        });
    }

    pub fn set_inset_px(&mut self, handle: u32, top: f32, right: f32, bottom: f32, left: f32) {
        self.with_style_mut(handle, |s| {
            s.inset = Rect {
                top: LengthPercentageAuto::length(top),
                right: LengthPercentageAuto::length(right),
                bottom: LengthPercentageAuto::length(bottom),
                left: LengthPercentageAuto::length(left),
            };
        });
    }

    pub fn set_aspect_ratio(&mut self, handle: u32, ratio: f32) {
        self.with_style_mut(handle, |s| {
            s.aspect_ratio = if ratio > 0.0 { Some(ratio) } else { None };
        });
    }

    pub fn set_fixed_measure(&mut self, handle: u32, width: f32, height: f32) {
        let node = self.resolve(handle);
        self.tree
            .set_node_context(node, Some(Size { width, height }))
            .unwrap();
    }

    pub fn compute_layout(&mut self, root: u32, available_width: f32, available_height: f32) {
        let root_id = self.resolve(root);
        let available = Size {
            width: if available_width.is_finite() {
                AvailableSpace::Definite(available_width)
            } else {
                AvailableSpace::MaxContent
            },
            height: if available_height.is_finite() {
                AvailableSpace::Definite(available_height)
            } else {
                AvailableSpace::MaxContent
            },
        };
        self.tree
            .compute_layout_with_measure(
                root_id,
                available,
                |known, _available, _node, ctx, _style| {
                    let measure = ctx.map(|s| *s).unwrap_or(Size::ZERO);
                    Size {
                        width: known.width.unwrap_or(measure.width),
                        height: known.height.unwrap_or(measure.height),
                    }
                },
            )
            .unwrap();
    }

    pub fn get_layout(&self, handle: u32) -> TaffyRect {
        let node = self.resolve(handle);
        let layout = self.tree.layout(node).unwrap();
        TaffyRect {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        }
    }
}

pub fn taffy_engine_new() -> Box<TaffyEngine> {
    Box::new(TaffyEngine::new())
}
