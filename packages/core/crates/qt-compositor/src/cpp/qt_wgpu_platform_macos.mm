#import <QuartzCore/CAMetalLayer.h>

extern "C" void qt_wgpu_platform_set_metal_layer_presents_with_transaction(
    void *metal_layer, bool presents_with_transaction) {
  if (metal_layer != nullptr) {
    id layer = (id)metal_layer;
    [layer setPresentsWithTransaction:presents_with_transaction];
  }
}
