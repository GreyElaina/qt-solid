use std::{
    collections::{BTreeMap, btree_map::Entry},
    fmt::Write,
};

use qt_solid_widget_core::schema::{
    ChildrenKind, EndpointWriteMode, EnumMeta, EventEchoMeta, EventFieldMeta, EventPayloadKind,
    MergedProp, QtTypeInfo, QtValueRepr, SpecHostMethodArg, SpecPropDefaultValue,
    SpecWidgetBinding, WidgetBinding, WidgetLibraryBindings, WidgetRegistry, merged_props,
};

#[derive(Clone)]
struct PropLeafSpec {
    path: Vec<String>,
    key: String,
    prop_id: Option<u16>,
    kind: PropJsValueKind,
    non_negative: bool,
    default: SpecPropDefaultValue,
    method: Option<String>,
    init_method: Option<String>,
    create: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTsExportStyle {
    DirectEntities,
    FunctionBridge,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeTsOptions<'a> {
    pub package_native_specifier: &'a str,
    pub resolve_relative_to_package: &'a str,
    pub resolve_relative_to_source: &'a str,
    pub export_style: NativeTsExportStyle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EventJsValueKind {
    String,
    Boolean,
    Number,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TsScalarKind {
    String,
    Boolean,
    Number,
    Integer,
    Enum(&'static EnumMeta),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PropJsValueKind {
    String,
    Boolean,
    Integer,
    Number,
    Enum(&'static EnumMeta),
}

#[derive(Clone, PartialEq, Eq)]
enum EventPayloadJsSpec {
    Unit,
    Scalar(EventJsValueKind),
    Object(Vec<EventPayloadFieldJsSpec>),
}

#[derive(Clone, PartialEq, Eq)]
struct EventPayloadFieldJsSpec {
    js_name: &'static str,
    kind: EventJsValueKind,
}

#[derive(Clone, PartialEq, Eq)]
struct EventEchoJsSpec {
    prop_js_name: &'static str,
    value_path: &'static str,
    kind: EventJsValueKind,
}

#[derive(Default)]
struct TsModule {
    sections: Vec<String>,
}

impl TsModule {
    fn push(&mut self, snippet: impl AsRef<str>) {
        let snippet = normalize_ts_snippet(snippet.as_ref());
        if !snippet.is_empty() {
            self.sections.push(snippet);
        }
    }

    fn finish(self) -> String {
        if self.sections.is_empty() {
            return String::new();
        }

        let mut out = self.sections.join("\n\n");
        out.push('\n');
        out
    }
}

fn normalize_ts_snippet(input: &str) -> String {
    let lines = input.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map(|index| index + 1)
        .unwrap_or(start);

    if start >= end {
        return String::new();
    }

    let body = &lines[start..end];
    let indent = body
        .iter()
        .filter_map(|line| {
            if line.trim().is_empty() {
                None
            } else {
                Some(
                    line.chars()
                        .take_while(|character| *character == ' ')
                        .count(),
                )
            }
        })
        .min()
        .unwrap_or(0);

    let mut out = String::new();
    for (index, line) in body.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }

        let trimmed = if line.len() >= indent {
            &line[indent..]
        } else {
            line.trim_start()
        };
        out.push_str(trimmed.trim_end());
    }
    out
}

fn spec_bindings_for_library(
    library: &'static WidgetLibraryBindings,
) -> &'static [&'static SpecWidgetBinding] {
    library.spec_bindings
}

fn build_library_registry(library: &'static WidgetLibraryBindings) -> WidgetRegistry {
    WidgetRegistry::build(&[library])
}

fn prop_decl_for_library(
    library: &'static WidgetLibraryBindings,
    spec_key: qt_solid_widget_core::decl::SpecWidgetKey,
) -> Option<&'static qt_solid_widget_core::runtime::WidgetPropDecl> {
    library
        .widget_prop_decls
        .iter()
        .copied()
        .find(|decl| decl.spec_key == spec_key)
}

fn merged_prop_setter_specs(
    registry: &WidgetRegistry,
    library: &'static WidgetLibraryBindings,
    binding: &WidgetBinding,
    spec: &'static SpecWidgetBinding,
) -> Vec<PropLeafSpec> {
    let mut leaves = BTreeMap::<String, PropLeafSpec>::new();
    let decl = prop_decl_for_library(library, spec.spec_key);

    for prop in merged_props(spec, decl) {
        let prop_id = registry.prop_id(binding.widget_type_id, &prop.key);
        let Some(leaf) = prop_setter_leaf_spec(prop, prop_id) else {
            continue;
        };
        leaves.insert(leaf.key.clone(), leaf);
    }

    if let Some(decl) = decl {
        for create_prop in decl.create_props {
            let kind = prop_js_value_kind(create_prop.value_type);
            let non_negative = type_is_non_negative(create_prop.value_type);
            let key = create_prop.key.to_owned();

            match leaves.entry(key.clone()) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().create = true;
                }
                Entry::Vacant(entry) => {
                    entry.insert(PropLeafSpec {
                        path: vec![key.clone()],
                        key: key.clone(),
                        prop_id: registry.prop_id(binding.widget_type_id, &key),
                        kind,
                        non_negative,
                        default: SpecPropDefaultValue::None,
                        method: None,
                        init_method: None,
                        create: true,
                    });
                }
            }
        }
    }

    leaves.into_values().collect()
}

