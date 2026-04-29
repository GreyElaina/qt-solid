use super::{TaffyEngine, taffy_engine_new};

#[cxx::bridge(namespace = "qt_taffy")]
pub mod bridge {
    struct TaffyRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    extern "Rust" {
        type TaffyEngine;

        fn taffy_engine_new() -> Box<TaffyEngine>;

        fn create_node(self: &mut TaffyEngine) -> u32;
        fn remove_node(self: &mut TaffyEngine, handle: u32);
        fn set_children(self: &mut TaffyEngine, parent: u32, children: &[u32]);

        fn set_display(self: &mut TaffyEngine, handle: u32, tag: u8);
        fn set_flex_direction(self: &mut TaffyEngine, handle: u32, tag: u8);
        fn set_flex_wrap(self: &mut TaffyEngine, handle: u32, tag: u8);
        fn set_flex_grow(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_flex_shrink(self: &mut TaffyEngine, handle: u32, value: f32);

        fn set_flex_basis_px(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_flex_basis_percent(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_flex_basis_auto(self: &mut TaffyEngine, handle: u32);

        fn set_width_px(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_width_percent(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_width_auto(self: &mut TaffyEngine, handle: u32);
        fn set_height_px(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_height_percent(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_height_auto(self: &mut TaffyEngine, handle: u32);

        fn set_min_width_px(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_min_height_px(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_max_width_px(self: &mut TaffyEngine, handle: u32, value: f32);
        fn set_max_height_px(self: &mut TaffyEngine, handle: u32, value: f32);

        fn set_align_self(self: &mut TaffyEngine, handle: u32, tag: u8);
        fn set_align_items(self: &mut TaffyEngine, handle: u32, tag: u8);
        fn set_justify_content(self: &mut TaffyEngine, handle: u32, tag: u8);

        fn set_gap_px(self: &mut TaffyEngine, handle: u32, row: f32, column: f32);
        fn set_padding_px(
            self: &mut TaffyEngine,
            handle: u32,
            top: f32,
            right: f32,
            bottom: f32,
            left: f32,
        );
        fn set_margin_px(
            self: &mut TaffyEngine,
            handle: u32,
            top: f32,
            right: f32,
            bottom: f32,
            left: f32,
        );
        fn set_margin_auto(self: &mut TaffyEngine, handle: u32);

        fn set_position_type(self: &mut TaffyEngine, handle: u32, tag: u8);
        fn set_inset_px(
            self: &mut TaffyEngine,
            handle: u32,
            top: f32,
            right: f32,
            bottom: f32,
            left: f32,
        );
        fn set_aspect_ratio(self: &mut TaffyEngine, handle: u32, ratio: f32);

        fn set_fixed_measure(self: &mut TaffyEngine, handle: u32, width: f32, height: f32);
        fn compute_layout(
            self: &mut TaffyEngine,
            root: u32,
            available_width: f32,
            available_height: f32,
        );
        fn get_layout(self: &TaffyEngine, handle: u32) -> TaffyRect;
    }
}
