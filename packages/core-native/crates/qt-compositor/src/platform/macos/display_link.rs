use std::{ffi::c_void, mem, ptr::NonNull, sync::Arc, sync::atomic::{AtomicBool, Ordering}};

use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, define_class, msg_send,
    rc::Retained,
    runtime::ProtocolObject,
};
use objc2_foundation::{
    NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes,
};
use objc2_quartz_core::{
    CAMetalDisplayLink, CAMetalDisplayLinkDelegate, CAMetalDisplayLinkUpdate,
    CAMetalLayer as ObjcCAMetalLayer,
};
use window_host::NativeFrameNotifier;

type MacosDisplayLinkCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

fn trace_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("QT_SOLID_WGPU_TRACE").is_some());
    *ENABLED
}

fn trace(args: std::fmt::Arguments<'_>) {
    if !trace_enabled() {
        return;
    }
    println!("[qt-display-link] {args}");
}

struct DisplayLinkDelegateIvars {
    context_ptr: usize,
    callback_ptr: usize,
    notifier_ptr: usize, // *const NativeFrameNotifier (Arc-cloned, owned by Handle)
    alive_ptr: usize,    // *const AtomicBool (Arc-cloned, shared with Handle)
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[ivars = DisplayLinkDelegateIvars]
    struct DisplayLinkDelegate;

    unsafe impl NSObjectProtocol for DisplayLinkDelegate {}

    unsafe impl CAMetalDisplayLinkDelegate for DisplayLinkDelegate {
        #[unsafe(method(metalDisplayLink:needsUpdate:))]
        fn metal_display_link_needs_update(
            &self,
            _link: &CAMetalDisplayLink,
            update: &CAMetalDisplayLinkUpdate,
        ) {
            let ivars = self.ivars();
            let drawable = update.drawable();
            let drawable_handle = Retained::into_raw(drawable) as *mut c_void;

            // Check revocation flag before touching any borrowed pointers.
            if ivars.alive_ptr != 0 {
                let alive = unsafe { &*(ivars.alive_ptr as *const AtomicBool) };
                if !alive.load(Ordering::Acquire) {
                    trace(format_args!(
                        "callback revoked context=0x{:x} drawable=0x{:x}",
                        ivars.context_ptr,
                        drawable_handle as usize
                    ));
                    // Release the retained drawable to avoid Metal resource leak.
                    drop(unsafe { Retained::from_raw(drawable_handle as *mut ProtocolObject<dyn objc2_quartz_core::CAMetalDrawable>) });
                    return;
                }
            }

            let callback: MacosDisplayLinkCallback = unsafe { mem::transmute(ivars.callback_ptr) };
            trace(format_args!(
                "callback context=0x{:x} drawable=0x{:x}",
                ivars.context_ptr,
                drawable_handle as usize
            ));
            // Notify main run loop that display-link has fired.
            if ivars.notifier_ptr != 0 {
                let notifier = unsafe { &*(ivars.notifier_ptr as *const NativeFrameNotifier) };
                notifier.notify();
            }
            unsafe {
                callback(ivars.context_ptr as *mut c_void, drawable_handle);
            }
        }
    }
);

impl DisplayLinkDelegate {
    fn new_with_parts(context_ptr: usize, callback_ptr: usize, notifier_ptr: usize, alive_ptr: usize) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DisplayLinkDelegateIvars {
            context_ptr,
            callback_ptr,
            notifier_ptr,
            alive_ptr,
        });
        unsafe { msg_send![super(this), init] }
    }
}

#[repr(C)]
pub struct MacosDisplayLinkHandle {
    layer_ptr: usize,
    context_ptr: usize,
    callback: MacosDisplayLinkCallback,
    notifier: Option<Box<NativeFrameNotifier>>,
    alive: Arc<AtomicBool>,
    delegate: Option<Retained<DisplayLinkDelegate>>,
    display_link: Option<Retained<CAMetalDisplayLink>>,
}