fn prop_setter_leaf_spec(prop: MergedProp, prop_id: Option<u16>) -> Option<PropLeafSpec> {
    let method_name = prop_setter_method_name(&prop)?;
    let (method, init_method) = match prop.write_mode() {
        EndpointWriteMode::CreateOnly(_) => (None, Some(method_name)),
        EndpointWriteMode::None => return None,
        EndpointWriteMode::Bound | EndpointWriteMode::Live(_) => {
            (Some(method_name), prop_init_setter_method_name(&prop))
        }
    };

    Some(PropLeafSpec {
        path: prop.path.into_iter().map(str::to_owned).collect(),
        key: prop.key,
        prop_id,
        kind: prop_js_value_kind(prop.value_type),
        non_negative: type_is_non_negative(prop.value_type),
        default: prop.default,
        method,
        init_method,
        create: false,
    })
}

pub fn render_library_intrinsics_ts(library: &'static WidgetLibraryBindings) -> String {
    let mut module = TsModule::default();
    module.push(
        r#"
        // Auto-generated by packages/core-native/crates/native/src/bin/generate_ts_bindings.rs. Do not edit.
        import type { JSX as SolidJsx } from "solid-js"
        "#,
    );

    let registry = build_library_registry(library);
    let spec_bindings = spec_bindings_for_library(library);
    let bindings = registry.bindings().to_vec();
    for spec in spec_bindings {
        let binding = registry.binding_by_spec_key(spec.spec_key);
        module.push(render_widget_prop_interface(
            library,
            &registry,
            binding,
            spec,
            binding.kind_name,
            binding.children,
        ));
    }

    module.push(render_prop_elements_interface(&bindings));
    module.finish()
}

pub fn render_library_native_dts(library: &'static WidgetLibraryBindings) -> String {
    let mut module = TsModule::default();
    module.push(
        r#"
        // Auto-generated by packages/core-native/crates/native/src/bin/generate_ts_bindings.rs. Do not edit.
        import type { QtApp, QtInitialProp, QtNode } from "@qt-solid/core/native"

        declare abstract class QtWidgetEntity {
          static create(app: QtApp): unknown
          __qtAttach?(initialProps: QtInitialProp[]): void
          get node(): QtNode
          get id(): number
          get parent(): QtNode | null
          get firstChild(): QtNode | null
          get nextSibling(): QtNode | null
          isTextNode(): boolean
          insertChild(child: QtNode, anchor?: QtNode | undefined | null): void
          removeChild(child: QtNode): void
          destroy(): void
        }
        "#,
    );

    let registry = build_library_registry(library);
    for spec in spec_bindings_for_library(library) {
        module.push(render_native_entity_class_dts(
            library,
            registry.binding_by_spec_key(spec.spec_key),
            spec,
        ));
    }

    module.finish()
}

pub fn render_library_native_ts(
    library: &'static WidgetLibraryBindings,
    options: NativeTsOptions<'_>,
) -> String {
    let mut module = TsModule::default();
    module.push(
        r#"
        // Auto-generated by packages/core-native/crates/native/src/bin/generate_ts_bindings.rs. Do not edit.
        import { readdirSync } from "node:fs"
        import { createRequire } from "node:module"
        import { dirname, join } from "node:path"
        import { fileURLToPath } from "node:url"
        "#,
    );

    if matches!(options.export_style, NativeTsExportStyle::FunctionBridge) {
        module.push(
            r#"
            import type { QtApp, QtNode } from "@qt-solid/core/native"
            "#,
        );
    }

    module.push(render_native_loader_prelude(options));

    let registry = build_library_registry(library);
    let spec_bindings = spec_bindings_for_library(library);

    match options.export_style {
        NativeTsExportStyle::DirectEntities => {
            let mut widget_names = BTreeMap::<String, ()>::new();
            for binding in registry.bindings() {
                widget_names.insert(widget_entity_class_name(binding.kind_name), ());
            }

            let exports = widget_names
                .keys()
                .map(|widget_name| format!("  {widget_name},"))
                .collect::<Vec<_>>()
                .join("\n");

            module.push(format!(
                "const nativeBinding = require(resolveNativeBinaryPath())\n\nexport const {{\n{exports}\n}} = nativeBinding"
            ));
        }
        NativeTsExportStyle::FunctionBridge => {
            module.push(render_function_bridge_raw_binding_interface(
                library,
                &registry,
                spec_bindings,
            ));
            module.push(
                "const nativeBinding = require(resolveNativeBinaryPath()) as RawNativeBinding",
            );

            for spec in spec_bindings {
                let binding = registry.binding_by_spec_key(spec.spec_key);
                module.push(render_function_bridge_widget_entity_ts(
                    library, &registry, binding, spec,
                ));
            }
        }
    }

    module.finish()
}

