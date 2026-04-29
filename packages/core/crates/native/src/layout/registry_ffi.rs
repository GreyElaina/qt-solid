use super::state::{self as layout_state, with_layout};

#[cxx::bridge(namespace = "qt_taffy")]
pub mod registry_bridge {
    struct EngineHandle {
        engine_id: u32,
        root_node: u32,
    }

    struct RegistryTaffyRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    extern "Rust" {
        fn engine_create() -> EngineHandle;
        fn engine_destroy(engine_id: u32);

        fn engine_create_node(engine_id: u32) -> u32;
        fn engine_remove_node(engine_id: u32, handle: u32);
        fn engine_set_children(engine_id: u32, parent: u32, children: &[u32]);

        fn engine_set_display(engine_id: u32, handle: u32, tag: u8);
        fn engine_set_flex_direction(engine_id: u32, handle: u32, tag: u8);
        fn engine_set_flex_wrap(engine_id: u32, handle: u32, tag: u8);
        fn engine_set_flex_grow(engine_id: u32, handle: u32, value: f32);
        fn engine_set_flex_shrink(engine_id: u32, handle: u32, value: f32);

        fn engine_set_flex_basis_px(engine_id: u32, handle: u32, value: f32);
        fn engine_set_flex_basis_auto(engine_id: u32, handle: u32);

        fn engine_set_width_px(engine_id: u32, handle: u32, value: f32);
        fn engine_set_width_auto(engine_id: u32, handle: u32);
        fn engine_set_height_px(engine_id: u32, handle: u32, value: f32);
        fn engine_set_height_auto(engine_id: u32, handle: u32);

        fn engine_set_min_width_px(engine_id: u32, handle: u32, value: f32);
        fn engine_set_min_height_px(engine_id: u32, handle: u32, value: f32);
        fn engine_set_max_width_px(engine_id: u32, handle: u32, value: f32);
        fn engine_set_max_height_px(engine_id: u32, handle: u32, value: f32);

        fn engine_set_align_self(engine_id: u32, handle: u32, tag: u8);
        fn engine_set_align_items(engine_id: u32, handle: u32, tag: u8);
        fn engine_set_justify_content(engine_id: u32, handle: u32, tag: u8);

        fn engine_set_gap_px(engine_id: u32, handle: u32, row: f32, column: f32);
        fn engine_set_padding_px(engine_id: u32, handle: u32, top: f32, right: f32, bottom: f32, left: f32);
        fn engine_set_margin_px(engine_id: u32, handle: u32, top: f32, right: f32, bottom: f32, left: f32);
        fn engine_set_margin_auto(engine_id: u32, handle: u32);

        fn engine_set_position_type(engine_id: u32, handle: u32, tag: u8);
        fn engine_set_inset_px(engine_id: u32, handle: u32, top: f32, right: f32, bottom: f32, left: f32);
        fn engine_set_aspect_ratio(engine_id: u32, handle: u32, ratio: f32);

        fn engine_set_fixed_measure(engine_id: u32, handle: u32, width: f32, height: f32);
        fn engine_compute_layout(engine_id: u32, root: u32, available_width: f32, available_height: f32);
        fn engine_get_layout(engine_id: u32, handle: u32) -> RegistryTaffyRect;

        fn child_layout_register(widget_id: u32);
        fn child_layout_unregister(widget_id: u32);
        fn child_layout_set_taffy_handle(widget_id: u32, handle: u32, engine_id: u32);
        fn child_layout_clear_taffy_handle(widget_id: u32);
    }
}

// --- Engine lifecycle ---

pub fn engine_create() -> registry_bridge::EngineHandle {
    with_layout(|r| {
        let (engine_id, root_node) = r.create_engine();
        registry_bridge::EngineHandle { engine_id, root_node }
    })
}

pub fn engine_destroy(engine_id: u32) {
    with_layout(|r| r.destroy_engine(engine_id));
}

// --- Engine operations ---

macro_rules! engine_op {
    ($ffi_name:ident => $method:ident($($arg:ident: $ty:ty),*)) => {
        pub fn $ffi_name(engine_id: u32, $($arg: $ty),*) {
            with_layout(|r| r.engine_mut(engine_id).$method($($arg),*));
        }
    };
}

