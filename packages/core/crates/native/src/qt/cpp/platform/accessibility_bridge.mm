// Accessibility bridge: patches QNSWindow to expose accesskit platform nodes.
//
// accesskit's SubclassingAdapter overrides the content view's
// accessibilityChildren. However, macOS queries NSWindow.accessibilityChildren
// directly — which never walks the content view. This file swizzles
// accessibilityChildren on the QNSWindow class to merge the super's
// window children (title bar buttons) with the content view's
// accesskit-provided children.

#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#import <objc/message.h>

// objc_msgSendSuper2 starts lookup from super_class's superclass,
// unlike objc_msgSendSuper which starts from super_class itself.
// This is the correct function for calling [super ...] from a dynamically
// added method, avoiding recursion when our override is on the class itself.
OBJC_EXPORT id objc_msgSendSuper2(struct objc_super *super, SEL op, ...);

namespace {

static NSArray *qt_solid_a11y_window_children(id self, SEL _cmd) {
    // Re-entrancy guard: NSWindow's default accessibilityChildren walks
    // accessibilityAttributeValue: which may call accessibilityChildren again.
    static thread_local bool in_progress = false;
    if (in_progress) {
        return @[];
    }
    in_progress = true;

    // Call super's accessibilityChildren via objc_msgSendSuper2.
    struct objc_super super_info = {
        .receiver = self,
        .super_class = object_getClass(self)
    };
    NSArray *original = ((NSArray *(*)(struct objc_super *, SEL))objc_msgSendSuper2)(
        &super_info, _cmd
    );

    // Content view's accessibilityChildren (accesskit nodes).
    NSView *content_view = [self contentView];
    NSArray *ak_children = content_view ? [content_view accessibilityChildren] : nil;

    in_progress = false;

    // Re-parent accesskit nodes to the window so VoiceOver's
    // parent-child chain is consistent.
    for (id child in ak_children) {
        if ([child respondsToSelector:@selector(setAccessibilityParent:)]) {
            [child setAccessibilityParent:self];
        }
    }

    if (!ak_children.count) {
        return original ?: @[];
    }
    if (!original.count) {
        return ak_children;
    }

    NSMutableArray *merged = [NSMutableArray arrayWithArray:original];
    [merged addObjectsFromArray:ak_children];
    return merged;
}

} // namespace

extern "C" bool qt_solid_bridge_nswindow_accessibility(void *nsview_ptr) {
    if (!nsview_ptr) {
        return false;
    }

    NSView *view = (__bridge NSView *)nsview_ptr;
    NSWindow *window = [view window];
    if (!window) {
        return false;
    }

    // 1. Fix content view's accessibilityParent to point to the window.
    //    accesskit root node calls view.accessibilityParent to find its
    //    parent in the a11y tree. Qt's QNSView returns nil or a stale
    //    QMacAccessibilityElement here. Override it on the real class.
    //
    //    Use [view class] instead of object_getClass(view) to get the
    //    declared class rather than any KVO-swizzled subclass.  Adding
    //    methods to NSKVONotifying_* subclasses can corrupt KVO's
    //    internal dependent-key resolution and cause crashes in
    //    -[NSView setFrameSize:] → NSKeyValueDidChange.
    {
        Class view_cls = [view class];
        SEL parent_sel = @selector(accessibilityParent);

        // class_addMethod only adds if not already present on this exact class.
        class_addMethod(view_cls, parent_sel,
                        imp_implementationWithBlock(^id(id self_) {
                            return [self_ window];
                        }),
                        "@@:");
    }

    // 2. Override NSWindow's accessibilityChildren to merge in accesskit nodes.
    //    Same rationale: use [window class] to avoid KVO subclasses.
    {
        Class cls = [window class];
        SEL sel = @selector(accessibilityChildren);

        unsigned int count = 0;
        Method *methods = class_copyMethodList(cls, &count);
        bool already_patched = false;
        for (unsigned int i = 0; i < count; ++i) {
            if (method_getName(methods[i]) == sel) {
                already_patched = true;
                break;
            }
        }
        free(methods);

        if (!already_patched) {
            class_addMethod(cls, sel, (IMP)qt_solid_a11y_window_children, "@@:");
        }
    }

    return true;
}
