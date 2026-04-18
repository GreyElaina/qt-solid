use std::collections::BTreeSet;

use crate::schema::{
    EventLowering, OpaqueCodegenDecl, PropLowering, PropMeta, QtTypeInfo, QtValueRepr,
    SpecHostMethodArg, SpecHostMethodMeta, SpecOpaqueDecl, WidgetBinding, WidgetCodegenDecl,
    WidgetHostEventMountCodegenMeta, WidgetHostOverrideCodegenMeta,
    WidgetHostPropGetterCodegenMeta, WidgetHostPropSetterCodegenMeta, WidgetLayoutKind,
    all_opaque_codegen_decls, all_opaque_decls, all_widget_bindings, all_widget_codegen_decls,
    widget_registry,
};

fn cpp_widget_variant(binding: &WidgetBinding, index: usize) -> String {
    format!(
        "Widget{}_{}",
        index + 1,
        sanitize_cpp_ident(binding.type_name)
    )
}

fn sanitize_cpp_ident(value: &str) -> String {
    let mut out = String::new();

    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            if index == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        "_Widget".to_owned()
    } else {
        out
    }
}

fn uses_box_layout(binding: &WidgetBinding) -> bool {
    binding.default_layout == Some(WidgetLayoutKind::Box)
}

fn find_widget_codegen_decl(binding: &WidgetBinding) -> Option<&'static WidgetCodegenDecl> {
    all_widget_codegen_decls()
        .iter()
        .copied()
        .find(|decl| decl.spec_key == binding.spec_key)
}

fn widget_override_class_name(binding: &WidgetBinding, index: usize) -> String {
    format!("RustHostOverride_{}", cpp_widget_variant(binding, index))
}

fn widget_ctor_expr(binding: &WidgetBinding, index: usize, owned: bool) -> String {
    if find_widget_codegen_decl(binding)
        .is_some_and(|decl| !decl.host_overrides.overrides.is_empty())
    {
        let class_name = widget_override_class_name(binding, index);
        return if owned {
            format!("std::make_unique<{class_name}>()")
        } else {
            format!("new {class_name}()")
        };
    }

    match binding.host.factory {
        Some("window.host") => {
            if owned {
                "std::make_unique<HostWindowWidget>()".to_owned()
            } else {
                "new HostWindowWidget()".to_owned()
            }
        }
        None => {
            if owned {
                format!("std::make_unique<{}>()", binding.host.class)
            } else {
                format!("new {}()", binding.host.class)
            }
        }
        Some(factory) => panic!("unsupported widget host factory {factory}"),
    }
}

fn indent_block(block: &str, prefix: &str) -> String {
    let mut out = String::new();
    for line in block.lines() {
        out.push_str(prefix);
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn render_override_callback_bridge(override_meta: &WidgetHostOverrideCodegenMeta) -> String {
    if override_meta.opaque.borrow() != qt_solid_widget_core::runtime::QtOpaqueBorrow::Mut {
        panic!(
            "host override {} uses unsupported opaque source {} with {:?} borrow",
            override_meta.rust_name,
            override_meta.opaque.cxx_class(),
            override_meta.opaque.borrow(),
        );
    }

    format!(
        "    auto dispatch_rust = [&](::{cxx_class} &opaque) {{\n      qt_solid_spike::qt::{bridge_fn}(node_id_, kind_tag_, rust::Str(\"{target}\"), opaque);\n    }};\n",
        cxx_class = override_meta.opaque.cxx_class(),
        bridge_fn = override_meta.bridge_fn,
        target = override_meta.target_name,
    )
}

fn render_widget_override_method(
    _binding: &WidgetBinding,
    override_meta: &WidgetHostOverrideCodegenMeta,
) -> String {
    format!(
        "protected:\n  {signature} override {{\n    auto &self = *this;\n{callback_bridge}    try {{\n{body}    }} catch (const rust::Error &error) {{\n      qWarning() << \"qt host override failed:\" << error.what();\n    }}\n  }}\n",
        signature = override_meta.signature,
        callback_bridge = render_override_callback_bridge(override_meta),
        body = indent_block(override_meta.lowering.body, "      "),
    )
}

fn render_widget_override_class(binding: &WidgetBinding, index: usize) -> Option<String> {
    let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
        return None;
    };
    if codegen_decl.host_overrides.overrides.is_empty() {
        return None;
    }

    let class_name = widget_override_class_name(binding, index);
    let methods = codegen_decl
        .host_overrides
        .overrides
        .iter()
        .map(|override_meta| render_widget_override_method(binding, override_meta))
        .collect::<Vec<_>>()
        .join("");

    Some(format!(
        "class {class_name} final : public {host_class}, public RustWidgetBindingHost {{\npublic:\n  explicit {class_name}(QWidget *parent = nullptr) : {host_class}(parent) {{}}\n\n  void bind_rust_widget(std::uint32_t node_id, std::uint8_t kind_tag) override {{\n    node_id_ = node_id;\n    kind_tag_ = kind_tag;\n  }}\n\n{methods}private:\n  std::uint32_t node_id_ = 0;\n  std::uint8_t kind_tag_ = 0;\n}};\n",
        class_name = class_name,
        host_class = binding.host.class,
        methods = methods,
    ))
}

fn find_host_event_mount<'a>(
    binding: &'a WidgetBinding,
    lower_name: &str,
) -> Option<&'a WidgetHostEventMountCodegenMeta> {
    find_widget_codegen_decl(binding).and_then(|decl| {
        decl.host_event_mounts
            .mounts
            .iter()
            .find(|mount| mount.event_lower_name == lower_name)
    })
}

fn validate_host_event_mounts(binding: &WidgetBinding) {
    for event in &binding.events {
        let EventLowering::Custom(lower_name) = event.lowering else {
            continue;
        };

        if find_host_event_mount(binding, lower_name).is_none() {
            panic!(
                "widget {} custom event {} is missing #[qt(export = event(...))] host spec",
                binding.type_name, lower_name
            );
        }
    }

    let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
        return;
    };

    for mount in codegen_decl.host_event_mounts.mounts {
        let exists = binding.events.iter().any(|event| {
            matches!(event.lowering, EventLowering::Custom(lower_name) if lower_name == mount.event_lower_name)
        });
        if !exists {
            panic!(
                "widget {} host event spec {} has no matching #[qt(host)] event method",
                binding.type_name, mount.event_lower_name
            );
        }
    }
}

