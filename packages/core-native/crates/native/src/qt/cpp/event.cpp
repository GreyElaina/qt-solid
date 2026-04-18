#include <tuple>
#include <utility>

class WidgetEventForwarder final : public QObject {
public:
  using EventHandler = std::function<void()>;

  WidgetEventForwarder(QWidget *target, QEvent::Type event_type,
                       EventHandler handler)
      : QObject(target), target_(target), event_type_(event_type),
        handler_(std::move(handler)) {
    target_->installEventFilter(this);
  }

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
    if (watched == target_ && event && event->type() == event_type_) {
      handler_();
    }

    return QObject::eventFilter(watched, event);
  }

private:
  QWidget *target_ = nullptr;
  QEvent::Type event_type_;
  EventHandler handler_;
};

QtListenerValue make_string_listener_value(std::string_view path,
                                           const QString &value) {
  const auto utf8 = value.toUtf8();
  QtListenerValue result;
  result.path = rust::String(path.data(), path.size());
  result.kind_tag = static_cast<std::uint8_t>(EventValueKind::String);
  result.string_value = rust::String(utf8.constData(), utf8.size());
  result.bool_value = false;
  result.i32_value = 0;
  result.f64_value = 0.0;
  return result;
}

QtListenerValue make_bool_listener_value(std::string_view path, bool value) {
  QtListenerValue result;
  result.path = rust::String(path.data(), path.size());
  result.kind_tag = static_cast<std::uint8_t>(EventValueKind::Bool);
  result.string_value = rust::String();
  result.bool_value = value;
  result.i32_value = 0;
  result.f64_value = 0.0;
  return result;
}

QtListenerValue make_i32_listener_value(std::string_view path, int value) {
  QtListenerValue result;
  result.path = rust::String(path.data(), path.size());
  result.kind_tag = static_cast<std::uint8_t>(EventValueKind::I32);
  result.string_value = rust::String();
  result.bool_value = false;
  result.i32_value = value;
  result.f64_value = 0.0;
  return result;
}

QtListenerValue make_f64_listener_value(std::string_view path, double value) {
  QtListenerValue result;
  result.path = rust::String(path.data(), path.size());
  result.kind_tag = static_cast<std::uint8_t>(EventValueKind::F64);
  result.string_value = rust::String();
  result.bool_value = false;
  result.i32_value = 0;
  result.f64_value = value;
  return result;
}

void append_listener_value(rust::Vec<QtListenerValue> &values,
                           std::string_view path, const QString &value) {
  values.push_back(make_string_listener_value(path, value));
}

void append_listener_value(rust::Vec<QtListenerValue> &values,
                           std::string_view path, bool value) {
  values.push_back(make_bool_listener_value(path, value));
}

void append_listener_value(rust::Vec<QtListenerValue> &values,
                           std::string_view path, int value) {
  values.push_back(make_i32_listener_value(path, value));
}

void append_listener_value(rust::Vec<QtListenerValue> &values,
                           std::string_view path, double value) {
  values.push_back(make_f64_listener_value(path, value));
}

void emit_marshaled_listener_event(std::uint32_t id, std::uint8_t kind_tag,
                                   std::uint8_t event_index,
                                   std::uint64_t trace_id) {
  qt_solid_spike::qt::emit_listener_event(id, kind_tag, event_index,
                                               trace_id, {});
}

const char *event_trace_detail(const CompiledEventBinding &event) {
  return event.lower_name.c_str();
}

template <typename T0>
void emit_marshaled_listener_event(std::uint32_t id, std::uint8_t kind_tag,
                                   std::uint8_t event_index,
                                   std::uint64_t trace_id,
                                   const CompiledEventBinding &event,
                                   const T0 &value0) {
  rust::Vec<QtListenerValue> values;
  const std::string_view path =
      event.payload_kind == EventPayloadKind::Object &&
              !event.payload_fields.empty()
          ? std::string_view(event.payload_fields[0].js_name)
          : std::string_view("");
  append_listener_value(values, path, value0);
  qt_solid_spike::qt::emit_listener_event(id, kind_tag, event_index,
                                          trace_id, std::move(values));
}