pub fn engine_create_node(engine_id: u32) -> u32 {
    with_layout(|r| r.engine_mut(engine_id).create_node())
}

pub fn engine_remove_node(engine_id: u32, handle: u32) {
    with_layout(|r| r.engine_mut(engine_id).remove_node(handle));
}

pub fn engine_set_children(engine_id: u32, parent: u32, children: &[u32]) {
    with_layout(|r| r.engine_mut(engine_id).set_children(parent, children));
}

engine_op!(engine_set_display => set_display(handle: u32, tag: u8));
engine_op!(engine_set_flex_direction => set_flex_direction(handle: u32, tag: u8));
engine_op!(engine_set_flex_wrap => set_flex_wrap(handle: u32, tag: u8));
engine_op!(engine_set_flex_grow => set_flex_grow(handle: u32, value: f32));
engine_op!(engine_set_flex_shrink => set_flex_shrink(handle: u32, value: f32));
engine_op!(engine_set_flex_basis_px => set_flex_basis_px(handle: u32, value: f32));
engine_op!(engine_set_flex_basis_auto => set_flex_basis_auto(handle: u32));
engine_op!(engine_set_width_px => set_width_px(handle: u32, value: f32));
engine_op!(engine_set_width_auto => set_width_auto(handle: u32));
engine_op!(engine_set_height_px => set_height_px(handle: u32, value: f32));
engine_op!(engine_set_height_auto => set_height_auto(handle: u32));
engine_op!(engine_set_min_width_px => set_min_width_px(handle: u32, value: f32));
engine_op!(engine_set_min_height_px => set_min_height_px(handle: u32, value: f32));
engine_op!(engine_set_max_width_px => set_max_width_px(handle: u32, value: f32));
engine_op!(engine_set_max_height_px => set_max_height_px(handle: u32, value: f32));
engine_op!(engine_set_align_self => set_align_self(handle: u32, tag: u8));
engine_op!(engine_set_align_items => set_align_items(handle: u32, tag: u8));
engine_op!(engine_set_justify_content => set_justify_content(handle: u32, tag: u8));
engine_op!(engine_set_gap_px => set_gap_px(handle: u32, row: f32, column: f32));
engine_op!(engine_set_padding_px => set_padding_px(handle: u32, top: f32, right: f32, bottom: f32, left: f32));
engine_op!(engine_set_margin_px => set_margin_px(handle: u32, top: f32, right: f32, bottom: f32, left: f32));
engine_op!(engine_set_margin_auto => set_margin_auto(handle: u32));
engine_op!(engine_set_position_type => set_position_type(handle: u32, tag: u8));
engine_op!(engine_set_inset_px => set_inset_px(handle: u32, top: f32, right: f32, bottom: f32, left: f32));
engine_op!(engine_set_aspect_ratio => set_aspect_ratio(handle: u32, ratio: f32));
engine_op!(engine_set_fixed_measure => set_fixed_measure(handle: u32, width: f32, height: f32));

pub fn engine_compute_layout(engine_id: u32, root: u32, available_width: f32, available_height: f32) {
    with_layout(|r| r.engine_mut(engine_id).compute_layout(root, available_width, available_height));
}

pub fn engine_get_layout(engine_id: u32, handle: u32) -> registry_bridge::RegistryTaffyRect {
    with_layout(|r| {
        let rect = r.engine(engine_id).get_layout(handle);
        registry_bridge::RegistryTaffyRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    })
}

// --- Child layout registry ---

pub fn child_layout_register(widget_id: u32) {
    with_layout(|r| r.register_child(widget_id));
}

pub fn child_layout_unregister(widget_id: u32) {
    with_layout(|r| r.unregister_child(widget_id));
}

pub fn child_layout_set_taffy_handle(widget_id: u32, handle: u32, engine_id: u32) {
    with_layout(|r| {
        let snapshot = {
            let layout = r.child_layout_mut(widget_id);
            layout.taffy_child_handle = handle;
            layout.taffy_engine_id = engine_id;
            layout.clone()
        };
        let engine = r.engine_mut(engine_id);
        layout_state::replay_child_taffy_style(&snapshot, engine);
    });
}

pub fn child_layout_clear_taffy_handle(widget_id: u32) {
    with_layout(|r| r.child_layout_mut(widget_id).taffy_child_handle = layout_state::NO_TAFFY_HANDLE);
}