fn render_event_mount_emit_bridge() -> &'static str {
    "      const auto event_binding = event;\n      const std::string event_detail = event_trace_detail(event_binding);\n      auto dispatch_event = [id, kind_tag, event_index, event_binding, event_detail](const auto &...values) {\n        const auto trace_id = qt_solid_spike::qt::next_trace_id();\n        qt_solid_spike::qt::trace_cpp_stage(\n            trace_id, rust::Str(\"cpp.signal.enter\"), id, 0,\n            rust::Str(event_detail.c_str(), event_detail.size()));\n        if constexpr (sizeof...(values) == 0) {\n          emit_marshaled_listener_event(id, kind_tag, event_index, trace_id);\n        } else {\n          emit_marshaled_listener_event(id, kind_tag, event_index, trace_id,\n                                        event_binding, values...);\n        }\n        qt_solid_spike::qt::trace_cpp_stage(\n            trace_id, rust::Str(\"cpp.signal.exit\"), id, 0,\n            rust::Str(event_detail.c_str(), event_detail.size()));\n      };\n"
}

fn render_host_event_mount_case(
    binding: &WidgetBinding,
    mount: &WidgetHostEventMountCodegenMeta,
) -> String {
    format!(
        "    if (event.lower_name == \"{lower_name}\") {{\n      auto *typed_widget = dynamic_cast<{host_class} *>(widget);\n      if (typed_widget == nullptr) {{\n        throw_error(\"Qt host event source requires {host_class}\");\n      }}\n      auto &self = *typed_widget;\n{emit_bridge}{body}      return true;\n    }}\n",
        lower_name = mount.event_lower_name,
        host_class = binding.host.class,
        emit_bridge = render_event_mount_emit_bridge(),
        body = indent_block(mount.lowering.body, "      "),
    )
}

fn render_widget_event_mount_dispatch_cpp() -> String {
    let mut cases = Vec::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        validate_host_event_mounts(binding);
        let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
            continue;
        };
        if codegen_decl.host_event_mounts.mounts.is_empty() {
            continue;
        }

        let variant = cpp_widget_variant(binding, index);
        let mounts = codegen_decl
            .host_event_mounts
            .mounts
            .iter()
            .map(|mount| render_host_event_mount_case(binding, mount))
            .collect::<Vec<_>>()
            .join("");
        cases.push(format!(
            "  case WidgetKind::{variant}: {{\n{mounts}    return false;\n  }}\n",
            variant = variant,
            mounts = mounts,
        ));
    }

    if cases.is_empty() {
        return "bool wire_generated_host_event(std::uint32_t, std::uint8_t, QObject *, const CompiledEventBinding &) {\n  return false;\n}\n".to_owned();
    }

    format!(
        "bool wire_generated_host_event(std::uint32_t id, std::uint8_t kind_tag,\n                               QObject *widget,\n                               const CompiledEventBinding &event) {{\n  const auto event_index = event.event_index;\n  switch (widget_kind_from_tag(kind_tag)) {{\n{cases}  }}\n  return false;\n}}\n",
        cases = format!("{}  default:\n    return false;\n", cases.join("")),
    )
}

fn find_bound_prop<'a>(binding: &'a WidgetBinding, lower_name: &str) -> Option<&'a PropMeta> {
    binding
        .props
        .iter()
        .find(|prop| matches!(prop.lowering, PropLowering::Custom(name) if name == lower_name))
}

fn find_read_bound_prop<'a>(binding: &'a WidgetBinding, lower_name: &str) -> Option<&'a PropMeta> {
    binding.props.iter().find(
        |prop| matches!(prop.read_lowering, Some(PropLowering::Custom(name)) if name == lower_name),
    )
}

fn validate_host_prop_codegen(binding: &WidgetBinding) {
    let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
        return;
    };

    for setter in codegen_decl.host_prop_setters.setters {
        let Some(prop) = find_bound_prop(binding, setter.prop_lower_name) else {
            panic!(
                "widget {} host prop setter spec {} has no matching widget prop entry",
                binding.type_name, setter.prop_lower_name
            );
        };
        if prop.value_type != setter.value_type {
            panic!(
                "widget {} host prop setter {} type mismatch: spec uses {}, widget prop entry uses {}",
                binding.type_name,
                setter.prop_lower_name,
                setter.value_type.rust_path(),
                prop.value_type.rust_path(),
            );
        }
    }

    for getter in codegen_decl.host_prop_getters.getters {
        let Some(prop) = find_read_bound_prop(binding, getter.prop_lower_name) else {
            panic!(
                "widget {} host prop getter spec {} has no matching readable widget prop entry",
                binding.type_name, getter.prop_lower_name
            );
        };
        if prop.value_type != getter.value_type {
            panic!(
                "widget {} host prop getter {} type mismatch: spec uses {}, widget prop entry uses {}",
                binding.type_name,
                getter.prop_lower_name,
                getter.value_type.rust_path(),
                prop.value_type.rust_path(),
            );
        }
    }
}

fn prop_payload_kind_expr(value_type: QtTypeInfo, context: &str) -> &'static str {
    match value_type.repr() {
        QtValueRepr::String => "PropPayloadKind::String",
        QtValueRepr::Bool => "PropPayloadKind::Bool",
        QtValueRepr::I32 { .. } => "PropPayloadKind::I32",
        QtValueRepr::Enum(_) => "PropPayloadKind::Enum",
        QtValueRepr::F64 { .. } => "PropPayloadKind::F64",
        repr => panic!("{context} uses unsupported prop repr {:?}", repr),
    }
}

fn cpp_prop_getter_return_type(value_type: QtTypeInfo, context: &str) -> &'static str {
    match value_type.repr() {
        QtValueRepr::String => "rust::String",
        QtValueRepr::Bool => "bool",
        QtValueRepr::I32 { .. } | QtValueRepr::Enum(_) => "std::int32_t",
        QtValueRepr::F64 { .. } => "double",
        repr => panic!("{context} uses unsupported prop getter repr {:?}", repr),
    }
}