fn borrowed_metal_layer(layer_ptr: usize) -> Option<&'static ObjcCAMetalLayer> {
    let ptr = NonNull::new(layer_ptr as *mut ObjcCAMetalLayer)?;
    Some(unsafe { ptr.as_ref() })
}

fn on_main_thread() -> bool {
    MainThreadMarker::new().is_some()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_macos_display_link_create(
    metal_layer: *mut c_void,
    context: *mut c_void,
    callback: Option<MacosDisplayLinkCallback>,
    notifier: *const c_void,
) -> *mut MacosDisplayLinkHandle {
    let Some(callback) = callback else {
        return std::ptr::null_mut();
    };
    if metal_layer.is_null() {
        return std::ptr::null_mut();
    }

    let notifier = if notifier.is_null() {
        None
    } else {
        // Clone the notifier from the opaque pointer (caller retains ownership of original).
        let notifier_ref = unsafe { &*(notifier as *const NativeFrameNotifier) };
        Some(Box::new(notifier_ref.clone()))
    };

    let handle = MacosDisplayLinkHandle {
        layer_ptr: metal_layer as usize,
        context_ptr: context as usize,
        callback,
        notifier,
        alive: Arc::new(AtomicBool::new(true)),
        delegate: None,
        display_link: None,
    };
    Box::into_raw(Box::new(handle))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_macos_display_link_start(
    handle: *mut MacosDisplayLinkHandle,
) -> bool {
    if !on_main_thread() {
        return false;
    }
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return false;
    };

    if handle.display_link.is_none() {
        let Some(layer) = borrowed_metal_layer(handle.layer_ptr) else {
            return false;
        };
        let notifier_ptr = handle.notifier.as_deref()
            .map_or(0, |n| n as *const NativeFrameNotifier as usize);
        let alive_ptr = Arc::into_raw(Arc::clone(&handle.alive)) as usize;
        let delegate = DisplayLinkDelegate::new_with_parts(
            handle.context_ptr,
            handle.callback as usize,
            notifier_ptr,
            alive_ptr,
        );
        let display_link =
            CAMetalDisplayLink::initWithMetalLayer(CAMetalDisplayLink::alloc(), layer);
        display_link.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        display_link.setPreferredFrameLatency(1.0);
        let main_run_loop = NSRunLoop::mainRunLoop();
        let common_modes = unsafe { NSRunLoopCommonModes };
        unsafe { display_link.addToRunLoop_forMode(&main_run_loop, common_modes) };
        trace(format_args!(
            "start create layer=0x{:x} context=0x{:x}",
            handle.layer_ptr,
            handle.context_ptr
        ));
        handle.delegate = Some(delegate);
        handle.display_link = Some(display_link);
    }

    if let Some(display_link) = handle.display_link.as_ref() {
        display_link.setPaused(false);
        trace(format_args!("start resume context=0x{:x}", handle.context_ptr));
        return true;
    }

    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_macos_display_link_stop(handle: *mut MacosDisplayLinkHandle) {
    if !on_main_thread() {
        return;
    }
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return;
    };
    if let Some(display_link) = handle.display_link.as_ref() {
        display_link.setPaused(true);
        trace(format_args!("stop pause context=0x{:x}", handle.context_ptr));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_macos_display_link_destroy(handle: *mut MacosDisplayLinkHandle) {
    let Some(handle) = NonNull::new(handle) else {
        return;
    };
    if !on_main_thread() {
        let leaked = unsafe { Box::from_raw(handle.as_ptr()) };
        Box::leak(leaked);
        return;
    }

    let mut handle = unsafe { Box::from_raw(handle.as_ptr()) };
    // Revoke first — any trailing callback will see this and no-op.
    handle.alive.store(false, Ordering::Release);
    if let Some(display_link) = handle.display_link.take() {
        display_link.setPaused(true);
        let main_run_loop = NSRunLoop::mainRunLoop();
        let common_modes = unsafe { NSRunLoopCommonModes };
        unsafe { display_link.removeFromRunLoop_forMode(&main_run_loop, common_modes) };
        display_link.invalidate();
        display_link.setDelegate(None);
    }
    handle.delegate.take();
}
