constexpr std::uint32_t kRootNodeId = 1;

// Widget and layout enum definitions
enum class WidgetKind : std::uint8_t {
  Widget_Window = 1,
};

enum class FlexDirectionKind : std::uint8_t {
  Column = 1,
  Row = 2,
};

enum class AlignItemsKind : std::uint8_t {
  FlexStart = 1,
  Center = 2,
  FlexEnd = 3,
  Stretch = 4,
};

enum class JustifyContentKind : std::uint8_t {
  FlexStart = 1,
  Center = 2,
  FlexEnd = 3,
};

enum class FlexWrapKind : std::uint8_t {
  NoWrap = 1,
  Wrap = 2,
  WrapReverse = 3,
};

enum class AlignSelfKind : std::uint8_t {
  Auto = 1,
  FlexStart = 2,
  FlexEnd = 3,
  Center = 4,
  Stretch = 5,
};

enum class FocusPolicyKind : std::uint8_t {
  NoFocus = 1,
  TabFocus = 2,
  ClickFocus = 3,
  StrongFocus = 4,
};

[[noreturn]] void throw_error(const char *message);

void request_qt_pump();

enum class WaitBridgeKind : std::uint8_t {
  None = 0,
  UnixFd = 1,
  WindowsHandle = 2,
};

enum class PumpDriverMode : std::uint8_t {
  PollingFallback = 0,
  ExternalWake = 1,
  WaitBridge = 2,
};

class QtHostState;

int current_runtime_wait_bridge_unix_fd() noexcept;
void drain_runtime_wait_bridge_notifications() noexcept;
#if !defined(__APPLE__)
std::optional<std::uint64_t> current_runtime_wait_bridge_timer_delay_ms()
    noexcept;
#endif

[[noreturn]] void throw_error(const char *message) {
  throw std::runtime_error(message);
}

[[noreturn]] void throw_uv_error(const char *operation, int status) {
  std::string message(operation);
  message += ": ";
  message += uv_strerror(status);
  throw std::runtime_error(message);
}

WaitBridgeKind wait_bridge_kind_from_tag(std::uint8_t tag) {
  switch (tag) {
  case static_cast<std::uint8_t>(WaitBridgeKind::None):
    return WaitBridgeKind::None;
  case static_cast<std::uint8_t>(WaitBridgeKind::UnixFd):
    return WaitBridgeKind::UnixFd;
  case static_cast<std::uint8_t>(WaitBridgeKind::WindowsHandle):
    return WaitBridgeKind::WindowsHandle;
  default:
    throw_error("received unknown window-host wait bridge kind tag");
  }
}

bool on_required_qt_host_thread() {
#if defined(__APPLE__)
  return pthread_main_np() == 1;
#else
  return true;
#endif
}

QString to_qstring(::rust::Str value) {
  return QString::fromUtf8(value.data(), static_cast<qsizetype>(value.size()));
}

::rust::String to_rust_string(const QString &value) {
  const auto utf8 = value.toUtf8();
  return ::rust::String(utf8.constData(), static_cast<std::size_t>(utf8.size()));
}

WidgetKind widget_kind_from_tag(std::uint8_t kind_tag) {
  switch (kind_tag) {
  case 1:
    return WidgetKind::Widget_Window;
  default:
    throw_error("received unknown widget kind tag");
  }
}

bool widget_kind_is_top_level(WidgetKind kind) {
  switch (kind) {
  case WidgetKind::Widget_Window:
    return true;
  }

  throw_error("received unknown widget kind for top-level lookup");
}

Qt::FocusPolicy focus_policy_from_tag(std::uint8_t focus_policy_tag) {
  switch (static_cast<FocusPolicyKind>(focus_policy_tag)) {
  case FocusPolicyKind::NoFocus:
    return Qt::NoFocus;
  case FocusPolicyKind::TabFocus:
    return Qt::TabFocus;
  case FocusPolicyKind::ClickFocus:
    return Qt::ClickFocus;
  case FocusPolicyKind::StrongFocus:
    return Qt::StrongFocus;
  }

  throw_error("received unknown focus policy tag");
}
