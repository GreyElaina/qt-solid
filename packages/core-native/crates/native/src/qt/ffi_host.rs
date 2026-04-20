fn widget_binding(kind_tag: u8) -> Option<&'static crate::bootstrap::WidgetBinding> {
    crate::bootstrap::widget_registry()
        .widget_type_id_from_host_tag(kind_tag)
        .map(|widget_type_id| crate::bootstrap::widget_registry().binding(widget_type_id))
}

pub(crate) fn qt_widget_event_count(kind_tag: u8) -> usize {
    widget_binding(kind_tag)
        .map(|binding| binding.events.len())
        .unwrap_or(0)
}

pub(crate) fn qt_widget_event_lower_kind(kind_tag: u8, index: usize) -> u8 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.events.get(index))
        .map(|event| match event.lowering {
            crate::bootstrap::EventLowering::QtSignal(_) => {
                crate::bootstrap::EventLowerKind::QtSignal as u8
            }
            crate::bootstrap::EventLowering::Custom(_) => {
                crate::bootstrap::EventLowerKind::Custom as u8
            }
        })
        .unwrap_or(0)
}

pub(crate) fn qt_widget_event_lower_name(kind_tag: u8, index: usize) -> &'static str {
    widget_binding(kind_tag)
        .and_then(|binding| binding.events.get(index))
        .map(|event| match event.lowering {
            crate::bootstrap::EventLowering::QtSignal(name)
            | crate::bootstrap::EventLowering::Custom(name) => name,
        })
        .unwrap_or("")
}

pub(crate) fn qt_widget_event_payload_kind(kind_tag: u8, index: usize) -> u8 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.events.get(index))
        .map(|event| event.payload_kind as u8)
        .unwrap_or(0)
}

fn event_scalar_kind_tag(value_type: crate::bootstrap::QtTypeInfo) -> u8 {
    match value_type.repr() {
        crate::bootstrap::QtValueRepr::String => 1,
        crate::bootstrap::QtValueRepr::Bool => 2,
        crate::bootstrap::QtValueRepr::I32 { .. } | crate::bootstrap::QtValueRepr::Enum(_) => 3,
        crate::bootstrap::QtValueRepr::F64 { .. } => 4,
        _ => panic!("unsupported event scalar type"),
    }
}

pub(crate) fn qt_widget_event_payload_scalar_kind(kind_tag: u8, index: usize) -> u8 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.events.get(index))
        .and_then(|event| event.payload_type)
        .map(event_scalar_kind_tag)
        .unwrap_or(0)
}

pub(crate) fn qt_widget_event_payload_field_count(kind_tag: u8, index: usize) -> usize {
    widget_binding(kind_tag)
        .and_then(|binding| binding.events.get(index))
        .map(|event| event.payload_fields.len())
        .unwrap_or(0)
}

pub(crate) fn qt_widget_event_payload_field_name(
    kind_tag: u8,
    index: usize,
    field_index: usize,
) -> &'static str {
    widget_binding(kind_tag)
        .and_then(|binding| binding.events.get(index))
        .and_then(|event| event.payload_fields.get(field_index))
        .map(|field| field.js_name)
        .unwrap_or("")
}

pub(crate) fn qt_widget_event_payload_field_kind(
    kind_tag: u8,
    index: usize,
    field_index: usize,
) -> u8 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.events.get(index))
        .and_then(|event| event.payload_fields.get(field_index))
        .map(|field| event_scalar_kind_tag(field.value_type))
        .unwrap_or(0)
}

pub(crate) fn qt_widget_prop_count(kind_tag: u8) -> usize {
    widget_binding(kind_tag)
        .map(|binding| binding.props.len())
        .unwrap_or(0)
}

pub(crate) fn qt_widget_prop_id(kind_tag: u8, index: usize) -> u16 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .map(|prop| u16::from(prop.index) + 1)
        .unwrap_or(0)
}

pub(crate) fn qt_widget_prop_js_name(kind_tag: u8, index: usize) -> &'static str {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .map(|prop| prop.js_name)
        .unwrap_or("")
}

pub(crate) fn qt_widget_prop_payload_kind(kind_tag: u8, index: usize) -> u8 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .map(|prop| match prop.value_type.repr() {
            crate::bootstrap::QtValueRepr::String => 1,
            crate::bootstrap::QtValueRepr::Bool => 2,
            crate::bootstrap::QtValueRepr::I32 { .. } => 3,
            crate::bootstrap::QtValueRepr::Enum(_) => 4,
            crate::bootstrap::QtValueRepr::F64 { .. } => 5,
            _ => panic!("unsupported prop payload type"),
        })
        .unwrap_or(0)
}

pub(crate) fn qt_widget_prop_non_negative(kind_tag: u8, index: usize) -> bool {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .map(|prop| prop.value_type.is_non_negative())
        .unwrap_or(false)
}

pub(crate) fn qt_widget_prop_lower_kind(kind_tag: u8, index: usize) -> u8 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .map(|prop| match prop.lowering {
            crate::bootstrap::PropLowering::MetaProperty(_) => {
                crate::bootstrap::PropLowerKind::MetaProperty as u8
            }
            crate::bootstrap::PropLowering::Custom(_) => {
                crate::bootstrap::PropLowerKind::Custom as u8
            }
        })
        .unwrap_or(0)
}

pub(crate) fn qt_widget_prop_lower_name(kind_tag: u8, index: usize) -> &'static str {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .map(|prop| match prop.lowering {
            crate::bootstrap::PropLowering::MetaProperty(name)
            | crate::bootstrap::PropLowering::Custom(name) => name,
        })
        .unwrap_or("")
}

pub(crate) fn qt_widget_prop_read_lower_kind(kind_tag: u8, index: usize) -> u8 {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .and_then(|prop| prop.read_lowering)
        .map(|lowering| match lowering {
            crate::bootstrap::PropLowering::MetaProperty(_) => {
                crate::bootstrap::PropLowerKind::MetaProperty as u8
            }
            crate::bootstrap::PropLowering::Custom(_) => {
                crate::bootstrap::PropLowerKind::Custom as u8
            }
        })
        .unwrap_or(0)
}

pub(crate) fn qt_widget_prop_read_lower_name(kind_tag: u8, index: usize) -> &'static str {
    widget_binding(kind_tag)
        .and_then(|binding| binding.props.get(index))
        .and_then(|prop| prop.read_lowering)
        .map(|lowering| match lowering {
            crate::bootstrap::PropLowering::MetaProperty(name)
            | crate::bootstrap::PropLowering::Custom(name) => name,
        })
        .unwrap_or("")
}

pub(crate) fn window_host_pump_zero_timeout() -> bool {
    crate::window_host::ffi_pump_zero_timeout()
}

pub(crate) fn window_host_supports_zero_timeout_pump() -> bool {
    crate::window_host::ffi_supports_zero_timeout_pump()
}

pub(crate) fn window_host_supports_external_wake() -> bool {
    crate::window_host::ffi_supports_external_wake()
}

pub(crate) fn window_host_wait_bridge_kind_tag() -> u8 {
    crate::window_host::ffi_wait_bridge_kind_tag()
}

pub(crate) fn window_host_wait_bridge_unix_fd() -> i32 {
    crate::window_host::ffi_wait_bridge_unix_fd()
}

pub(crate) fn window_host_wait_bridge_windows_handle() -> u64 {
    crate::window_host::ffi_wait_bridge_windows_handle()
}

pub(crate) fn window_host_request_wake() {
    crate::window_host::ffi_request_wake();
}

pub(crate) fn window_host_request_native_wait_once() {
    crate::window_host::ffi_request_native_wait_once();
}
