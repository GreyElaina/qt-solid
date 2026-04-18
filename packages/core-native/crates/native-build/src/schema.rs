use qt_solid_widget_core::decl::SpecWidgetKey;

pub(crate) use qt_solid_widget_core::codegen::*;
pub(crate) use qt_solid_widget_core::runtime::WidgetPropDecl;
pub(crate) use qt_solid_widget_core::schema::*;

pub(crate) fn widget_registry() -> &'static WidgetRegistry {
    qt_solid_widgets_registry::widget_registry()
}

pub(crate) fn prop_decl(spec_key: SpecWidgetKey) -> Option<&'static WidgetPropDecl> {
    qt_solid_widgets_registry::all_widget_libraries()
        .iter()
        .flat_map(|library| library.widget_prop_decls.iter().copied())
        .find(|decl| decl.spec_key == spec_key)
}

pub(crate) fn all_widget_bindings() -> &'static [&'static WidgetBinding] {
    widget_registry().bindings()
}

pub(crate) fn all_opaque_decls() -> &'static [&'static SpecOpaqueDecl] {
    qt_solid_widgets_registry::all_opaque_decls()
}

pub(crate) fn all_opaque_codegen_decls() -> &'static [&'static OpaqueCodegenDecl] {
    qt_solid_widgets_registry::all_opaque_codegen_decls()
}

pub(crate) fn all_widget_codegen_decls() -> &'static [&'static WidgetCodegenDecl] {
    qt_solid_widgets_registry::all_widget_codegen_decls()
}
