use std::ffi::c_void;
use std::num::{NonZeroIsize, NonZeroU32};
use std::ptr::NonNull;

/// Platform-native surface handle passed to wgpu for surface creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SurfaceHandle {
    AppKit(NonNull<c_void>),
    Win32(NonZeroIsize),
    Xcb {
        window: NonZeroU32,
        connection: NonNull<c_void>,
    },
    Wayland {
        surface: NonNull<c_void>,
        display: NonNull<c_void>,
    },
}

// SAFETY: The pointers in SurfaceHandle are opaque platform handles (NSView*,
// xcb_connection_t*, wl_surface*, wl_display*) that are created by the
// windowing system and remain valid for the lifetime of the surface.
// They are never dereferenced on the Rust side — only passed through to
// wgpu/raw-window-handle. The containing SurfaceTarget is Send because it
// is passed between the main thread and render threads.
unsafe impl Send for SurfaceHandle {}
unsafe impl Sync for SurfaceHandle {}

/// Complete surface description passed from C++ on each frame drive.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SurfaceTarget {
    pub(crate) handle: SurfaceHandle,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}