fn render_host_prop_setter_case(
    binding: &WidgetBinding,
    setter: &WidgetHostPropSetterCodegenMeta,
) -> String {
    let self_binding = if setter.lowering.body.contains("self") {
        "      auto &self = *typed_widget;\n"
    } else {
        ""
    };

    format!(
        "    if (binding.lower_name == \"{lower_name}\") {{\n      auto *typed_widget = dynamic_cast<{host_class} *>(widget_entry.widget);\n      if (typed_widget == nullptr) {{\n        throw_error(\"Qt host prop setter requires {host_class}\");\n      }}\n{self_binding}{body}      return true;\n    }}\n",
        lower_name = setter.prop_lower_name,
        host_class = binding.host.class,
        self_binding = self_binding,
        body = indent_block(setter.lowering.body, "      "),
    )
}

fn render_host_prop_getter_case(
    binding: &WidgetBinding,
    getter: &WidgetHostPropGetterCodegenMeta,
) -> String {
    let return_type = cpp_prop_getter_return_type(
        getter.value_type,
        &format!(
            "widget {} host prop getter {}",
            binding.type_name, getter.prop_lower_name
        ),
    );
    let self_binding = if getter.lowering.body.contains("self") {
        "      auto &self = *typed_widget;\n"
    } else {
        ""
    };

    format!(
        "    if (binding.read_lower_name == \"{lower_name}\") {{\n      auto *typed_widget = dynamic_cast<{host_class} *>(widget_entry.widget);\n      if (typed_widget == nullptr) {{\n        throw_error(\"Qt host prop getter requires {host_class}\");\n      }}\n{self_binding}      *out = ([&]() -> {return_type} {{\n{body}      }})();\n      return true;\n    }}\n",
        lower_name = getter.prop_lower_name,
        host_class = binding.host.class,
        self_binding = self_binding,
        return_type = return_type,
        body = indent_block(getter.lowering.body, "        "),
    )
}

fn render_prop_support_case(lower_name: &str, payload_kind: &str) -> String {
    format!(
        "    if (lower_name == \"{lower_name}\") {{\n      return payload_kind == {payload_kind};\n    }}\n",
        lower_name = lower_name,
        payload_kind = payload_kind,
    )
}

fn render_widget_prop_support_dispatch_cpp(read: bool) -> String {
    let mut cases = Vec::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        validate_host_prop_codegen(binding);
        let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
            continue;
        };
        let metas = if read {
            codegen_decl
                .host_prop_getters
                .getters
                .iter()
                .map(|getter| {
                    (
                        getter.prop_lower_name,
                        prop_payload_kind_expr(
                            getter.value_type,
                            &format!(
                                "widget {} host prop getter {}",
                                binding.type_name, getter.prop_lower_name
                            ),
                        ),
                    )
                })
                .collect::<Vec<_>>()
        } else {
            codegen_decl
                .host_prop_setters
                .setters
                .iter()
                .map(|setter| {
                    (
                        setter.prop_lower_name,
                        prop_payload_kind_expr(
                            setter.value_type,
                            &format!(
                                "widget {} host prop setter {}",
                                binding.type_name, setter.prop_lower_name
                            ),
                        ),
                    )
                })
                .collect::<Vec<_>>()
        };
        if metas.is_empty() {
            continue;
        }

        let variant = cpp_widget_variant(binding, index);
        let body = metas
            .into_iter()
            .map(|(lower_name, payload_kind)| render_prop_support_case(lower_name, payload_kind))
            .collect::<Vec<_>>()
            .join("");
        cases.push(format!(
            "  case WidgetKind::{variant}: {{\n{body}    return false;\n  }}\n",
            variant = variant,
            body = body,
        ));
    }

    let fn_name = if read {
        "supports_generated_host_prop_reading"
    } else {
        "supports_generated_host_prop_lowering"
    };

    if cases.is_empty() {
        return format!(
            "bool {fn_name}(WidgetKind, std::string_view, PropPayloadKind) {{\n  return false;\n}}\n",
        );
    }

    format!(
        "bool {fn_name}(WidgetKind kind, std::string_view lower_name,\n                                 PropPayloadKind payload_kind) {{\n  switch (kind) {{\n{cases}  default:\n    return false;\n  }}\n}}\n",
        fn_name = fn_name,
        cases = cases.join(""),
    )
}

fn render_widget_prop_apply_dispatch_cpp(
    fn_name: &str,
    value_type_match: fn(QtTypeInfo) -> bool,
    value_cpp_type: &str,
) -> String {
    let mut cases = Vec::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        validate_host_prop_codegen(binding);
        let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
            continue;
        };
        let setters = codegen_decl
            .host_prop_setters
            .setters
            .iter()
            .filter(|setter| value_type_match(setter.value_type))
            .map(|setter| render_host_prop_setter_case(binding, setter))
            .collect::<Vec<_>>()
            .join("");
        if setters.is_empty() {
            continue;
        }

        let variant = cpp_widget_variant(binding, index);
        cases.push(format!(
            "  case WidgetKind::{variant}: {{\n{setters}    return false;\n  }}\n",
            variant = variant,
            setters = setters,
        ));
    }

    if cases.is_empty() {
        return format!(
            "bool {fn_name}(WidgetEntry &, const CompiledPropBinding &, {value_cpp_type}) {{\n  return false;\n}}\n",
            fn_name = fn_name,
            value_cpp_type = value_cpp_type,
        );
    }

    format!(
        "bool {fn_name}(WidgetEntry &widget_entry,\n                              const CompiledPropBinding &binding,\n                              {value_cpp_type} value) {{\n  switch (widget_entry.kind) {{\n{cases}  default:\n    return false;\n  }}\n}}\n",
        fn_name = fn_name,
        value_cpp_type = value_cpp_type,
        cases = cases.join(""),
    )
}

