#include "qt/macos_event_buffer_bridge.h"

#include <AppKit/AppKit.h>
#include <ApplicationServices/ApplicationServices.h>
#include <Block.h>
#include <dispatch/dispatch.h>
#include <objc/message.h>
#include <objc/runtime.h>
#include <unistd.h>
#include <fcntl.h>

#include <cerrno>
#include <atomic>
#include <cstdint>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>

namespace qt_solid_spike::qt {

namespace {

constexpr char kBridgeQueueLabel[] =
    "majo.akashina.qt-solid-spike.macos-event-buffer";
constexpr long long kMaxBufferedEvents = 128;

void throw_errno(const char *operation) {
  std::string message(operation);
  message += ": ";
  message += std::strerror(errno);
  throw std::runtime_error(message);
}

void set_nonblocking_close_on_exec(int fd) {
  const int current_flags = fcntl(fd, F_GETFL, 0);
  if (current_flags < 0) {
    throw_errno("fcntl(F_GETFL)");
  }
  if (fcntl(fd, F_SETFL, current_flags | O_NONBLOCK) != 0) {
    throw_errno("fcntl(F_SETFL)");
  }

  const int current_fd_flags = fcntl(fd, F_GETFD, 0);
  if (current_fd_flags < 0) {
    throw_errno("fcntl(F_GETFD)");
  }
  if (fcntl(fd, F_SETFD, current_fd_flags | FD_CLOEXEC) != 0) {
    throw_errno("fcntl(F_SETFD)");
  }
}

CGEventMask input_event_mask() {
  CGEventMask mask = 0;
  mask |= (CGEventMask(1) << kCGEventLeftMouseDown);
  mask |= (CGEventMask(1) << kCGEventLeftMouseUp);
  mask |= (CGEventMask(1) << kCGEventRightMouseDown);
  mask |= (CGEventMask(1) << kCGEventRightMouseUp);
  mask |= (CGEventMask(1) << kCGEventMouseMoved);
  mask |= (CGEventMask(1) << kCGEventLeftMouseDragged);
  mask |= (CGEventMask(1) << kCGEventRightMouseDragged);
  mask |= (CGEventMask(1) << kCGEventOtherMouseDown);
  mask |= (CGEventMask(1) << kCGEventOtherMouseUp);
  mask |= (CGEventMask(1) << kCGEventOtherMouseDragged);
  mask |= (CGEventMask(1) << kCGEventScrollWheel);
  mask |= (CGEventMask(1) << kCGEventKeyDown);
  mask |= (CGEventMask(1) << kCGEventKeyUp);
  mask |= (CGEventMask(1) << kCGEventFlagsChanged);
  mask |= (CGEventMask(1) << kCGEventTabletPointer);
  mask |= (CGEventMask(1) << kCGEventTabletProximity);
  return mask;
}

SEL sel_new() {
  static SEL selector = sel_getUid("new");
  return selector;
}

SEL sel_release() {
  static SEL selector = sel_getUid("release");
  return selector;
}

SEL sel_set_event_mask() {
  static SEL selector = sel_getUid("setEventMask:");
  return selector;
}

SEL sel_set_max_event_count() {
  static SEL selector = sel_getUid("setMaxEventCount:");
  return selector;
}

SEL sel_set_dispatch_queue_block() {
  static SEL selector = sel_getUid("setDispatchQueue:block:");
  return selector;
}

SEL sel_set_enabled() {
  static SEL selector = sel_getUid("setEnabled:");
  return selector;
}

SEL sel_drain_events() {
  static SEL selector = sel_getUid("drainEvents:");
  return selector;
}

id objc_new(Class klass) {
  return reinterpret_cast<id (*)(id, SEL)>(objc_msgSend)((id)klass, sel_new());
}

void objc_release(id object) {
  reinterpret_cast<void (*)(id, SEL)>(objc_msgSend)(object, sel_release());
}

void objc_set_event_mask(id object, std::uint64_t value) {
  reinterpret_cast<void (*)(id, SEL, std::uint64_t)>(objc_msgSend)(
      object, sel_set_event_mask(), value);
}

void objc_set_max_event_count(id object, long long value) {
  reinterpret_cast<void (*)(id, SEL, long long)>(objc_msgSend)(
      object, sel_set_max_event_count(), value);
}

void objc_set_dispatch_queue_block(id object, dispatch_queue_t queue,
                                   dispatch_block_t block) {
  reinterpret_cast<void (*)(id, SEL, id, id)>(objc_msgSend)(
      object, sel_set_dispatch_queue_block(), (id)queue, (id)block);
}

void objc_set_enabled(id object, BOOL enabled) {
  reinterpret_cast<void (*)(id, SEL, BOOL)>(objc_msgSend)(object,
                                                          sel_set_enabled(),
                                                          enabled);
}

id objc_drain_events(id object, bool *overflow) {
  return reinterpret_cast<id (*)(id, SEL, bool *)>(objc_msgSend)(
      object, sel_drain_events(), overflow);
}

} // namespace

struct MacosEventBufferBridge::Impl {
  Impl() {
    if (pipe(pipe_fds_) != 0) {
      throw_errno("pipe");
    }

    try {
      set_nonblocking_close_on_exec(pipe_fds_[0]);
      set_nonblocking_close_on_exec(pipe_fds_[1]);
      setup_event_buffer();
    } catch (...) {
      teardown_event_buffer();
      close_fds();
      throw;
    }
  }

