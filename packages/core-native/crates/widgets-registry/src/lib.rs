use std::{collections::BTreeMap, sync::LazyLock};

use qt_solid_widget_core::{
    codegen::{
        OpaqueCodegenDecl, QT_WIDGET_CODEGEN_FRAGMENTS, WidgetCodegenDecl, WidgetCodegenFragment,
        WidgetHostEventMountCodegenMeta, WidgetHostEventMountCodegenSet,
        WidgetHostOverrideCodegenMeta, WidgetHostOverrideCodegenSet,
        WidgetHostPropGetterCodegenMeta, WidgetHostPropGetterCodegenSet,
        WidgetHostPropSetterCodegenMeta, WidgetHostPropSetterCodegenSet,
    },
    decl::SpecWidgetKey,
    schema::{SpecOpaqueDecl, WidgetLibraryBindings, WidgetRegistry},
};

pub mod assembly {
    use super::{
        BTreeMap, LazyLock, OpaqueCodegenDecl, QT_WIDGET_CODEGEN_FRAGMENTS, SpecOpaqueDecl,
        SpecWidgetKey, WidgetCodegenBuilder, WidgetCodegenDecl, WidgetLibraryBindings,
    };

    static ALL: LazyLock<Vec<&'static WidgetLibraryBindings>> = LazyLock::new(|| {
        vec![
            qt_solid_core_widgets::core_widgets_library(),
            qt_solid_example_widgets_schema::widgets::example_widgets_runtime_library(),
        ]
    });

    static ALL_WIDGET_CODEGEN_DECLS: LazyLock<Vec<WidgetCodegenDecl>> = LazyLock::new(|| {
        let mut builders = BTreeMap::<SpecWidgetKey, WidgetCodegenBuilder>::new();

        for fragment in QT_WIDGET_CODEGEN_FRAGMENTS.iter().copied() {
            builders
                .entry(fragment.spec_key)
                .or_default()
                .extend(fragment);
        }

        builders
            .into_iter()
            .map(|(spec_key, builder)| builder.finish(spec_key))
            .collect()
    });

    static ALL_WIDGET_CODEGEN_DECL_REFS: LazyLock<Vec<&'static WidgetCodegenDecl>> =
        LazyLock::new(|| ALL_WIDGET_CODEGEN_DECLS.iter().collect());

    pub fn all_widget_libraries() -> &'static [&'static WidgetLibraryBindings] {
        ALL.as_slice()
    }

    pub fn all_opaque_decls() -> &'static [&'static SpecOpaqueDecl] {
        static ALL_OPAQUES: LazyLock<Vec<&'static SpecOpaqueDecl>> = LazyLock::new(|| {
            all_widget_libraries()
                .iter()
                .flat_map(|library| library.opaque_decls.iter().copied())
                .collect()
        });

        ALL_OPAQUES.as_slice()
    }

    pub fn all_opaque_codegen_decls() -> &'static [&'static OpaqueCodegenDecl] {
        static ALL_OPAQUES: LazyLock<Vec<&'static OpaqueCodegenDecl>> = LazyLock::new(|| {
            all_widget_libraries()
                .iter()
                .flat_map(|library| library.opaque_codegen_decls.iter().copied())
                .collect()
        });

        ALL_OPAQUES.as_slice()
    }

    pub fn all_widget_codegen_decls() -> &'static [&'static WidgetCodegenDecl] {
        ALL_WIDGET_CODEGEN_DECL_REFS.as_slice()
    }
}

#[derive(Default)]
struct WidgetCodegenBuilder {
    host_overrides: Vec<WidgetHostOverrideCodegenMeta>,
    host_event_mounts: Vec<WidgetHostEventMountCodegenMeta>,
    host_prop_setters: Vec<WidgetHostPropSetterCodegenMeta>,
    host_prop_getters: Vec<WidgetHostPropGetterCodegenMeta>,
}

impl WidgetCodegenBuilder {
    fn extend(&mut self, fragment: &'static WidgetCodegenFragment) {
        let decl = (fragment.decl)();

        for meta in decl.host_overrides.overrides {
            self.push_override(fragment.spec_key, *meta);
        }

        for meta in decl.host_event_mounts.mounts {
            self.push_event_mount(fragment.spec_key, *meta);
        }

        for meta in decl.host_prop_setters.setters {
            self.push_prop_setter(fragment.spec_key, *meta);
        }

        for meta in decl.host_prop_getters.getters {
            self.push_prop_getter(fragment.spec_key, *meta);
        }
    }

    fn push_override(&mut self, spec_key: SpecWidgetKey, meta: WidgetHostOverrideCodegenMeta) {
        if let Some(existing) = self
            .host_overrides
            .iter()
            .find(|existing| existing.target_name == meta.target_name)
        {
            panic!(
                "duplicate #[qt(host)] override export {} for widget {}: {} and {}",
                meta.target_name,
                spec_key.raw(),
                existing.rust_name,
                meta.rust_name
            );
        }

        self.host_overrides.push(meta);
    }