pub fn render_library_host_ts(
    library: &'static WidgetLibraryBindings,
    library_native_specifier: &str,
) -> String {
    let mut module = TsModule::default();
    module.push(
        r#"
        // Auto-generated by packages/core-native/crates/native/src/bin/generate_ts_bindings.rs. Do not edit.
        "#,
    );
    let registry = build_library_registry(library);
    let spec_bindings = spec_bindings_for_library(library);
    let bindings = registry.bindings().to_vec();
    module.push(render_native_type_imports(
        library,
        &registry,
        spec_bindings,
        &bindings,
        library_native_specifier,
    ));
    module.push(
        r#"
        import {
          asBoolean,
          asF64,
          asI32,
          asNonNegativeF64,
          asNonNegativeI32,
          asString,
          createEnumValueParser,
          type EventExportSpec,
          type PropLeafBinding as SharedPropLeafBinding,
          type QtWidgetBinding as SharedQtWidgetBinding,
        } from "@qt-solid/core/qt-host.shared"
        "#,
    );

    let mut enum_domains = BTreeMap::new();
    let mut prop_cases = BTreeMap::<String, ()>::new();
    let mut export_payload_cases = BTreeMap::new();
    let mut export_echo_cases = BTreeMap::new();

    for spec in spec_bindings {
        let binding = registry.binding_by_spec_key(spec.spec_key);
        for prop in merged_prop_setter_specs(&registry, library, binding, spec) {
            if let PropJsValueKind::Enum(domain) = prop.kind {
                enum_domains.entry(domain.name).or_insert(domain.values);
            }
            if let Some(method) = prop.method.as_ref() {
                match prop_cases.entry(method.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(());
                    }
                    Entry::Occupied(_) => {}
                }
            }
            if let Some(init_method) = prop.init_method.as_ref() {
                match prop_cases.entry(init_method.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(());
                    }
                    Entry::Occupied(_) => {}
                }
            }
        }
    }

    for binding in &bindings {
        for event in &binding.events {
            let echoes = normalize_event_echo_specs(
                event.payload_kind,
                event.payload_type,
                &event.payload_fields,
                &event.echoes,
            );
            for export_name in event.exports {
                let payload = normalize_event_payload_spec(
                    event.payload_kind,
                    event.payload_type,
                    &event.payload_fields,
                );
                match export_payload_cases.entry(*export_name) {
                    Entry::Vacant(entry) => {
                        entry.insert(payload);
                    }
                    Entry::Occupied(entry) => {
                        assert!(
                            entry.get() == &payload,
                            "event export {export_name} diverged across widgets"
                        );
                    }
                }
                match export_echo_cases.entry(*export_name) {
                    Entry::Vacant(entry) => {
                        entry.insert(echoes.clone());
                    }
                    Entry::Occupied(entry) => {
                        assert!(
                            entry.get() == &echoes,
                            "event export {export_name} echo diverged across widgets"
                        );
                    }
                }
            }
        }
    }

    module.push(render_prop_method_names(
        prop_cases.keys().map(String::as_str),
    ));
    module.push(
        r#"
        export type PropLeafBinding = SharedPropLeafBinding<PropMethodName>
        export type QtWidgetBinding = SharedQtWidgetBinding<PropMethodName>
        "#,
    );

    for (name, values) in &enum_domains {
        module.push(render_enum_helper(name, values));
    }

    module.push(render_event_export_specs(
        &registry,
        &export_payload_cases,
        &export_echo_cases,
    ));
    module.push(render_widget_bindings(library, &registry, spec_bindings));

    module.finish()
}

fn render_native_entity_class_dts(
    library: &'static WidgetLibraryBindings,
    binding: &WidgetBinding,
    spec: &'static SpecWidgetBinding,
) -> String {
    let type_name = widget_entity_class_name(binding.kind_name);
    let merged_props = merged_props(spec, prop_decl_for_library(library, spec.spec_key));
    let mut setters = BTreeMap::<String, String>::new();
    let mut getters = BTreeMap::<String, String>::new();
    for prop in &merged_props {
        if let Some(method) = prop_setter_method_name(prop) {
            setters.insert(method, prop_ts_type(prop.value_type));
        }
        if prop.getter_slot.is_some() {
            getters.insert(prop_getter_method_name(prop), prop_ts_type(prop.value_type));
        }
    }

    let mut out = String::new();
    writeln!(
        &mut out,
        "export declare class {type_name} extends QtWidgetEntity {{"
    )
    .expect("write native entity class");
    writeln!(&mut out, "  static create(app: QtApp): {type_name}")
        .expect("write native entity create");
    for (method_name, value_type) in setters {
        writeln!(&mut out, "  {method_name}(value: {value_type}): void")
            .expect("write native entity setter");
    }
    for (method_name, value_type) in getters {
        writeln!(&mut out, "  {method_name}(): {value_type}").expect("write native entity getter");
    }
    for method in binding.methods.host_methods {
        writeln!(
            &mut out,
            "  {}({}): {}",
            method.js_name,
            render_method_ts_args(method.args),
            render_method_ts_return(method.return_type)
        )
        .expect("write native entity method");
    }
    out.push('}');
    out
}