fn render_widget_prop_read_dispatch_cpp(
    fn_name: &str,
    value_type_match: fn(QtTypeInfo) -> bool,
    value_cpp_type: &str,
) -> String {
    let mut cases = Vec::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        validate_host_prop_codegen(binding);
        let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
            continue;
        };
        let getters = codegen_decl
            .host_prop_getters
            .getters
            .iter()
            .filter(|getter| value_type_match(getter.value_type))
            .map(|getter| render_host_prop_getter_case(binding, getter))
            .collect::<Vec<_>>()
            .join("");
        if getters.is_empty() {
            continue;
        }

        let variant = cpp_widget_variant(binding, index);
        cases.push(format!(
            "  case WidgetKind::{variant}: {{\n{getters}    return false;\n  }}\n",
            variant = variant,
            getters = getters,
        ));
    }

    if cases.is_empty() {
        return format!(
            "bool {fn_name}(WidgetEntry &, const CompiledPropBinding &, {value_cpp_type} *) {{\n  return false;\n}}\n",
            fn_name = fn_name,
            value_cpp_type = value_cpp_type,
        );
    }

    format!(
        "bool {fn_name}(WidgetEntry &widget_entry,\n                             const CompiledPropBinding &binding,\n                             {value_cpp_type} *out) {{\n  switch (widget_entry.kind) {{\n{cases}  default:\n    return false;\n  }}\n}}\n",
        fn_name = fn_name,
        value_cpp_type = value_cpp_type,
        cases = cases.join(""),
    )
}

fn render_widget_prop_dispatch_impl_cpp() -> String {
    fn is_string(value_type: QtTypeInfo) -> bool {
        matches!(value_type.repr(), QtValueRepr::String)
    }
    fn is_bool(value_type: QtTypeInfo) -> bool {
        matches!(value_type.repr(), QtValueRepr::Bool)
    }
    fn is_i32(value_type: QtTypeInfo) -> bool {
        matches!(
            value_type.repr(),
            QtValueRepr::I32 { .. } | QtValueRepr::Enum(_)
        )
    }
    fn is_f64(value_type: QtTypeInfo) -> bool {
        matches!(value_type.repr(), QtValueRepr::F64 { .. })
    }

    format!(
        "{write_support}\n{read_support}\n{apply_string}\n{apply_bool}\n{apply_i32}\n{apply_f64}\n{read_string}\n{read_bool}\n{read_i32}\n{read_f64}\n",
        write_support = render_widget_prop_support_dispatch_cpp(false),
        read_support = render_widget_prop_support_dispatch_cpp(true),
        apply_string = render_widget_prop_apply_dispatch_cpp(
            "apply_generated_string_prop",
            is_string,
            "rust::Str",
        ),
        apply_bool =
            render_widget_prop_apply_dispatch_cpp("apply_generated_bool_prop", is_bool, "bool",),
        apply_i32 = render_widget_prop_apply_dispatch_cpp(
            "apply_generated_i32_prop",
            is_i32,
            "std::int32_t",
        ),
        apply_f64 =
            render_widget_prop_apply_dispatch_cpp("apply_generated_f64_prop", is_f64, "double",),
        read_string = render_widget_prop_read_dispatch_cpp(
            "read_generated_string_prop",
            is_string,
            "rust::String",
        ),
        read_bool =
            render_widget_prop_read_dispatch_cpp("read_generated_bool_prop", is_bool, "bool",),
        read_i32 = render_widget_prop_read_dispatch_cpp(
            "read_generated_i32_prop",
            is_i32,
            "std::int32_t",
        ),
        read_f64 =
            render_widget_prop_read_dispatch_cpp("read_generated_f64_prop", is_f64, "double",),
    )
}

fn render_widget_probe_case(binding: &WidgetBinding, index: usize) -> String {
    let variant = cpp_widget_variant(binding, index);
    let expr = widget_ctor_expr(binding, index, true);

    format!(
        "case WidgetKind::{variant}:\n    return {expr};",
        variant = variant,
        expr = expr,
    )
}

fn render_widget_create_case(binding: &WidgetBinding, index: usize) -> String {
    let variant = cpp_widget_variant(binding, index);
    let widget_expr = widget_ctor_expr(binding, index, false);

    let mut out = String::new();
    out.push_str(&format!(
        "case WidgetKind::{variant}: {{\n",
        variant = variant
    ));

    if uses_box_layout(binding) {
        out.push_str(&format!(
            "  auto *widget = {widget_expr};\n",
            widget_expr = widget_expr
        ));
        out.push_str("  auto *layout = new QBoxLayout(QBoxLayout::TopToBottom);\n");
        if binding.host.factory == Some("window.host") {
            out.push_str("  layout->setSizeConstraint(QLayout::SetNoConstraint);\n");
        }
        out.push_str("  widget->setLayout(layout);\n");
        out.push_str("  entry.widget = widget;\n");
        out.push_str("  entry.layout = layout;\n");
        out.push_str("  apply_layout_style(entry);\n");
    } else {
        out.push_str(&format!(
            "  entry.widget = {widget_expr};\n",
            widget_expr = widget_expr
        ));
    }

    out.push_str("  break;\n}\n");
    out
}

#[derive(Default)]
struct WidgetHostMethodHelperUsage {
    return_string: bool,
    return_bool: bool,
    return_i32: bool,
    return_enum: bool,
    return_f64: bool,
    arg_string: bool,
    arg_bool: bool,
    arg_i32: bool,
    arg_f64: bool,
}

fn record_widget_host_method_helper_usage(
    usage: &mut WidgetHostMethodHelperUsage,
    method: &SpecHostMethodMeta,
) {
    match method.return_type.repr() {
        QtValueRepr::Unit => {}
        QtValueRepr::String => usage.return_string = true,
        QtValueRepr::Bool => usage.return_bool = true,
        QtValueRepr::I32 { .. } => usage.return_i32 = true,
        QtValueRepr::Enum(_) => usage.return_enum = true,
        QtValueRepr::F64 { .. } => usage.return_f64 = true,
        repr => panic!(
            "widget host method {} uses unsupported return repr {:?}",
            method.host_name, repr
        ),
    }

    for arg in method.args {
        match arg.value_type.repr() {
            QtValueRepr::String => usage.arg_string = true,
            QtValueRepr::Bool => usage.arg_bool = true,
            QtValueRepr::I32 { .. } => usage.arg_i32 = true,
            QtValueRepr::F64 { .. } => usage.arg_f64 = true,
            QtValueRepr::Enum(_) => panic!(
                "widget host method {} arg {} uses unsupported enum repr for generated dispatch",
                method.host_name, arg.rust_name
            ),
            repr => panic!(
                "widget host method {} arg {} uses unsupported repr {:?}",
                method.host_name, arg.rust_name, repr
            ),
        }
    }
}