template <typename Tuple, std::size_t... Index>
void append_marshaled_listener_values(rust::Vec<QtListenerValue> &values,
                                      const CompiledEventBinding &event,
                                      const Tuple &tuple,
                                      std::index_sequence<Index...>) {
  (append_listener_value(values, event.payload_fields[Index].js_name,
                         std::get<Index>(tuple)),
   ...);
}

template <typename T0, typename T1, typename... Rest>
void emit_marshaled_listener_event(std::uint32_t id, std::uint8_t kind_tag,
                                   std::uint8_t event_index,
                                   std::uint64_t trace_id,
                                   const CompiledEventBinding &event,
                                   const T0 &value0, const T1 &value1,
                                   const Rest &...rest) {
  constexpr std::size_t value_count = 2 + sizeof...(Rest);
  if (event.payload_kind != EventPayloadKind::Object) {
    throw_error("multi-value event emission requires object payload");
  }
  if (event.payload_fields.size() != value_count) {
    throw_error("event payload field count does not match emitted values");
  }

  rust::Vec<QtListenerValue> values;
  const auto tuple = std::forward_as_tuple(value0, value1, rest...);
  append_marshaled_listener_values(values, event, tuple,
                                   std::make_index_sequence<value_count>{});
  qt_solid_spike::qt::emit_listener_event(id, kind_tag, event_index,
                                          trace_id, std::move(values));
}

#include "qt_widget_event_mounts.inc"