fn render_native_loader_prelude(options: NativeTsOptions<'_>) -> String {
    format!(
        r#"
        const require = createRequire(import.meta.url)

        function resolveNativeDir(): string {{
          try {{
            const packageNativeEntry = require.resolve("{package_native_specifier}")
            return join(dirname(packageNativeEntry), "{resolve_relative_to_package}")
          }} catch {{
            return join(dirname(fileURLToPath(import.meta.url)), "{resolve_relative_to_source}")
          }}
        }}

        const nativeDir = resolveNativeDir()

        function isMusl() {{
          if (process.platform !== "linux") {{
            return false
          }}

          try {{
            return require("node:fs").readFileSync("/usr/bin/ldd", "utf8").includes("musl")
          }} catch {{
            return false
          }}
        }}

        function resolveNativeBinaryPath() {{
          const entries = new Set(readdirSync(nativeDir))
          const candidates = []

          if (process.platform === "darwin") {{
            candidates.push("index.darwin-universal.node")
            candidates.push(`index.darwin-${{process.arch}}.node`)
          }} else if (process.platform === "linux") {{
            const abi = isMusl() ? "musl" : "gnu"
            candidates.push(`index.linux-${{process.arch}}-${{abi}}.node`)
          }} else if (process.platform === "win32") {{
            candidates.push(`index.win32-${{process.arch}}-msvc.node`)
            candidates.push(`index.win32-${{process.arch}}-gnu.node`)
          }} else if (process.platform === "freebsd") {{
            candidates.push(`index.freebsd-${{process.arch}}.node`)
          }} else if (process.platform === "android") {{
            if (process.arch === "arm64") {{
              candidates.push("index.android-arm64.node")
            }} else if (process.arch === "arm") {{
              candidates.push("index.android-arm-eabi.node")
            }}
          }}

          const match = candidates.find((candidate) => entries.has(candidate))
          if (!match) {{
            throw new Error(
              `No native binary for ${{process.platform}}/${{process.arch}} under ${{nativeDir}}`,
            )
          }}

          return join(nativeDir, match)
        }}
        "#,
        package_native_specifier = options.package_native_specifier,
        resolve_relative_to_package = options.resolve_relative_to_package,
        resolve_relative_to_source = options.resolve_relative_to_source,
    )
}

fn render_function_bridge_raw_binding_interface(
    library: &'static WidgetLibraryBindings,
    registry: &WidgetRegistry,
    spec_bindings: &'static [&'static SpecWidgetBinding],
) -> String {
    let mut out = String::new();
    out.push_str("interface RawNativeBinding {\n");

    for spec in spec_bindings {
        let binding = registry.binding_by_spec_key(spec.spec_key);
        writeln!(
            &mut out,
            "  {}(app: QtApp): QtNode",
            raw_create_function_name(binding.kind_name)
        )
        .expect("write raw native create signature");

        for prop in merged_props(spec, prop_decl_for_library(library, spec.spec_key)) {
            let Some(_) = prop_setter_method_name(&prop) else {
                continue;
            };

            writeln!(
                &mut out,
                "  {}(node: QtNode, value: {}): void",
                raw_apply_function_name(binding.kind_name, &prop),
                prop_ts_type(prop.value_type),
            )
            .expect("write raw native apply signature");
        }
    }

    out.push('}');
    out
}

fn render_function_bridge_widget_entity_ts(
    library: &'static WidgetLibraryBindings,
    _registry: &WidgetRegistry,
    binding: &WidgetBinding,
    spec: &'static SpecWidgetBinding,
) -> String {
    let type_name = widget_entity_class_name(binding.kind_name);
    let create_fn = raw_create_function_name(binding.kind_name);
    let mut setters = Vec::new();

    for prop in merged_props(spec, prop_decl_for_library(library, spec.spec_key)) {
        let Some(method_name) = prop_setter_method_name(&prop) else {
            continue;
        };
        let value_type = prop_ts_type(prop.value_type);
        let apply_fn = raw_apply_function_name(binding.kind_name, &prop);
        setters.push(format!(
            "  {method_name}(value: {value_type}): void {{\n    nativeBinding.{apply_fn}(this.#node, value)\n  }}"
        ));
    }

    let setters = setters.join("\n\n");

    format!(
        r#"
        export class {type_name} {{
          readonly #node: QtNode

          private constructor(node: QtNode) {{
            this.#node = node
          }}

          static create(app: QtApp): {type_name} {{
            return new {type_name}(nativeBinding.{create_fn}(app))
          }}

          get node(): QtNode {{
            return this.#node
          }}

          get id(): number {{
            return this.#node.id
          }}

          get parent(): QtNode | null {{
            return this.#node.parent
          }}

          get firstChild(): QtNode | null {{
            return this.#node.firstChild
          }}

          get nextSibling(): QtNode | null {{
            return this.#node.nextSibling
          }}

          isTextNode(): boolean {{
            return this.#node.isTextNode()
          }}

          insertChild(child: QtNode, anchor?: QtNode | undefined | null): void {{
            this.#node.insertChild(child, anchor ?? null)
          }}

          removeChild(child: QtNode): void {{
            this.#node.removeChild(child)
          }}

          destroy(): void {{
            this.#node.destroy()
          }}

        {setters}
        }}
        "#,
        type_name = type_name,
        create_fn = create_fn,
        setters = setters,
    )
}

fn raw_create_function_name(kind_name: &str) -> String {
    format!("create{}Node", pascal_case(kind_name))
}