fn render_widget_host_method_helper_defs(usage: &WidgetHostMethodHelperUsage) -> String {
    let mut out = String::new();

    if usage.return_string {
        out.push_str(
            "static QtMethodValue widget_method_string(rust::String value) {\n  return QtMethodValue{.kind_tag = 1,\n                       .string_value = std::move(value),\n                       .bool_value = false,\n                       .i32_value = 0,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }
    if usage.return_bool {
        out.push_str(
            "static QtMethodValue widget_method_bool(bool value) {\n  return QtMethodValue{.kind_tag = 2,\n                       .string_value = rust::String(),\n                       .bool_value = value,\n                       .i32_value = 0,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }
    if usage.return_i32 {
        out.push_str(
            "static QtMethodValue widget_method_i32(std::int32_t value) {\n  return QtMethodValue{.kind_tag = 3,\n                       .string_value = rust::String(),\n                       .bool_value = false,\n                       .i32_value = value,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }
    if usage.return_f64 {
        out.push_str(
            "static QtMethodValue widget_method_f64(double value) {\n  return QtMethodValue{.kind_tag = 4,\n                       .string_value = rust::String(),\n                       .bool_value = false,\n                       .i32_value = 0,\n                       .f64_value = value};\n}\n\n",
        );
    }
    if usage.return_enum {
        out.push_str(
            "static QtMethodValue widget_method_enum(std::int32_t value) {\n  return QtMethodValue{.kind_tag = 5,\n                       .string_value = rust::String(),\n                       .bool_value = false,\n                       .i32_value = value,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }

    if usage.arg_string {
        out.push_str(
            "static rust::Str expect_widget_method_string_arg(const rust::Vec<QtMethodValue> &args,\n                                          std::size_t index,\n                                          std::size_t expected_len,\n                                          const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt host method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 1) {\n    throw_error(\"Qt host method expected string argument\");\n  }\n\n  (void)method_name;\n  return value.string_value;\n}\n\n",
        );
    }
    if usage.arg_bool {
        out.push_str(
            "static bool expect_widget_method_bool_arg(const rust::Vec<QtMethodValue> &args,\n                                   std::size_t index,\n                                   std::size_t expected_len,\n                                   const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt host method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 2) {\n    throw_error(\"Qt host method expected bool argument\");\n  }\n\n  (void)method_name;\n  return value.bool_value;\n}\n\n",
        );
    }
    if usage.arg_i32 {
        out.push_str(
            "static std::int32_t expect_widget_method_i32_arg(const rust::Vec<QtMethodValue> &args,\n                                          std::size_t index,\n                                          std::size_t expected_len,\n                                          const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt host method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 3) {\n    throw_error(\"Qt host method expected i32 argument\");\n  }\n\n  (void)method_name;\n  return value.i32_value;\n}\n\n",
        );
    }
    if usage.arg_f64 {
        out.push_str(
            "static double expect_widget_method_f64_arg(const rust::Vec<QtMethodValue> &args,\n                                    std::size_t index,\n                                    std::size_t expected_len,\n                                    const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt host method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 4) {\n    throw_error(\"Qt host method expected f64 argument\");\n  }\n\n  (void)method_name;\n  return value.f64_value;\n}\n\n",
        );
    }

    out
}

fn render_widget_method_arg_expr(method: &SpecHostMethodMeta, index: usize) -> String {
    let arg = &method.args[index];
    match arg.value_type.repr() {
        QtValueRepr::String => format!(
            "to_qstring(expect_widget_method_string_arg(args, {index}, {arg_count}, \"{method_name}\"))",
            index = index,
            arg_count = method.args.len(),
            method_name = method.host_name,
        ),
        QtValueRepr::Bool => format!(
            "expect_widget_method_bool_arg(args, {index}, {arg_count}, \"{method_name}\")",
            index = index,
            arg_count = method.args.len(),
            method_name = method.host_name,
        ),
        QtValueRepr::I32 { .. } => format!(
            "expect_widget_method_i32_arg(args, {index}, {arg_count}, \"{method_name}\")",
            index = index,
            arg_count = method.args.len(),
            method_name = method.host_name,
        ),
        QtValueRepr::F64 { .. } => format!(
            "expect_widget_method_f64_arg(args, {index}, {arg_count}, \"{method_name}\")",
            index = index,
            arg_count = method.args.len(),
            method_name = method.host_name,
        ),
        QtValueRepr::Enum(_) => panic!(
            "widget host method {} arg {} uses unsupported enum repr for generated dispatch",
            method.host_name, arg.rust_name
        ),
        repr => panic!(
            "widget host method {} arg {} uses unsupported repr {:?}",
            method.host_name, arg.rust_name, repr
        ),
    }
}

fn render_widget_host_method_case(method: &SpecHostMethodMeta) -> String {
    let args = (0..method.args.len())
        .map(|index| render_widget_method_arg_expr(method, index))
        .collect::<Vec<_>>()
        .join(", ");
    let invoke = format!("self.{}({})", method.host_name, args);
    let arg_check = if method.args.is_empty() {
        format!(
            "        if (args.size() != 0) {{\n          throw_error(\"Qt host method {} expects no arguments\");\n        }}\n",
            method.host_name
        )
    } else {
        String::new()
    };

    let body = match method.return_type.repr() {
        QtValueRepr::Unit => {
            format!("{arg_check}        {invoke};\n        return method_unit();\n")
        }
        QtValueRepr::String => format!(
            "{arg_check}        const auto __qt_return_value = {invoke};\n        return widget_method_string(to_rust_string(__qt_return_value));\n"
        ),
        QtValueRepr::Bool => format!(
            "{arg_check}        const auto __qt_return_value = {invoke};\n        return widget_method_bool(__qt_return_value);\n"
        ),
        QtValueRepr::I32 { .. } => format!(
            "{arg_check}        const auto __qt_return_value = {invoke};\n        return widget_method_i32(__qt_return_value);\n"
        ),
        QtValueRepr::Enum(_) => format!(
            "{arg_check}        const auto __qt_return_value = {invoke};\n        return widget_method_enum(static_cast<std::int32_t>(__qt_return_value));\n"
        ),
        QtValueRepr::F64 { .. } => format!(
            "{arg_check}        const auto __qt_return_value = {invoke};\n        return widget_method_f64(__qt_return_value);\n"
        ),
        repr => panic!(
            "widget host method {} uses unsupported return repr {:?}",
            method.host_name, repr
        ),
    };

    format!(
        "      case {slot}: {{\n{body}      }}\n",
        slot = method.slot,
        body = body,
    )
}

fn render_widget_host_method_dispatch_impl_cpp() -> String {
    let mut usage = WidgetHostMethodHelperUsage::default();
    let mut cases = Vec::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        if binding.methods.host_methods.is_empty() {
            continue;
        }

        for method in binding.methods.host_methods {
            record_widget_host_method_helper_usage(&mut usage, method);
        }

        let variant = cpp_widget_variant(binding, index);
        let method_cases = binding
            .methods
            .host_methods
            .iter()
            .map(render_widget_host_method_case)
            .collect::<Vec<_>>()
            .join("");

        cases.push(format!(
            "    case WidgetKind::{variant}: {{\n      auto *typed_widget = dynamic_cast<{host_class} *>(widget.widget);\n      if (typed_widget == nullptr) {{\n        throw_error(\"Qt host method dispatch requires {host_class}\");\n      }}\n      auto &self = *typed_widget;\n      switch (slot) {{\n{method_cases}        default:\n          break;\n      }}\n      break;\n    }}\n",
            variant = variant,
            host_class = binding.host.class,
            method_cases = method_cases,
        ));
    }

    if cases.is_empty() {
        return "QtMethodValue call_generated_host_slot(const WidgetEntry &, std::uint16_t,\n                                      const rust::Vec<QtMethodValue> &) {\n  throw_error(\"Qt host contract has no host method slot\");\n}\n"
            .to_owned();
    }

    format!(
        "{helpers}QtMethodValue call_generated_host_slot(const WidgetEntry &widget, std::uint16_t slot,\n                                      const rust::Vec<QtMethodValue> &args) {{\n  switch (widget.kind) {{\n{cases}    default:\n      break;\n  }}\n  throw_error(\"Qt host contract has no host method slot\");\n}}\n",
        helpers = render_widget_host_method_helper_defs(&usage),
        cases = cases.join(""),
    )
}

pub fn render_widget_host_includes_cpp() -> String {
    let mut includes = BTreeSet::new();

    for binding in all_widget_bindings() {
        if binding.host.factory == Some("window.host") {
            continue;
        }
        includes.insert(binding.host.include);
    }

    for binding in all_widget_bindings() {
        let Some(codegen_decl) = find_widget_codegen_decl(binding) else {
            continue;
        };
        for override_meta in codegen_decl.host_overrides.overrides {
            includes.extend(override_meta.lowering.extra_includes.iter().copied());
            includes.insert(override_meta.opaque.cxx_include());
        }
        for mount in codegen_decl.host_event_mounts.mounts {
            includes.extend(mount.lowering.extra_includes.iter().copied());
        }
        for setter in codegen_decl.host_prop_setters.setters {
            includes.extend(setter.lowering.extra_includes.iter().copied());
        }
        for getter in codegen_decl.host_prop_getters.getters {
            includes.extend(getter.lowering.extra_includes.iter().copied());
        }
    }

    for codegen_decl in all_opaque_codegen_decls() {
        includes.insert(codegen_decl.opaque.cxx_include());
        for method in codegen_decl.methods.methods {
            if let Some(lowering) = method.lowering {
                includes.extend(lowering.extra_includes.iter().copied());
            }
        }
    }

    let mut out = String::new();
    for include in includes {
        out.push_str("#include ");
        out.push_str(include);
        out.push('\n');
    }
    out
}

pub fn render_widget_override_classes_cpp() -> String {
    all_widget_bindings()
        .iter()
        .enumerate()
        .filter_map(|(index, binding)| render_widget_override_class(binding, index))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_widget_event_mounts_cpp() -> String {
    render_widget_event_mount_dispatch_cpp()
}

pub fn render_widget_prop_dispatch_cpp() -> String {
    render_widget_prop_dispatch_impl_cpp()
}

pub fn render_widget_host_method_dispatch_cpp() -> String {
    render_widget_host_method_dispatch_impl_cpp()
}

pub fn render_widget_kind_enum_cpp() -> String {
    let mut out = String::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        let variant = cpp_widget_variant(binding, index);
        let value = widget_registry().host_tag(binding.widget_type_id);
        out.push_str(&format!(
            "{variant} = {value},\n",
            variant = variant,
            value = value
        ));
    }

    out
}

pub fn render_widget_kind_from_tag_cpp() -> String {
    let mut out = String::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        let variant = cpp_widget_variant(binding, index);
        let value = widget_registry().host_tag(binding.widget_type_id);
        out.push_str(&format!(
            "case {value}:\n  return WidgetKind::{variant};\n",
            value = value,
            variant = variant,
        ));
    }

    out
}

pub fn render_widget_kind_values_cpp() -> String {
    let values = all_widget_bindings()
        .iter()
        .enumerate()
        .map(|(index, binding)| cpp_widget_variant(binding, index))
        .map(|variant| format!("    WidgetKind::{variant}", variant = variant))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        "constexpr std::array<WidgetKind, {count}> kAllWidgetKinds = {{\n{values}\n}};\n",
        count = all_widget_bindings().len(),
        values = values,
    )
}

pub fn render_widget_top_level_cases_cpp() -> String {
    let mut out = String::new();

    for (index, binding) in all_widget_bindings().iter().enumerate() {
        let variant = cpp_widget_variant(binding, index);
        let top_level = if binding.host.top_level {
            "true"
        } else {
            "false"
        };
        out.push_str(&format!(
            "case WidgetKind::{variant}:\n  return {top_level};\n",
            variant = variant,
            top_level = top_level,
        ));
    }

    out
}

pub fn render_widget_probe_cases_cpp() -> String {
    all_widget_bindings()
        .iter()
        .enumerate()
        .map(|(index, binding)| render_widget_probe_case(binding, index))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_widget_create_cases_cpp() -> String {
    all_widget_bindings()
        .iter()
        .enumerate()
        .map(|(index, binding)| render_widget_create_case(binding, index))
        .collect::<Vec<_>>()
        .join("\n")
}

fn cpp_return_type(method: &SpecHostMethodMeta) -> &'static str {
    match method.return_type.repr() {
        QtValueRepr::Unit => "void",
        QtValueRepr::String => "rust::String",
        QtValueRepr::Bool => "bool",
        QtValueRepr::I32 { .. } | QtValueRepr::Enum(_) => "std::int32_t",
        QtValueRepr::F64 { .. } => "double",
        repr => panic!(
            "opaque host method {} uses unsupported return repr {:?}",
            method.host_name, repr
        ),
    }
}

fn render_method_value_ctor(method: &SpecHostMethodMeta) -> &'static str {
    match method.return_type.repr() {
        QtValueRepr::Unit => "method_unit",
        QtValueRepr::String => "method_string",
        QtValueRepr::Bool => "method_bool",
        QtValueRepr::I32 { .. } => "method_i32",
        QtValueRepr::Enum(_) => "method_enum",
        QtValueRepr::F64 { .. } => "method_f64",
        repr => panic!(
            "opaque host method {} uses unsupported return repr {:?}",
            method.host_name, repr
        ),
    }
}

fn render_method_arg_unpack(
    method: &SpecHostMethodMeta,
    arg: &SpecHostMethodArg,
    index: usize,
) -> String {
    let helper = match arg.value_type.repr() {
        QtValueRepr::String => "expect_method_string_arg",
        QtValueRepr::Bool => "expect_method_bool_arg",
        QtValueRepr::I32 { .. } => "expect_method_i32_arg",
        QtValueRepr::Enum(_) => "expect_method_enum_arg",
        QtValueRepr::F64 { .. } => "expect_method_f64_arg",
        repr => panic!(
            "opaque host method {} arg {} uses unsupported repr {:?}",
            method.host_name, arg.rust_name, repr
        ),
    };

    format!(
        "    const auto {name} = {helper}(args, {index}, {arg_count}, \"{method_name}\");\n",
        name = arg.rust_name,
        helper = helper,
        index = index,
        arg_count = method.args.len(),
        method_name = method.host_name,
    )
}

fn render_method_body(method: &SpecHostMethodMeta, body: &str) -> String {
    let arg_unpacks = method
        .args
        .iter()
        .enumerate()
        .map(|(index, arg)| render_method_arg_unpack(method, arg, index))
        .collect::<Vec<_>>()
        .join("");

    let return_type = cpp_return_type(method);
    let value_ctor = render_method_value_ctor(method);

    match method.return_type.repr() {
        QtValueRepr::Unit => format!(
            "{arg_unpacks}    ([&]() -> {return_type} {{\n      {body}\n    }})();\n    return {value_ctor}();\n",
            arg_unpacks = arg_unpacks,
            return_type = return_type,
            body = body,
            value_ctor = value_ctor,
        ),
        _ => format!(
            "{arg_unpacks}    const auto __qt_return_value = ([&]() -> {return_type} {{\n      {body}\n    }})();\n    return {value_ctor}(__qt_return_value);\n",
            arg_unpacks = arg_unpacks,
            return_type = return_type,
            body = body,
            value_ctor = value_ctor,
        ),
    }
}

fn find_opaque_decl(codegen_decl: &OpaqueCodegenDecl) -> &'static SpecOpaqueDecl {
    all_opaque_decls()
        .iter()
        .copied()
        .find(|decl| decl.opaque == codegen_decl.opaque)
        .unwrap_or_else(|| {
            panic!(
                "missing opaque spec for {}",
                codegen_decl.opaque.rust_path()
            )
        })
}

fn find_spec_method<'a>(
    decl: &'a SpecOpaqueDecl,
    codegen_method: &crate::schema::OpaqueMethodCodegenMeta,
) -> &'a SpecHostMethodMeta {
    decl.methods
        .methods
        .iter()
        .find(|method| method.slot == codegen_method.slot)
        .unwrap_or_else(|| {
            panic!(
                "missing opaque method spec for {} slot {}",
                decl.opaque.rust_path(),
                codegen_method.slot
            )
        })
}

fn render_opaque_dispatch_case(
    decl: &SpecOpaqueDecl,
    codegen_method: &crate::schema::OpaqueMethodCodegenMeta,
) -> String {
    let method = find_spec_method(decl, codegen_method);
    let Some(lowering) = codegen_method.lowering else {
        panic!(
            "opaque host method {}::{} is missing qt::cpp! lowering",
            decl.opaque.rust_path(),
            method.rust_name
        );
    };
    let body = render_method_body(method, lowering.body);

    format!(
        "  case {slot}: {{\n{body}  }}\n",
        slot = method.slot,
        body = body,
    )
}

#[derive(Default)]
struct OpaqueHelperUsage {
    return_string: bool,
    return_bool: bool,
    return_i32: bool,
    return_enum: bool,
    return_f64: bool,
    arg_string: bool,
    arg_bool: bool,
    arg_i32: bool,
    arg_enum: bool,
    arg_f64: bool,
}

fn record_opaque_helper_usage(usage: &mut OpaqueHelperUsage, method: &SpecHostMethodMeta) {
    match method.return_type.repr() {
        QtValueRepr::Unit => {}
        QtValueRepr::String => usage.return_string = true,
        QtValueRepr::Bool => usage.return_bool = true,
        QtValueRepr::I32 { .. } => usage.return_i32 = true,
        QtValueRepr::Enum(_) => usage.return_enum = true,
        QtValueRepr::F64 { .. } => usage.return_f64 = true,
        repr => panic!(
            "opaque host method {} uses unsupported return repr {:?}",
            method.host_name, repr
        ),
    }

    for arg in method.args {
        match arg.value_type.repr() {
            QtValueRepr::String => usage.arg_string = true,
            QtValueRepr::Bool => usage.arg_bool = true,
            QtValueRepr::I32 { .. } => usage.arg_i32 = true,
            QtValueRepr::Enum(_) => usage.arg_enum = true,
            QtValueRepr::F64 { .. } => usage.arg_f64 = true,
            repr => panic!(
                "opaque host method {} arg {} uses unsupported repr {:?}",
                method.host_name, arg.rust_name, repr
            ),
        }
    }
}

fn render_opaque_helper_defs(usage: &OpaqueHelperUsage) -> String {
    let mut out = String::new();

    if usage.return_string {
        out.push_str(
            "static QtMethodValue method_string(rust::String value) {\n  return QtMethodValue{.kind_tag = 1,\n                       .string_value = std::move(value),\n                       .bool_value = false,\n                       .i32_value = 0,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }
    if usage.return_bool {
        out.push_str(
            "static QtMethodValue method_bool(bool value) {\n  return QtMethodValue{.kind_tag = 2,\n                       .string_value = rust::String(),\n                       .bool_value = value,\n                       .i32_value = 0,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }
    if usage.return_i32 {
        out.push_str(
            "static QtMethodValue method_i32(std::int32_t value) {\n  return QtMethodValue{.kind_tag = 3,\n                       .string_value = rust::String(),\n                       .bool_value = false,\n                       .i32_value = value,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }
    if usage.return_f64 {
        out.push_str(
            "static QtMethodValue method_f64(double value) {\n  return QtMethodValue{.kind_tag = 4,\n                       .string_value = rust::String(),\n                       .bool_value = false,\n                       .i32_value = 0,\n                       .f64_value = value};\n}\n\n",
        );
    }
    if usage.return_enum {
        out.push_str(
            "static QtMethodValue method_enum(std::int32_t value) {\n  return QtMethodValue{.kind_tag = 5,\n                       .string_value = rust::String(),\n                       .bool_value = false,\n                       .i32_value = value,\n                       .f64_value = 0.0};\n}\n\n",
        );
    }

    if usage.arg_string {
        out.push_str(
            "static rust::Str expect_method_string_arg(const rust::Vec<QtMethodValue> &args,\n                                   std::size_t index,\n                                   std::size_t expected_len,\n                                   const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt opaque method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 1) {\n    throw_error(\"Qt opaque method expected string argument\");\n  }\n\n  (void)method_name;\n  return value.string_value;\n}\n\n",
        );
    }
    if usage.arg_bool {
        out.push_str(
            "static bool expect_method_bool_arg(const rust::Vec<QtMethodValue> &args,\n                            std::size_t index,\n                            std::size_t expected_len,\n                            const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt opaque method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 2) {\n    throw_error(\"Qt opaque method expected bool argument\");\n  }\n\n  (void)method_name;\n  return value.bool_value;\n}\n\n",
        );
    }
    if usage.arg_i32 {
        out.push_str(
            "static std::int32_t expect_method_i32_arg(const rust::Vec<QtMethodValue> &args,\n                                   std::size_t index,\n                                   std::size_t expected_len,\n                                   const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt opaque method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 3) {\n    throw_error(\"Qt opaque method expected i32 argument\");\n  }\n\n  (void)method_name;\n  return value.i32_value;\n}\n\n",
        );
    }
    if usage.arg_f64 {
        out.push_str(
            "static double expect_method_f64_arg(const rust::Vec<QtMethodValue> &args,\n                             std::size_t index,\n                             std::size_t expected_len,\n                             const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt opaque method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 4) {\n    throw_error(\"Qt opaque method expected f64 argument\");\n  }\n\n  (void)method_name;\n  return value.f64_value;\n}\n\n",
        );
    }
    if usage.arg_enum {
        out.push_str(
            "static std::int32_t expect_method_enum_arg(const rust::Vec<QtMethodValue> &args,\n                                    std::size_t index,\n                                    std::size_t expected_len,\n                                    const char *method_name) {\n  if (args.size() != expected_len) {\n    throw_error(\"Qt opaque method received wrong argument count\");\n  }\n\n  const auto &value = args[index];\n  if (value.kind_tag != 5) {\n    throw_error(\"Qt opaque method expected enum argument\");\n  }\n\n  (void)method_name;\n  return value.i32_value;\n}\n\n",
        );
    }

    out
}

fn render_opaque_dispatch_fn(codegen_decl: &OpaqueCodegenDecl) -> String {
    let decl = find_opaque_decl(codegen_decl);
    let cases = codegen_decl
        .methods
        .methods
        .iter()
        .map(|method| render_opaque_dispatch_case(decl, method))
        .collect::<Vec<_>>()
        .join("");

    format!(
        "QtMethodValue {fn_name}(::{cxx_class} &self, std::uint16_t slot,\n                         const rust::Vec<QtMethodValue> &args) {{\n  switch (slot) {{\n{cases}  default:\n    throw_error(\"Qt opaque contract has no slot\");\n  }}\n}}\n",
        fn_name = codegen_decl.host_call_fn,
        cxx_class = decl.opaque.cxx_class(),
        cases = cases,
    )
}

pub fn render_opaque_dispatch_cpp() -> String {
    let mut usage = OpaqueHelperUsage::default();
    let functions = all_opaque_codegen_decls()
        .iter()
        .map(|decl| {
            let spec_decl = find_opaque_decl(decl);
            for method in decl.methods.methods {
                if method.lowering.is_some() {
                    record_opaque_helper_usage(&mut usage, find_spec_method(spec_decl, method));
                }
            }
            render_opaque_dispatch_fn(decl)
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("{}{}\n", render_opaque_helper_defs(&usage), functions)
}
