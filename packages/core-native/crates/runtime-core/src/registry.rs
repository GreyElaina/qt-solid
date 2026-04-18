use qt_solid_widget_core::{
    decl::{NodeClass, SpecWidgetKey, WidgetTypeId},
    runtime::{WidgetNativeDecl, WidgetPropDecl},
    schema::{
        EventPayloadKind, PropMeta, SpecWidgetBinding, WidgetBinding, WidgetLibraryBindings,
        WidgetRegistry,
    },
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct RuntimeWidgetRegistry {
    inner: WidgetRegistry,
    native_decls_by_spec_key: BTreeMap<SpecWidgetKey, &'static WidgetNativeDecl>,
    prop_decls_by_spec_key: BTreeMap<SpecWidgetKey, &'static WidgetPropDecl>,
}

impl RuntimeWidgetRegistry {
    pub fn build(libraries: &[&'static WidgetLibraryBindings]) -> Self {
        let mut native_decls_by_spec_key = BTreeMap::new();
        let mut prop_decls_by_spec_key = BTreeMap::new();

        for library in libraries {
            for decl in library.widget_native_decls {
                if native_decls_by_spec_key
                    .insert(decl.spec_key, *decl)
                    .is_some()
                {
                    panic!(
                        "duplicate native widget declaration for spec widget key {}",
                        decl.spec_key.raw()
                    );
                }
            }

            for decl in library.widget_prop_decls {
                if prop_decls_by_spec_key
                    .insert(decl.spec_key, *decl)
                    .is_some()
                {
                    panic!(
                        "duplicate prop widget declaration for spec widget key {}",
                        decl.spec_key.raw()
                    );
                }
            }

            for spec in library.spec_bindings {
                let has_native = native_decls_by_spec_key.contains_key(&spec.spec_key);
                let has_prop_ctor = prop_decls_by_spec_key
                    .get(&spec.spec_key)
                    .and_then(|decl| decl.create_instance)
                    .is_some();
                if !(has_native || has_prop_ctor) {
                    panic!(
                        "missing widget constructor declaration for spec widget key {}",
                        spec.spec_key.raw()
                    );
                }
            }
        }

        Self {
            inner: WidgetRegistry::build(libraries),
            native_decls_by_spec_key,
            prop_decls_by_spec_key,
        }
    }

    pub fn bindings(&self) -> &[&'static WidgetBinding] {
        self.inner.bindings()
    }

    pub fn binding(&self, widget_type_id: WidgetTypeId) -> &'static WidgetBinding {
        self.inner.binding(widget_type_id)
    }

    pub fn binding_for_node_class(&self, class: NodeClass) -> &'static WidgetBinding {
        self.inner.binding_for_node_class(class)
    }

    pub fn kind_name_for_node_class(&self, class: NodeClass) -> &'static str {
        self.inner.kind_name_for_node_class(class)
    }

    pub fn host_tag(&self, widget_type_id: WidgetTypeId) -> u8 {
        self.inner.host_tag(widget_type_id)
    }

    pub fn binding_by_spec_key_str(&self, spec_key: &str) -> Option<&'static WidgetBinding> {
        self.inner
            .spec_bindings()
            .iter()
            .find(|spec| spec.spec_key.raw() == spec_key)
            .map(|spec| self.inner.binding_by_spec_key(spec.spec_key))
    }

    pub fn native_decl(&self, spec_key: SpecWidgetKey) -> &'static WidgetNativeDecl {
        self.native_decl_opt(spec_key).unwrap_or_else(|| {
            panic!(
                "missing native widget declaration for spec widget key {}",
                spec_key.raw()
            )
        })
    }

    pub fn native_decl_opt(&self, spec_key: SpecWidgetKey) -> Option<&'static WidgetNativeDecl> {
        self.native_decls_by_spec_key.get(&spec_key).copied()
    }

    pub fn native_decl_for_widget_type_id(
        &self,
        widget_type_id: WidgetTypeId,
    ) -> &'static WidgetNativeDecl {
        self.native_decl(self.inner.binding(widget_type_id).spec_key)
    }

    pub fn prop_decl(&self, spec_key: SpecWidgetKey) -> Option<&'static WidgetPropDecl> {
        self.prop_decls_by_spec_key.get(&spec_key).copied()
    }

    pub fn prop_decl_for_widget_type_id(
        &self,
        widget_type_id: WidgetTypeId,
    ) -> Option<&'static WidgetPropDecl> {
        self.prop_decl(self.inner.binding(widget_type_id).spec_key)
    }

    pub fn widget_type_id_from_host_tag(&self, tag: u8) -> Option<WidgetTypeId> {
        self.inner.widget_type_id_from_host_tag(tag)
    }

    pub fn prop_id(&self, widget_type_id: WidgetTypeId, js_name: &str) -> Option<u16> {
        self.inner.prop_id(widget_type_id, js_name)
    }

    pub fn prop_id_for_class(&self, class: NodeClass, js_name: &str) -> Option<u16> {
        self.inner.prop_id_for_class(class, js_name)
    }

    pub fn prop_id_by_symbol(&self, symbol: &str) -> Option<u16> {
        self.inner.prop_id_by_symbol(symbol)
    }

    pub fn prop_meta_for_id(
        &self,
        widget_type_id: WidgetTypeId,
        prop_id_value: u16,
    ) -> Option<&'static PropMeta> {
        self.inner.prop_meta_for_id(widget_type_id, prop_id_value)
    }

    pub fn prop_meta_for_class_id(
        &self,
        class: NodeClass,
        prop_id_value: u16,
    ) -> Option<&'static PropMeta> {
        self.inner.prop_meta_for_class_id(class, prop_id_value)
    }

    pub fn export_id(&self, export_name: &str) -> Option<u16> {
        self.inner.export_id(export_name)
    }

    pub fn spec_bindings(&self) -> &[&'static SpecWidgetBinding] {
        self.inner.spec_bindings()
    }

    pub fn export_meta_for_id(
        &self,
        export_id_value: u16,
    ) -> Option<(&'static str, EventPayloadKind)> {
        self.inner.export_meta_for_id(export_id_value)
    }
}