  ~Impl() {
    teardown_event_buffer();
    close_fds();
  }

  int read_fd() const noexcept { return pipe_fds_[0]; }

  void drain() noexcept {
    std::uint8_t buffer[128];
    for (;;) {
      const ssize_t read_count = read(pipe_fds_[0], buffer, sizeof(buffer));
      if (read_count > 0) {
        continue;
      }
      if (read_count == 0) {
        break;
      }
      if (errno == EINTR) {
        continue;
      }
      if (errno == EAGAIN || errno == EWOULDBLOCK) {
        break;
      }
      break;
    }
  }

  void on_event_buffer_ready() noexcept {
    @autoreleasepool {
      if (event_buffer_ == nil) {
        return;
      }

      bool overflow = false;
      (void)objc_drain_events(event_buffer_, &overflow);

      if (stopping_.load(std::memory_order_acquire)) {
        return;
      }

      signal_pipe();
    }
  }

private:
  void setup_event_buffer() {
    Class event_buffer_class = objc_getClass("_NSCGEventBuffer");
    if (event_buffer_class == Nil) {
      throw std::runtime_error("_NSCGEventBuffer unavailable");
    }

    event_buffer_ = objc_new(event_buffer_class);
    if (event_buffer_ == nil) {
      throw std::runtime_error("failed to construct _NSCGEventBuffer");
    }

    queue_ = dispatch_queue_create(kBridgeQueueLabel, DISPATCH_QUEUE_SERIAL);
    if (queue_ == nullptr) {
      throw std::runtime_error("dispatch_queue_create failed for event buffer");
    }

    auto *self = this;
    event_block_ = Block_copy(^{
      self->on_event_buffer_ready();
    });
    if (event_block_ == nullptr) {
      throw std::runtime_error("Block_copy failed for event buffer callback");
    }

    objc_set_event_mask(event_buffer_, input_event_mask());
    objc_set_max_event_count(event_buffer_, kMaxBufferedEvents);
    objc_set_dispatch_queue_block(event_buffer_, queue_, event_block_);
    objc_set_enabled(event_buffer_, YES);
  }

  void teardown_event_buffer() noexcept {
    stopping_.store(true, std::memory_order_release);

    if (event_buffer_ != nil) {
      objc_set_enabled(event_buffer_, NO);
    }

    if (queue_ != nullptr) {
      dispatch_sync(queue_, ^{
      });
    }

    if (event_buffer_ != nil) {
      objc_release(event_buffer_);
      event_buffer_ = nil;
    }

    if (event_block_ != nullptr) {
      Block_release(event_block_);
      event_block_ = nullptr;
    }

#if !OS_OBJECT_USE_OBJC
    if (queue_ != nullptr) {
      dispatch_release(queue_);
    }
#endif
    queue_ = nullptr;
  }

  void signal_pipe() noexcept {
    const std::uint8_t byte = 1;
    for (;;) {
      const ssize_t write_count = write(pipe_fds_[1], &byte, sizeof(byte));
      if (write_count == sizeof(byte)) {
        return;
      }
      if (write_count < 0 && errno == EINTR) {
        continue;
      }
      // Pipe already readable is enough; coalesce wakeups.
      return;
    }
  }

  void close_fds() noexcept {
    if (pipe_fds_[0] >= 0) {
      close(pipe_fds_[0]);
      pipe_fds_[0] = -1;
    }
    if (pipe_fds_[1] >= 0) {
      close(pipe_fds_[1]);
      pipe_fds_[1] = -1;
    }
  }

  int pipe_fds_[2] = {-1, -1};
  std::atomic<bool> stopping_{false};
  id event_buffer_ = nil;
  dispatch_queue_t queue_ = nullptr;
  dispatch_block_t event_block_ = nullptr;
};

MacosEventBufferBridge::MacosEventBufferBridge()
    : impl_(std::make_unique<Impl>()) {}

MacosEventBufferBridge::~MacosEventBufferBridge() = default;

int MacosEventBufferBridge::read_fd() const noexcept { return impl_->read_fd(); }

void MacosEventBufferBridge::drain() noexcept { impl_->drain(); }

} // namespace qt_solid_spike::qt

#import <QuartzCore/CAMetalLayer.h>

extern "C" void qt_wgpu_set_metal_layer_presents_with_transaction(void *layer, bool value) {
  if (layer == nullptr) {
    return;
  }
  auto *metal_layer = (__bridge CAMetalLayer *)layer;
  metal_layer.presentsWithTransaction = value ? YES : NO;
}