fn raw_apply_function_name(kind_name: &str, prop: &MergedProp) -> String {
    format!(
        "apply{}{}",
        pascal_case(kind_name),
        path_method_suffix(prop.path.iter().copied())
    )
}

fn render_method_ts_args(args: &[SpecHostMethodArg]) -> String {
    args.iter()
        .map(|arg| format!("{}: {}", arg.js_name, method_arg_ts_type(arg.value_type)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_method_ts_return(value_type: QtTypeInfo) -> String {
    if matches!(value_type.repr(), QtValueRepr::Unit) {
        return "void".to_owned();
    }

    scalar_ts_type(value_type)
}

fn method_arg_ts_type(value_type: QtTypeInfo) -> String {
    scalar_ts_type(value_type)
}

fn enum_meta_ts_type(domain: &'static EnumMeta) -> String {
    domain
        .values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn scalar_ts_type(value_type: QtTypeInfo) -> String {
    match value_type.repr() {
        QtValueRepr::Unit => "void".to_owned(),
        QtValueRepr::String => ts_type_for_scalar_kind(TsScalarKind::String),
        QtValueRepr::Bool => ts_type_for_scalar_kind(TsScalarKind::Boolean),
        QtValueRepr::I32 { .. } => ts_type_for_scalar_kind(TsScalarKind::Integer),
        QtValueRepr::F64 { .. } => ts_type_for_scalar_kind(TsScalarKind::Number),
        QtValueRepr::Enum(domain) => ts_type_for_scalar_kind(TsScalarKind::Enum(domain)),
        _ => value_type.ts_type().to_owned(),
    }
}

fn type_is_non_negative(value_type: QtTypeInfo) -> bool {
    value_type.is_non_negative()
}

fn event_payload_scalar_kind(value_type: QtTypeInfo) -> EventJsValueKind {
    match value_type.repr() {
        QtValueRepr::String => EventJsValueKind::String,
        QtValueRepr::Bool => EventJsValueKind::Boolean,
        QtValueRepr::I32 { .. } | QtValueRepr::F64 { .. } | QtValueRepr::Enum(_) => {
            EventJsValueKind::Number
        }
        other => panic!("unsupported event export scalar type {:?}", other),
    }
}

fn render_widget_prop_interface(
    library: &'static WidgetLibraryBindings,
    registry: &WidgetRegistry,
    binding: &WidgetBinding,
    spec: &'static SpecWidgetBinding,
    kind_name: &str,
    children: ChildrenKind,
) -> String {
    let mut out = String::new();
    writeln!(
        &mut out,
        "export interface {} {{",
        intrinsic_interface_name(kind_name)
    )
    .expect("write widget prop interface");
    let setter_leaves = merged_prop_setter_specs(&registry, library, binding, spec);
    render_prop_interface_members(&mut out, &setter_leaves, 1);

    if children != ChildrenKind::None {
        out.push_str("  children?: SolidJsx.Element\n");
    }

    for event in &binding.events {
        let callback = event_export_callback_type(
            event.payload_kind,
            event.payload_type,
            &event.payload_fields,
        );
        for export_name in event.exports {
            writeln!(&mut out, "  {export_name}?: {callback}").expect("write widget event export");
        }
    }

    out.push('}');
    out
}

fn render_prop_elements_interface(bindings: &[&WidgetBinding]) -> String {
    let mut out = String::new();
    out.push_str("export interface QtIntrinsicElements {\n");
    for binding in bindings {
        writeln!(
            &mut out,
            "  {}: {}",
            binding.kind_name,
            intrinsic_interface_name(binding.kind_name)
        )
        .expect("write prop elements entry");
    }
    out.push('}');
    out
}

fn widget_entity_class_name(kind_name: &str) -> String {
    format!("Qt{}", pascal_case(kind_name))
}

fn path_method_suffix<'a>(path: impl IntoIterator<Item = &'a str>) -> String {
    path.into_iter().map(pascal_case).collect::<String>()
}

fn prop_setter_method_name(prop: &MergedProp) -> Option<String> {
    if matches!(prop.write_mode(), EndpointWriteMode::None) {
        return None;
    }

    Some(format!(
        "set{}",
        path_method_suffix(prop.path.iter().copied())
    ))
}

fn prop_init_setter_method_name(prop: &MergedProp) -> Option<String> {
    prop.init_only_slot()?;

    if !prop.has_live_write() {
        return prop_setter_method_name(prop);
    }

    Some(format!(
        "__qtInit{}",
        path_method_suffix(prop.path.iter().copied())
    ))
}

fn prop_getter_method_name(prop: &MergedProp) -> String {
    format!("get{}", path_method_suffix(prop.path.iter().copied()))
}

fn render_widget_bindings(
    library: &'static WidgetLibraryBindings,
    registry: &WidgetRegistry,
    spec_bindings: &[&'static SpecWidgetBinding],
) -> String {
    let mut module = TsModule::default();
    let mut entity_map_entries = Vec::new();
    let mut widget_bindings = String::new();
    widget_bindings.push_str("export const widgetBindings = {\n");

    for spec in spec_bindings {
        let binding = registry.binding_by_spec_key(spec.spec_key);
        let entity_name = widget_entity_class_name(binding.kind_name);
        let setter_leaves = merged_prop_setter_specs(&registry, library, binding, spec);

        writeln!(&mut widget_bindings, "  \"{}\": {{", binding.kind_name)
            .expect("write widget binding key");
        widget_bindings.push_str("    props: {\n");

        for leaf in setter_leaves {
            write!(&mut widget_bindings, "      {}: ", leaf.key).expect("write widget prop key");
            render_prop_leaf_binding(&mut widget_bindings, &leaf);
            widget_bindings.push_str(",\n");
        }
        widget_bindings.push_str("    },\n");
        widget_bindings.push_str("  },\n");
        entity_map_entries.push(format!("  \"{}\": {entity_name},", binding.kind_name));
    }

    widget_bindings.push_str("} satisfies Record<string, QtWidgetBinding>");
    module.push(widget_bindings);

    let mut entity_map = String::new();
    entity_map.push_str("export const qtWidgetEntityMap = {\n");
    for entry in entity_map_entries {
        entity_map.push_str(&entry);
        entity_map.push('\n');
    }
    entity_map.push_str("} satisfies Record<string, QtWidgetEntityCtor>");
    module.push(entity_map);

    module.finish()
}

fn render_enum_helper(name: &str, values: &[&str]) -> String {
    let constant = enum_values_constant(name);
    let values_literal = values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "const {constant} = [{values_literal}] as const\n\nconst as{name}Value = createEnumValueParser<{name}>({constant})"
    )
}

fn render_prop_method_names<'a>(props: impl Iterator<Item = &'a str>) -> String {
    let mut op_names = props.map(str::to_owned).collect::<Vec<_>>();
    op_names.sort_unstable();
    op_names.dedup();

    let union = if op_names.is_empty() {
        "never".to_owned()
    } else {
        op_names
            .into_iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(" | ")
    };

    format!("export type PropMethodName = {union}")
}

fn render_event_export_specs(
    registry: &WidgetRegistry,
    export_payload_cases: &BTreeMap<&'static str, EventPayloadJsSpec>,
    export_echo_cases: &BTreeMap<&'static str, Vec<EventEchoJsSpec>>,
) -> String {
    let mut out = String::new();
    out.push_str("export const eventExportSpecs = {\n");
    for (export_name, payload) in export_payload_cases {
        let export_id_value = registry
            .export_id(export_name)
            .expect("schema event export id");
        let echoes = export_echo_cases
            .get(export_name)
            .cloned()
            .unwrap_or_default();
        writeln!(&mut out, "  {export_name}: {{").expect("write event export spec key");
        writeln!(&mut out, "    exportId: {export_id_value},").expect("write event export spec id");
        writeln!(
            &mut out,
            "    payloadKind: \"{}\",",
            event_payload_kind_tag(payload)
        )
        .expect("write event export payload kind");
        out.push_str("    payloadFields: [\n");
        for field in event_payload_fields(payload) {
            writeln!(
                &mut out,
                "      {{ key: \"{}\", valueKind: \"{}\", valuePath: \"{}\" }},",
                field.js_name,
                event_decode_kind_tag(field.kind),
                field.js_name
            )
            .expect("write event export payload field");
        }
        out.push_str("    ],\n");
        out.push_str("    echoes: [\n");
        for echo in &echoes {
            writeln!(
                &mut out,
                "      {{ propKey: \"{}\", valueKind: \"{}\", valuePath: \"{}\" }},",
                echo.prop_js_name,
                event_decode_kind_tag(echo.kind),
                echo.value_path
            )
            .expect("write event export echo spec");
        }
        out.push_str("    ],\n");
        out.push_str("  },\n");
    }
    out.push_str("} as const satisfies Record<string, EventExportSpec>");
    out
}

fn prop_js_value_kind(value_type: QtTypeInfo) -> PropJsValueKind {
    match value_type.repr() {
        QtValueRepr::String => PropJsValueKind::String,
        QtValueRepr::Bool => PropJsValueKind::Boolean,
        QtValueRepr::I32 { .. } => PropJsValueKind::Integer,
        QtValueRepr::F64 { .. } => PropJsValueKind::Number,
        QtValueRepr::Enum(domain) => PropJsValueKind::Enum(domain),
        other => panic!("unsupported prop JS value type {:?}", other),
    }
}

fn render_prop_interface_members(out: &mut String, leaves: &[PropLeafSpec], indent: usize) {
    let mut root_leaves = BTreeMap::<&str, &PropLeafSpec>::new();
    let mut groups = BTreeMap::<&str, Vec<PropLeafSpec>>::new();

    for leaf in leaves {
        let (head, tail) = leaf
            .path
            .split_first()
            .expect("prop path should not be empty");
        if tail.is_empty() {
            root_leaves.insert(head.as_str(), leaf);
        } else {
            let mut nested = leaf.clone();
            nested.path = tail.to_vec();
            groups.entry(head.as_str()).or_default().push(nested);
        }
    }

    for (name, leaf) in root_leaves {
        let indent_prefix = "  ".repeat(indent);
        writeln!(
            out,
            "{indent_prefix}{name}?: {}",
            prop_ts_type_from_kind(leaf.kind)
        )
        .expect("write prop leaf member");
    }

    for (name, nested) in groups {
        let indent_prefix = "  ".repeat(indent);
        writeln!(out, "{indent_prefix}{name}?: {{").expect("write prop group start");
        render_prop_interface_members(out, &nested, indent + 1);
        writeln!(out, "{indent_prefix}}}").expect("write prop group end");
    }
}

fn render_prop_leaf_binding(out: &mut String, leaf: &PropLeafSpec) {
    out.push_str("{ ");
    out.push_str(&format!("key: \"{}\"", leaf.key));
    if let Some(prop_id) = leaf.prop_id {
        out.push_str(&format!(", propId: {prop_id}"));
    }
    if leaf.create {
        out.push_str(", create: true");
    }
    out.push_str(&format!(
        ", valueKind: \"{}\"",
        prop_js_value_kind_tag(leaf.kind)
    ));
    if let Some(reset) = prop_default_literal(leaf.default) {
        out.push_str(&format!(", reset: {reset}"));
    }
    if let Some(method) = leaf.method.as_ref() {
        out.push_str(&format!(", method: \"{}\"", method));
    }
    if let Some(init_method) = leaf.init_method.as_ref() {
        out.push_str(&format!(", initMethod: \"{}\"", init_method));
    }
    out.push_str(&format!(
        ", parseValue(value, key) {{ return {}; }}",
        prop_leaf_coerce_expression(leaf, "value", "key")
    ));
    out.push_str(" }");
}

fn intrinsic_interface_name(kind_name: &str) -> String {
    format!("{}IntrinsicProps", pascal_case(kind_name))
}

fn prop_ts_type(value_type: QtTypeInfo) -> String {
    scalar_ts_type(value_type)
}

fn prop_js_value_kind_tag(kind: PropJsValueKind) -> &'static str {
    match kind {
        PropJsValueKind::String => "string",
        PropJsValueKind::Boolean => "boolean",
        PropJsValueKind::Integer => "integer",
        PropJsValueKind::Number => "number",
        PropJsValueKind::Enum(_) => "enum",
    }
}

fn prop_ts_type_from_kind(kind: PropJsValueKind) -> String {
    match kind {
        PropJsValueKind::String => "string".to_owned(),
        PropJsValueKind::Boolean => "boolean".to_owned(),
        PropJsValueKind::Integer | PropJsValueKind::Number => "number".to_owned(),
        PropJsValueKind::Enum(domain) => enum_meta_ts_type(domain),
    }
}

fn prop_default_literal(default: SpecPropDefaultValue) -> Option<String> {
    match default {
        SpecPropDefaultValue::None => None,
        SpecPropDefaultValue::Bool(value) => Some(value.to_string()),
        SpecPropDefaultValue::I32(value) => Some(value.to_string()),
        SpecPropDefaultValue::F64(value) => Some(format!("{value:?}")),
        SpecPropDefaultValue::String(value) | SpecPropDefaultValue::Enum(value) => {
            Some(format!("{value:?}"))
        }
    }
}

fn event_export_callback_type(
    payload_kind: EventPayloadKind,
    payload_type: Option<QtTypeInfo>,
    fields: &[EventFieldMeta],
) -> String {
    match payload_kind {
        EventPayloadKind::Unit => "() => void".to_owned(),
        EventPayloadKind::Scalar => format!(
            "(value: {}) => void",
            scalar_ts_type(payload_type.expect("scalar event payload type"))
        ),
        EventPayloadKind::Object => {
            let members = fields
                .iter()
                .map(|field| format!("{}: {}", field.js_name, scalar_ts_type(field.value_type)))
                .collect::<Vec<_>>()
                .join("; ");
            format!("(payload: {{ {members} }}) => void")
        }
    }
}

fn ts_type_for_scalar_kind(kind: TsScalarKind) -> String {
    match kind {
        TsScalarKind::String => "string".to_owned(),
        TsScalarKind::Boolean => "boolean".to_owned(),
        TsScalarKind::Integer => "number".to_owned(),
        TsScalarKind::Number => "number".to_owned(),
        TsScalarKind::Enum(domain) => enum_meta_ts_type(domain),
    }
}

fn normalize_event_payload_spec(
    payload_kind: EventPayloadKind,
    payload_type: Option<QtTypeInfo>,
    fields: &[EventFieldMeta],
) -> EventPayloadJsSpec {
    match payload_kind {
        EventPayloadKind::Unit => EventPayloadJsSpec::Unit,
        EventPayloadKind::Scalar => EventPayloadJsSpec::Scalar(event_payload_scalar_kind(
            payload_type.expect("scalar event payload type"),
        )),
        EventPayloadKind::Object => EventPayloadJsSpec::Object(
            fields
                .iter()
                .map(|field| EventPayloadFieldJsSpec {
                    js_name: field.js_name,
                    kind: event_payload_scalar_kind(field.value_type),
                })
                .collect(),
        ),
    }
}

fn event_decode_kind_tag(kind: EventJsValueKind) -> &'static str {
    match kind {
        EventJsValueKind::String => "string",
        EventJsValueKind::Boolean => "boolean",
        EventJsValueKind::Number => "number",
    }
}

