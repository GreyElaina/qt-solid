pub mod types;
pub mod compositor_core;
#[cfg(not(target_os = "macos"))]
pub mod compositor_actor;
#[cfg(not(target_os = "macos"))]
pub mod frame_signal;
pub mod surface;
pub mod platform;

pub use types::*;
pub use compositor_core::*;

// Platform-specific compositor entry points
#[cfg(target_os = "macos")]
pub use platform::macos::{
    load_or_create_compositor, present_compositor_frame, present_compositor_frame_async,
    compositor_frame_is_busy, compositor_frame_is_initialized,
    release_metal_drawable, destroy_compositor,
};

#[cfg(target_os = "windows")]
pub use platform::windows::{
    load_or_create_compositor, present_compositor_frame, present_compositor_frame_async,
    compositor_frame_is_busy, compositor_frame_is_initialized, destroy_compositor,
};

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "x11"))]
pub use platform::linux_x11::{
    load_or_create_compositor, present_compositor_frame, present_compositor_frame_async,
    compositor_frame_is_busy, compositor_frame_is_initialized, destroy_compositor,
};

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "wayland"))]
pub use platform::wayland::{
    load_or_create_compositor, present_compositor_frame, present_compositor_frame_async,
    compositor_frame_is_busy, compositor_frame_is_initialized, destroy_compositor,
};

// Fallback: no platform compositor, use raw wgpu surface
#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(any(target_os = "linux", target_os = "freebsd"), any(feature = "x11", feature = "wayland")),
)))]
pub use surface::{
    load_or_create_compositor, present_compositor_frame, present_compositor_frame_async,
    compositor_frame_is_busy, compositor_frame_is_initialized,
};

// Always available from surface (shared infra)
pub use surface::{
    compositor_surface_target, destroy_window_compositor, evict_layer_textures,
    prepare_compositor_frame, window_compositor_adapter_pci_bus_id,
    with_window_compositor_device_queue,
    with_window_compositor_layer_texture, with_window_compositor_layer_texture_handle,
    PreparedCompositorFrame,
};

pub use platform::backend_kind;
