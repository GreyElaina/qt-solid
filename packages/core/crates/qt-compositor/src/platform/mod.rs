#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "x11"))]
pub mod linux_x11;

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "wayland"))]
pub mod wayland;

#[cfg(target_os = "macos")]
pub fn backend_kind() -> crate::compositor_core::CompositorBackendKind {
    crate::compositor_core::CompositorBackendKind::Macos
}

#[cfg(target_os = "windows")]
pub fn backend_kind() -> crate::compositor_core::CompositorBackendKind {
    crate::compositor_core::CompositorBackendKind::Windows
}

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "x11"))]
pub fn backend_kind() -> crate::compositor_core::CompositorBackendKind {
    crate::compositor_core::CompositorBackendKind::X11
}

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "wayland"))]
pub fn backend_kind() -> crate::compositor_core::CompositorBackendKind {
    crate::compositor_core::CompositorBackendKind::Wayland
}
