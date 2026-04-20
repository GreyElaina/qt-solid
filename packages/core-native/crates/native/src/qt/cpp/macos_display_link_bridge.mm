#include "qt/macos_display_link_bridge.h"

#if defined(__APPLE__)

#import <QuartzCore/CAMetalDisplayLink.h>
#import <QuartzCore/CAMetalLayer.h>

@interface QtSolidMetalDisplayLinkTarget : NSObject <CAMetalDisplayLinkDelegate>
@property(nonatomic, assign) void *context;
@property(nonatomic, assign) MacosDisplayLinkCallback callback;
@end

@implementation QtSolidMetalDisplayLinkTarget

- (void)metalDisplayLink:(CAMetalDisplayLink *)link
             needsUpdate:(CAMetalDisplayLinkUpdate *)update {
  (void)link;
  if (self.callback != nullptr) {
    void *retained_drawable = (__bridge void *)[(id)update.drawable retain];
    self.callback(self.context, retained_drawable);
  }
}

@end

struct MacosDisplayLinkHandle {
  CAMetalLayer *metal_layer = nil;
  QtSolidMetalDisplayLinkTarget *target = nil;
  CAMetalDisplayLink *display_link = nil;
};

MacosDisplayLinkHandle *qt_macos_display_link_create(
    void *metal_layer, void *context, MacosDisplayLinkCallback callback) {
  if (metal_layer == nullptr || callback == nullptr) {
    return nullptr;
  }

  auto *handle = new MacosDisplayLinkHandle;
  handle->metal_layer = (__bridge CAMetalLayer *)metal_layer;
  handle->target = [QtSolidMetalDisplayLinkTarget new];
  handle->target.context = context;
  handle->target.callback = callback;
  return handle;
}

bool qt_macos_display_link_start(MacosDisplayLinkHandle *handle) {
  if (handle == nullptr || handle->metal_layer == nil || handle->target == nil) {
    return false;
  }

  if (@available(macOS 14.0, *)) {
    if (handle->display_link == nil) {
      handle->display_link =
          [[CAMetalDisplayLink alloc] initWithMetalLayer:handle->metal_layer];
      if (handle->display_link == nil) {
        return false;
      }
      handle->display_link.delegate = handle->target;
      handle->display_link.preferredFrameLatency = 1.0f;
      [handle->display_link addToRunLoop:[NSRunLoop mainRunLoop]
                                 forMode:NSRunLoopCommonModes];
    }
    handle->display_link.paused = NO;
    return true;
  }

  return false;
}

void qt_macos_display_link_stop(MacosDisplayLinkHandle *handle) {
  if (handle == nullptr || handle->display_link == nil) {
    return;
  }
  handle->display_link.paused = YES;
}

void qt_macos_display_link_destroy(MacosDisplayLinkHandle *handle) {
  if (handle == nullptr) {
    return;
  }
  if (handle->display_link != nil) {
    [handle->display_link invalidate];
    handle->display_link = nil;
  }
  handle->target = nil;
  handle->metal_layer = nil;
  delete handle;
}

#endif