void wire_widget_events(std::uint32_t id, QObject *widget,
                        const CompiledWidgetContract &contract) {
  for (const auto &event : contract.events) {
    const auto kind_tag = contract.kind_tag;
    const auto event_index = event.event_index;

    if (event.lower_kind == EventLowerKind::Custom) {
      if (!wire_generated_host_event(id, kind_tag, widget, event)) {
        throw_error("Qt host contract has unsupported custom event lowering");
      }
      continue;
    }

    const QMetaMethod signal =
        widget->metaObject()->method(event.signal_method_index);

    switch (event.payload_kind) {
    case EventPayloadKind::Unit:
      QMetaObject::connect(
          widget, signal, widget, [id, kind_tag, event_index, event]() {
            const auto trace_id = qt_solid_spike::qt::next_trace_id();
            qt_solid_spike::qt::trace_cpp_stage(
                trace_id, rust::Str("cpp.signal.enter"), id, 0,
                rust::Str(event_trace_detail(event)));
            emit_marshaled_listener_event(id, kind_tag, event_index, trace_id);
            qt_solid_spike::qt::trace_cpp_stage(
                trace_id, rust::Str("cpp.signal.exit"), id, 0,
                rust::Str(event_trace_detail(event)));
          });
      break;
    case EventPayloadKind::Scalar:
      switch (event.scalar_kind) {
      case EventValueKind::String:
        QMetaObject::connect(
            widget, signal, widget,
            [id, kind_tag, event_index, event](const QString &value) {
              const auto trace_id = qt_solid_spike::qt::next_trace_id();
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.enter"), id, 0,
                  rust::Str(event_trace_detail(event)));
              emit_marshaled_listener_event(id, kind_tag, event_index, trace_id,
                                            event, value);
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.exit"), id, 0,
                  rust::Str(event_trace_detail(event)));
            });
        break;
      case EventValueKind::Bool:
        QMetaObject::connect(
            widget, signal, widget,
            [id, kind_tag, event_index, event](bool value) {
              const auto trace_id = qt_solid_spike::qt::next_trace_id();
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.enter"), id, 0,
                  rust::Str(event_trace_detail(event)));
              emit_marshaled_listener_event(id, kind_tag, event_index, trace_id,
                                            event, value);
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.exit"), id, 0,
                  rust::Str(event_trace_detail(event)));
            });
        break;
      case EventValueKind::I32:
        QMetaObject::connect(
            widget, signal, widget,
            [id, kind_tag, event_index, event](int value) {
              const auto trace_id = qt_solid_spike::qt::next_trace_id();
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.enter"), id, 0,
                  rust::Str(event_trace_detail(event)));
              emit_marshaled_listener_event(id, kind_tag, event_index, trace_id,
                                            event, value);
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.exit"), id, 0,
                  rust::Str(event_trace_detail(event)));
            });
        break;
      case EventValueKind::F64:
        QMetaObject::connect(
            widget, signal, widget,
            [id, kind_tag, event_index, event](double value) {
              const auto trace_id = qt_solid_spike::qt::next_trace_id();
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.enter"), id, 0,
                  rust::Str(event_trace_detail(event)));
              emit_marshaled_listener_event(id, kind_tag, event_index, trace_id,
                                            event, value);
              qt_solid_spike::qt::trace_cpp_stage(
                  trace_id, rust::Str("cpp.signal.exit"), id, 0,
                  rust::Str(event_trace_detail(event)));
            });
        break;
      }
      break;
    case EventPayloadKind::Object:
      if (event.payload_fields.size() == 1) {
        switch (event.payload_fields[0].kind) {
        case EventValueKind::String:
          QMetaObject::connect(
              widget, signal, widget,
              [id, kind_tag, event_index, event](const QString &value0) {
                const auto trace_id = qt_solid_spike::qt::next_trace_id();
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.enter"), id, 0,
                    rust::Str(event_trace_detail(event)));
                emit_marshaled_listener_event(id, kind_tag, event_index,
                                              trace_id, event, value0);
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.exit"), id, 0,
                    rust::Str(event_trace_detail(event)));
              });
          break;
        case EventValueKind::Bool:
          QMetaObject::connect(
              widget, signal, widget,
              [id, kind_tag, event_index, event](bool value0) {
                const auto trace_id = qt_solid_spike::qt::next_trace_id();
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.enter"), id, 0,
                    rust::Str(event_trace_detail(event)));
                emit_marshaled_listener_event(id, kind_tag, event_index,
                                              trace_id, event, value0);
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.exit"), id, 0,
                    rust::Str(event_trace_detail(event)));
              });
          break;
        case EventValueKind::I32:
          QMetaObject::connect(
              widget, signal, widget,
              [id, kind_tag, event_index, event](int value0) {
                const auto trace_id = qt_solid_spike::qt::next_trace_id();
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.enter"), id, 0,
                    rust::Str(event_trace_detail(event)));
                emit_marshaled_listener_event(id, kind_tag, event_index,
                                              trace_id, event, value0);
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.exit"), id, 0,
                    rust::Str(event_trace_detail(event)));
              });
          break;
        case EventValueKind::F64:
          QMetaObject::connect(
              widget, signal, widget,
              [id, kind_tag, event_index, event](double value0) {
                const auto trace_id = qt_solid_spike::qt::next_trace_id();
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.enter"), id, 0,
                    rust::Str(event_trace_detail(event)));
                emit_marshaled_listener_event(id, kind_tag, event_index,
                                              trace_id, event, value0);
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.exit"), id, 0,
                    rust::Str(event_trace_detail(event)));
              });
          break;
        }
        break;
      }

      if (event.payload_fields.size() == 2) {
        const auto kind0 = event.payload_fields[0].kind;
        const auto kind1 = event.payload_fields[1].kind;
        if (kind0 == EventValueKind::I32 && kind1 == EventValueKind::I32) {
          QMetaObject::connect(
              widget, signal, widget,
              [id, kind_tag, event_index, event](int value0, int value1) {
                const auto trace_id = qt_solid_spike::qt::next_trace_id();
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.enter"), id, 0,
                    rust::Str(event_trace_detail(event)));
                emit_marshaled_listener_event(id, kind_tag, event_index,
                                              trace_id, event, value0, value1);
                qt_solid_spike::qt::trace_cpp_stage(
                    trace_id, rust::Str("cpp.signal.exit"), id, 0,
                    rust::Str(event_trace_detail(event)));
              });
          break;
        }

        throw_error("Qt host event object payload supports only i32/i32 pairs");
      }

      throw_error("Qt host event object payload supports at most two fields");
    }
  }
}
