#pragma once

#include <memory>

namespace qt_solid_spike::qt {

class MacosEventBufferBridge final {
public:
  MacosEventBufferBridge();
  ~MacosEventBufferBridge();

  MacosEventBufferBridge(const MacosEventBufferBridge &) = delete;
  MacosEventBufferBridge &operator=(const MacosEventBufferBridge &) = delete;
  MacosEventBufferBridge(MacosEventBufferBridge &&) = delete;
  MacosEventBufferBridge &operator=(MacosEventBufferBridge &&) = delete;

  int read_fd() const noexcept;
  void drain() noexcept;

private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

} // namespace qt_solid_spike::qt