fn event_payload_kind_tag(payload: &EventPayloadJsSpec) -> &'static str {
    match payload {
        EventPayloadJsSpec::Unit => "unit",
        EventPayloadJsSpec::Scalar(EventJsValueKind::String) => "string",
        EventPayloadJsSpec::Scalar(EventJsValueKind::Boolean) => "boolean",
        EventPayloadJsSpec::Scalar(EventJsValueKind::Number) => "number",
        EventPayloadJsSpec::Object(_) => "object",
    }
}

fn event_payload_fields(payload: &EventPayloadJsSpec) -> &[EventPayloadFieldJsSpec] {
    match payload {
        EventPayloadJsSpec::Object(fields) => fields,
        _ => &[],
    }
}

fn normalize_event_echo_specs(
    payload_kind: EventPayloadKind,
    payload_type: Option<QtTypeInfo>,
    fields: &[EventFieldMeta],
    echoes: &[EventEchoMeta],
) -> Vec<EventEchoJsSpec> {
    echoes
        .iter()
        .map(|echo| EventEchoJsSpec {
            prop_js_name: echo.prop_js_name,
            value_path: echo.value_path,
            kind: event_value_kind_for_path(payload_kind, payload_type, fields, echo.value_path),
        })
        .collect()
}

fn event_value_kind_for_path(
    payload_kind: EventPayloadKind,
    payload_type: Option<QtTypeInfo>,
    fields: &[EventFieldMeta],
    path: &str,
) -> EventJsValueKind {
    match payload_kind {
        EventPayloadKind::Unit => panic!("unit payload cannot drive controlled echo"),
        EventPayloadKind::Scalar => {
            assert!(path.is_empty(), "scalar payload must use empty path");
            event_payload_scalar_kind(payload_type.expect("scalar event payload type"))
        }
        EventPayloadKind::Object => fields
            .iter()
            .find(|field| field.js_name == path)
            .map(|field| event_payload_scalar_kind(field.value_type))
            .unwrap_or_else(|| {
                panic!("event payload field {path} is missing from controlled echo")
            }),
    }
}

