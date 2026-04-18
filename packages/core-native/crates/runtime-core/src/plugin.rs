use crate::handles::{AppHandle, ControllerHandle, HostHandle, NodeHandle};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStrView {
    pub ptr: *const u8,
    pub len: usize,
}

impl RuntimeStrView {
    pub const fn new(raw: &'static str) -> Self {
        Self {
            ptr: raw.as_ptr(),
            len: raw.len(),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Ok = 0,
    InvalidArg = 1,
    NotFound = 2,
    Failed = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RuntimeResult<T> {
    pub status: RuntimeStatus,
    pub value: T,
}

impl<T> RuntimeResult<T> {
    pub const fn ok(value: T) -> Self {
        Self {
            status: RuntimeStatus::Ok,
            value,
        }
    }

    pub const fn with_status(status: RuntimeStatus, value: T) -> Self {
        Self { status, value }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WidgetNodeBootstrap {
    pub node: NodeHandle,
    pub host: HostHandle,
    pub controller: ControllerHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WidgetDescriptor {
    pub spec_key: RuntimeStrView,
    pub intrinsic_name: RuntimeStrView,
    pub type_name: RuntimeStrView,
}

pub type CreateWidgetFn =
    extern "C" fn(app: AppHandle, spec_key: RuntimeStrView) -> RuntimeResult<WidgetNodeBootstrap>;
pub type DestroyWidgetFn = extern "C" fn(controller: ControllerHandle) -> RuntimeStatus;
pub type AttachChildFn =
    extern "C" fn(parent: HostHandle, child: HostHandle, anchor: HostHandle) -> RuntimeStatus;
pub type DetachChildFn = extern "C" fn(parent: HostHandle, child: HostHandle) -> RuntimeStatus;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WidgetLibraryPlugin {
    pub library_key: RuntimeStrView,
    pub widgets: &'static [WidgetDescriptor],
    pub create_widget: CreateWidgetFn,
    pub destroy_widget: DestroyWidgetFn,
    pub attach_child: AttachChildFn,
    pub detach_child: DetachChildFn,
}