    fn push_event_mount(&mut self, spec_key: SpecWidgetKey, meta: WidgetHostEventMountCodegenMeta) {
        if let Some(existing) = self
            .host_event_mounts
            .iter()
            .find(|existing| existing.event_lower_name == meta.event_lower_name)
        {
            panic!(
                "duplicate #[qt(host)] event export {} for widget {}: {} and {}",
                meta.event_lower_name,
                spec_key.raw(),
                existing.rust_name,
                meta.rust_name
            );
        }

        self.host_event_mounts.push(meta);
    }

    fn push_prop_setter(&mut self, spec_key: SpecWidgetKey, meta: WidgetHostPropSetterCodegenMeta) {
        if let Some(existing) = self
            .host_prop_setters
            .iter()
            .find(|existing| existing.prop_lower_name == meta.prop_lower_name)
        {
            panic!(
                "duplicate #[qt(host)] prop setter export {} for widget {}: {} and {}",
                meta.prop_lower_name,
                spec_key.raw(),
                existing.rust_name,
                meta.rust_name
            );
        }

        self.host_prop_setters.push(meta);
    }

    fn push_prop_getter(&mut self, spec_key: SpecWidgetKey, meta: WidgetHostPropGetterCodegenMeta) {
        if let Some(existing) = self
            .host_prop_getters
            .iter()
            .find(|existing| existing.prop_lower_name == meta.prop_lower_name)
        {
            panic!(
                "duplicate #[qt(host)] prop getter export {} for widget {}: {} and {}",
                meta.prop_lower_name,
                spec_key.raw(),
                existing.rust_name,
                meta.rust_name
            );
        }

        self.host_prop_getters.push(meta);
    }

    fn finish(self, spec_key: SpecWidgetKey) -> WidgetCodegenDecl {
        let host_overrides = Box::leak(Box::new(WidgetHostOverrideCodegenSet {
            overrides: Box::leak(self.host_overrides.into_boxed_slice()),
        }));
        let host_event_mounts = Box::leak(Box::new(WidgetHostEventMountCodegenSet {
            mounts: Box::leak(self.host_event_mounts.into_boxed_slice()),
        }));
        let host_prop_setters = Box::leak(Box::new(WidgetHostPropSetterCodegenSet {
            setters: Box::leak(self.host_prop_setters.into_boxed_slice()),
        }));
        let host_prop_getters = Box::leak(Box::new(WidgetHostPropGetterCodegenSet {
            getters: Box::leak(self.host_prop_getters.into_boxed_slice()),
        }));

        WidgetCodegenDecl {
            spec_key,
            host_overrides,
            host_event_mounts,
            host_prop_setters,
            host_prop_getters,
        }
    }
}

static REGISTRY: LazyLock<WidgetRegistry> =
    LazyLock::new(|| WidgetRegistry::build(assembly::all_widget_libraries()));

pub fn widget_registry() -> &'static WidgetRegistry {
    &REGISTRY
}

pub fn all_widget_libraries() -> &'static [&'static WidgetLibraryBindings] {
    assembly::all_widget_libraries()
}

pub fn all_opaque_decls() -> &'static [&'static SpecOpaqueDecl] {
    assembly::all_opaque_decls()
}

pub fn all_opaque_codegen_decls() -> &'static [&'static OpaqueCodegenDecl] {
    assembly::all_opaque_codegen_decls()
}

pub fn all_widget_codegen_decls() -> &'static [&'static WidgetCodegenDecl] {
    assembly::all_widget_codegen_decls()
}

#[cfg(test)]
mod tests {
    use super::all_widget_codegen_decls;

    fn find_widget_codegen(
        type_name: &str,
    ) -> &'static qt_solid_widget_core::codegen::WidgetCodegenDecl {
        all_widget_codegen_decls()
            .iter()
            .copied()
            .find(|decl| decl.spec_key.raw().ends_with(type_name))
            .unwrap_or_else(|| panic!("missing widget codegen decl for {type_name}"))
    }

    #[test]
    fn button_widget_does_not_export_box_layout_props() {
        let decl = find_widget_codegen("ButtonWidget");
        let setters = decl
            .host_prop_setters
            .setters
            .iter()
            .map(|meta| meta.prop_lower_name)
            .collect::<Vec<_>>();

        assert!(setters.contains(&"grow"));
        assert!(setters.contains(&"shrink"));
        assert!(!setters.contains(&"direction"));
        assert!(!setters.contains(&"justifyContent"));
        assert!(!setters.contains(&"alignItems"));
        assert!(!setters.contains(&"gap"));
        assert!(!setters.contains(&"padding"));
    }
}
