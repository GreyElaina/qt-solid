use std::{ffi::c_void, mem, ptr::NonNull, sync::Arc, sync::atomic::{AtomicBool, Ordering}};

use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, define_class, msg_send, sel,
    rc::Retained,
};
use objc2_foundation::{
    NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes,
};
use objc2_quartz_core::{CADisplayLink, CAFrameRateRange};
use window_host::NativeFrameNotifier;

type MacosDisplayLinkCallback = unsafe extern "C" fn(*mut c_void);

struct DisplayLinkTargetIvars {
    context_ptr: usize,
    callback_ptr: usize,
    notifier_ptr: usize,
    alive_ptr: usize,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[ivars = DisplayLinkTargetIvars]
    struct DisplayLinkTarget;

    unsafe impl NSObjectProtocol for DisplayLinkTarget {}

    impl DisplayLinkTarget {
        #[unsafe(method(tick:))]
        fn tick(&self, _link: &CADisplayLink) {
            let ivars = self.ivars();

            if ivars.alive_ptr != 0 {
                let alive = unsafe { &*(ivars.alive_ptr as *const AtomicBool) };
                if !alive.load(Ordering::Acquire) {
                    return;
                }
            }

            let callback: MacosDisplayLinkCallback = unsafe { mem::transmute(ivars.callback_ptr) };
            if ivars.notifier_ptr != 0 {
                let notifier = unsafe { &*(ivars.notifier_ptr as *const NativeFrameNotifier) };
                notifier.notify();
            }
            unsafe {
                callback(ivars.context_ptr as *mut c_void);
            }
        }
    }
);

impl DisplayLinkTarget {
    fn new_with_parts(context_ptr: usize, callback_ptr: usize, notifier_ptr: usize, alive_ptr: usize) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DisplayLinkTargetIvars {
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
    context_ptr: usize,
    callback: MacosDisplayLinkCallback,
    notifier: Option<Box<NativeFrameNotifier>>,
    alive: Arc<AtomicBool>,
    target: Option<Retained<DisplayLinkTarget>>,
    display_link: Option<Retained<CADisplayLink>>,
}

fn on_main_thread() -> bool {
    MainThreadMarker::new().is_some()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_macos_display_link_create(
    context: *mut c_void,
    callback: Option<MacosDisplayLinkCallback>,
    notifier: *const c_void,
) -> *mut MacosDisplayLinkHandle {
    let Some(callback) = callback else {
        return std::ptr::null_mut();
    };

    let notifier = if notifier.is_null() {
        None
    } else {
        let notifier_ref = unsafe { &*(notifier as *const NativeFrameNotifier) };
        Some(Box::new(notifier_ref.clone()))
    };

    let handle = MacosDisplayLinkHandle {
        context_ptr: context as usize,
        callback,
        notifier,
        alive: Arc::new(AtomicBool::new(true)),
        target: None,
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
        let notifier_ptr = handle.notifier.as_deref()
            .map_or(0, |n| n as *const NativeFrameNotifier as usize);
        let alive_ptr = Arc::into_raw(Arc::clone(&handle.alive)) as usize;
        let target = DisplayLinkTarget::new_with_parts(
            handle.context_ptr,
            handle.callback as usize,
            notifier_ptr,
            alive_ptr,
        );
        let display_link = unsafe {
            CADisplayLink::displayLinkWithTarget_selector(&target, sel!(tick:))
        };
        let main_run_loop = NSRunLoop::mainRunLoop();
        let common_modes = unsafe { NSRunLoopCommonModes };
        unsafe { display_link.addToRunLoop_forMode(&main_run_loop, common_modes) };
        handle.target = Some(target);
        handle.display_link = Some(display_link);
    }

    if let Some(display_link) = handle.display_link.as_ref() {
        display_link.setPreferredFrameRateRange(CAFrameRateRange::new(15.0, 120.0, 120.0));
        display_link.setPaused(false);
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
    handle.alive.store(false, Ordering::Release);
    if let Some(display_link) = handle.display_link.take() {
        display_link.setPaused(true);
        let main_run_loop = NSRunLoop::mainRunLoop();
        let common_modes = unsafe { NSRunLoopCommonModes };
        unsafe { display_link.removeFromRunLoop_forMode(&main_run_loop, common_modes) };
        display_link.invalidate();
    }
    handle.target.take();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_macos_display_link_set_preferred_fps(
    handle: *mut MacosDisplayLinkHandle,
    fps: f32,
) {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return;
    };
    let Some(display_link) = handle.display_link.as_ref() else {
        return;
    };
    let preferred = fps.max(1.0);
    let minimum = (preferred * 0.5).max(1.0);
    let maximum = preferred;
    display_link.setPreferredFrameRateRange(CAFrameRateRange::new(minimum, maximum, preferred));
}