fn render_native_type_imports(
    library: &'static WidgetLibraryBindings,
    registry: &WidgetRegistry,
    spec_bindings: &[&'static SpecWidgetBinding],
    bindings: &[&WidgetBinding],
    library_native_specifier: &str,
) -> String {
    let mut raw_names = BTreeMap::<&'static str, ()>::new();
    let mut widget_names = BTreeMap::<String, ()>::new();
    raw_names.insert("QtApp", ());
    raw_names.insert("QtNode", ());

    for spec in spec_bindings {
        let binding = registry.binding_by_spec_key(spec.spec_key);
        for prop in merged_prop_setter_specs(&registry, library, binding, spec) {
            if let PropJsValueKind::Enum(domain) = prop.kind {
                raw_names.insert(domain.name, ());
            }
        }
    }

    for binding in bindings {
        widget_names.insert(widget_entity_class_name(binding.kind_name), ());
    }

    let mut out = String::from("import type {\n");
    for name in raw_names.keys() {
        out.push_str(&format!("  {name},\n"));
    }
    out.push_str("} from \"@qt-solid/core/native\"\n\n");
    out.push_str(
        "import type {\n  QtWidgetEntityCtor,\n} from \"@qt-solid/core/widget-library\"\n\n",
    );
    out.push_str("import {\n");
    for name in widget_names.keys() {
        out.push_str(&format!("  {name},\n"));
    }
    out.push_str(&format!("}} from \"{library_native_specifier}\"\n\n"));
    out
}

fn enum_values_constant(name: &str) -> String {
    format!("{}_VALUES", screaming_snake_case(name))
}

fn pascal_case(value: &str) -> String {
    let mut result = String::new();
    let mut upper_next = true;

    for character in value.chars() {
        if character == '_' || character == '-' {
            upper_next = true;
            continue;
        }

        if upper_next {
            result.extend(character.to_uppercase());
            upper_next = false;
        } else {
            result.push(character);
        }
    }

    result
}

fn screaming_snake_case(value: &str) -> String {
    let mut result = String::new();

    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            result.push('_');
        }
        result.extend(character.to_uppercase());
    }

    result
}

fn prop_leaf_coerce_expression(leaf: &PropLeafSpec, value_expr: &str, key_expr: &str) -> String {
    match leaf.kind {
        PropJsValueKind::String => format!("asString({value_expr}, {key_expr})"),
        PropJsValueKind::Boolean => format!("asBoolean({value_expr}, {key_expr})"),
        PropJsValueKind::Integer => {
            if leaf.non_negative {
                format!("asNonNegativeI32({value_expr}, {key_expr})")
            } else {
                format!("asI32({value_expr}, {key_expr})")
            }
        }
        PropJsValueKind::Number => {
            if leaf.non_negative {
                format!("asNonNegativeF64({value_expr}, {key_expr})")
            } else {
                format!("asF64({value_expr}, {key_expr})")
            }
        }
        PropJsValueKind::Enum(domain) => {
            format!("as{}Value({value_expr}, {key_expr})", domain.name)
        }
    }
}
