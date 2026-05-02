//! Accessibility bridge: patches QNSWindow to expose accesskit platform nodes.
//!
//! accesskit's SubclassingAdapter overrides the content view's
//! accessibilityChildren. However, macOS queries NSWindow.accessibilityChildren
//! directly — which never walks the content view. This module patches
//! accessibilityChildren on the QNSWindow class to merge the super's
//! window children (title bar buttons) with the content view's
//! accesskit-provided children.
//!
//! Uses plain `extern "C"` function IMPs instead of `imp_implementationWithBlock`
//! to avoid KVO/method-signature issues: block-based IMPs don't provide a valid
//! `NSMethodSignature` for `_NSGetValueWithMethod`, causing crashes during
//! `NSView setFrameSize:` when AppKit's dependent-key caching tries to
//! introspect accessibility-related selectors.

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;
use objc2::ffi;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Imp, NSObject, Sel};
use objc2::{msg_send, sel};
use objc2_app_kit::NSView;
use objc2_foundation::NSArray;

// ── accessibilityParent override for the content view ──────────────────

/// IMP for `accessibilityParent` on the content view class.
/// Returns `[self window]` so that accesskit root nodes find the correct
/// parent in the a11y tree.
unsafe extern "C" fn view_accessibility_parent(this: &NSObject, _cmd: Sel) -> *mut AnyObject {
    let window: *mut AnyObject = unsafe { msg_send![this, window] };
    window
}

// ── accessibilityChildren override for the window ──────────────────────

thread_local! {
    static IN_PROGRESS: Cell<bool> = const { Cell::new(false) };
}

/// IMP for `accessibilityChildren` on the window class.
/// Merges super's children (title bar buttons etc.) with the content view's
/// accesskit-provided children.
unsafe extern "C" fn window_accessibility_children(
    this: &NSObject,
    _cmd: Sel,
) -> *mut AnyObject {
    // Re-entrancy guard: NSWindow's default accessibilityChildren walks
    // accessibilityAttributeValue: which may call accessibilityChildren again.
    let reentrant = IN_PROGRESS.with(|f| {
        if f.get() {
            return true;
        }
        f.set(true);
        false
    });
    if reentrant {
        let empty: Retained<NSArray<AnyObject>> = NSArray::new();
        return Retained::into_raw(empty).cast();
    }

    // Call super's accessibilityChildren via objc_msgSendSuper.
    // We pass the *superclass* of the window's class so lookup starts
    // from the right level (skipping our added override).
    let this_class: *const AnyClass = unsafe { ffi::object_getClass(ptr::from_ref(this) as *const AnyObject) };
    let superclass: *const AnyClass = unsafe { ffi::class_getSuperclass(this_class) };
    let mut super_info = ffi::objc_super {
        receiver: ptr::from_ref(this) as *mut AnyObject,
        super_class: superclass,
    };
    let original: *mut AnyObject = unsafe {
        let f: unsafe extern "C-unwind" fn(*mut ffi::objc_super, Sel) -> *mut AnyObject =
            std::mem::transmute(ffi::objc_msgSendSuper as unsafe extern "C-unwind" fn());
        f(&mut super_info, _cmd)
    };

    // Content view's accessibilityChildren (accesskit nodes).
    let content_view: *mut NSView = unsafe { msg_send![this, contentView] };
    let ak_children: *mut NSArray<AnyObject> = if !content_view.is_null() {
        unsafe { msg_send![&*content_view, accessibilityChildren] }
    } else {
        ptr::null_mut()
    };

    IN_PROGRESS.with(|f| f.set(false));

    // Re-parent accesskit nodes to the window so VoiceOver's
    // parent-child chain is consistent.
    if !ak_children.is_null() {
        let ak_arr = unsafe { &*ak_children };
        let count: usize = ak_arr.len();
        for i in 0..count {
            let child: *mut AnyObject = unsafe { msg_send![ak_arr, objectAtIndex: i] };
            if child.is_null() { continue; }
            let responds: bool =
                unsafe { msg_send![&*child, respondsToSelector: sel!(setAccessibilityParent:)] };
            if responds {
                let _: () =
                    unsafe { msg_send![&*child, setAccessibilityParent: ptr::from_ref(this)] };
            }
        }
    }

    let ak_empty = ak_children.is_null()
        || (!ak_children.is_null() && unsafe { (*ak_children).len() } == 0);
    let orig_empty =
        original.is_null() || (!original.is_null() && unsafe { (*(original as *const NSArray<AnyObject>)).len() } == 0);

    if ak_empty {
        if original.is_null() {
            let empty: Retained<NSArray<AnyObject>> = NSArray::new();
            return Retained::into_raw(empty).cast();
        }
        return original;
    }
    if orig_empty {
        return ak_children.cast();
    }

    // Merge: original + ak_children
    let ns_mutable_array_cls =
        objc2::runtime::AnyClass::get(c"NSMutableArray").expect("NSMutableArray class not found");
    let merged: *mut AnyObject =
        unsafe { msg_send![ns_mutable_array_cls, arrayWithArray: original] };
    let _: () = unsafe { msg_send![&*merged, addObjectsFromArray: ak_children] };
    merged
}

// ── Public entry point ─────────────────────────────────────────────────

/// Patches `accessibilityParent` on the content view's class and
/// `accessibilityChildren` on the window's class. Uses `[obj class]`
/// (declared class) rather than `object_getClass` to avoid polluting
/// KVO isa-swizzled subclasses.
///
/// # Safety
///
/// `nsview_ptr` must be a valid `NSView *`.
pub(crate) unsafe fn bridge_nswindow_accessibility(nsview_ptr: *mut c_void) -> bool {
    if nsview_ptr.is_null() {
        return false;
    }

    let view: &NSView = unsafe { &*(nsview_ptr as *const NSView) };
    let window: *mut AnyObject = unsafe { msg_send![view, window] };
    if window.is_null() {
        return false;
    }

    // 1. Override accessibilityParent on the content view's *declared* class
    //    (not the KVO subclass from object_getClass).
    {
        let view_cls: *const AnyClass = unsafe { msg_send![view, class] };
        let parent_sel = sel!(accessibilityParent);
        unsafe {
            ffi::class_addMethod(
                view_cls as *mut AnyClass,
                parent_sel,
                std::mem::transmute::<
                    unsafe extern "C" fn(&NSObject, Sel) -> *mut AnyObject,
                    Imp,
                >(view_accessibility_parent),
                c"@@:".as_ptr(),
            );
        }
    }

    // 2. Override accessibilityChildren on the window's *declared* class.
    {
        let win_cls: *const AnyClass = unsafe { msg_send![&*window, class] };
        let sel = sel!(accessibilityChildren);

        // class_addMethod only adds if not already present on this exact
        // class, so we can just call it — it's a no-op if already patched.
        unsafe {
            ffi::class_addMethod(
                win_cls as *mut AnyClass,
                sel,
                std::mem::transmute::<
                    unsafe extern "C" fn(&NSObject, Sel) -> *mut AnyObject,
                    Imp,
                >(window_accessibility_children),
                c"@@:".as_ptr(),
            );
        }
    }

    true
}
