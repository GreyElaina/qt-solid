#pragma once

#include <cstdint>

class RustWidgetBindingHost {
public:
  virtual ~RustWidgetBindingHost() = default;
  virtual void bind_rust_widget(std::uint32_t node_id,
                                std::uint8_t kind_tag) = 0;
};
