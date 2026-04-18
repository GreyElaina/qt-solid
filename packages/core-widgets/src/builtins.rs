use std::sync::OnceLock;

use qt::codegen::{OpaqueCodegenDecl, QtOpaqueCodegenDecl};
use qt::runtime::{
    QtWidgetNativeDecl, WidgetNativeDecl, WidgetPropDecl, collect_widget_prop_decls,
};
use qt::schema::{
    QtOpaqueDecl, QtWidgetDecl, SpecOpaqueDecl, SpecWidgetBinding, WidgetLibraryBindings,
};

use super::widgets::{
    button::ButtonWidget,
    canvas::{CanvasWidget, QtPainter},
    check::CheckWidget,
    double_spin_box::DoubleSpinBoxWidget,
    group::GroupWidget,
    input::InputWidget,
    label::LabelWidget,
    slider::SliderWidget,
    text::TextWidget,
    view::ViewWidget,
    window::WindowWidget,
};

fn core_widgets_spec_bindings() -> &'static [&'static SpecWidgetBinding] {
    static ALL: OnceLock<Vec<&'static SpecWidgetBinding>> = OnceLock::new();
    ALL.get_or_init(|| {
        vec![
            WindowWidget::spec(),
            ViewWidget::spec(),
            GroupWidget::spec(),
            LabelWidget::spec(),
            ButtonWidget::spec(),
            CanvasWidget::spec(),
            InputWidget::spec(),
            CheckWidget::spec(),
            TextWidget::spec(),
            SliderWidget::spec(),
            DoubleSpinBoxWidget::spec(),
        ]
    })
    .as_slice()
}

fn core_widgets_opaque_decls() -> &'static [&'static SpecOpaqueDecl] {
    static ALL: OnceLock<Vec<&'static SpecOpaqueDecl>> = OnceLock::new();
    ALL.get_or_init(|| vec![&QtPainter::SPEC]).as_slice()
}

fn core_widgets_opaque_codegen_decls() -> &'static [&'static OpaqueCodegenDecl] {
    static ALL: OnceLock<Vec<&'static OpaqueCodegenDecl>> = OnceLock::new();
    ALL.get_or_init(|| vec![&QtPainter::CODEGEN]).as_slice()
}

fn core_widgets_native_decls() -> &'static [&'static WidgetNativeDecl] {
    static ALL: OnceLock<Vec<&'static WidgetNativeDecl>> = OnceLock::new();
    ALL.get_or_init(|| {
        vec![
            &WindowWidget::NATIVE_DECL,
            &ViewWidget::NATIVE_DECL,
            &GroupWidget::NATIVE_DECL,
            &LabelWidget::NATIVE_DECL,
            &ButtonWidget::NATIVE_DECL,
            &CanvasWidget::NATIVE_DECL,
            &InputWidget::NATIVE_DECL,
            &CheckWidget::NATIVE_DECL,
            &TextWidget::NATIVE_DECL,
            &SliderWidget::NATIVE_DECL,
            &DoubleSpinBoxWidget::NATIVE_DECL,
        ]
    })
    .as_slice()
}

fn core_widgets_prop_decls() -> &'static [&'static WidgetPropDecl] {
    static ALL: OnceLock<Vec<&'static WidgetPropDecl>> = OnceLock::new();
    ALL.get_or_init(|| collect_widget_prop_decls(core_widgets_spec_bindings()))
        .as_slice()
}

pub fn core_widgets_library() -> &'static WidgetLibraryBindings {
    static LIBRARY: OnceLock<WidgetLibraryBindings> = OnceLock::new();
    LIBRARY.get_or_init(|| WidgetLibraryBindings {
        library_key: "@qt-solid/core-widgets",
        spec_bindings: core_widgets_spec_bindings(),
        opaque_decls: core_widgets_opaque_decls(),
        opaque_codegen_decls: core_widgets_opaque_codegen_decls(),
        widget_native_decls: core_widgets_native_decls(),
        widget_prop_decls: core_widgets_prop_decls(),
    })
}

#[cfg(test)]
mod tests {
    use super::core_widgets_library;
    use qt::schema::{WidgetRegistry, merged_props};

    #[test]
    fn every_spec_has_constructor_decl() {
        let library = core_widgets_library();

        for spec in library.spec_bindings {
            let has_native = library
                .widget_native_decls
                .iter()
                .any(|decl| decl.spec_key == spec.spec_key);
            let has_prop_ctor = library
                .widget_prop_decls
                .iter()
                .find(|decl| decl.spec_key == spec.spec_key)
                .and_then(|decl| decl.create_instance)
                .is_some();

            assert!(
                has_native || has_prop_ctor,
                "missing widget constructor declaration for spec widget key {}",
                spec.spec_key.raw()
            );
        }
    }

    #[test]
    fn window_widget_prop_decl_exposes_frame_state() {
        let library = core_widgets_library();
        let decl = library
            .widget_prop_decls
            .iter()
            .find(|decl| decl.spec_key.raw().ends_with("WindowWidget"))
            .copied()
            .expect("window prop decl");

        assert!(decl.create_instance.is_some());

        let keys = decl
            .props
            .iter()
            .map(|prop| prop.path.join("."))
            .collect::<Vec<_>>();

        assert!(keys.contains(&"seq".to_owned()));
        assert!(keys.contains(&"elapsedMs".to_owned()));
        assert!(keys.contains(&"deltaMs".to_owned()));
        assert!(keys.contains(&"tick".to_owned()));
        assert!(keys.contains(&"nextFrameRequested".to_owned()));

        let spec = library
            .spec_bindings
            .iter()
            .find(|spec| spec.spec_key == decl.spec_key)
            .copied()
            .expect("window spec binding");
        let merged_keys = merged_props(spec, Some(decl))
            .into_iter()
            .filter(|prop| !prop.is_bound)
            .map(|prop| prop.key)
            .collect::<Vec<_>>();

        assert!(merged_keys.contains(&"seq".to_owned()));
        assert!(merged_keys.contains(&"elapsedMs".to_owned()));
        assert!(merged_keys.contains(&"deltaMs".to_owned()));
        assert!(merged_keys.contains(&"tick".to_owned()));
        assert!(merged_keys.contains(&"nextFrameRequested".to_owned()));
    }

    #[test]
    fn window_widget_host_binding_excludes_frame_state_props() {
        let library = core_widgets_library();
        let registry = WidgetRegistry::build(&[library]);
        let window_spec = library
            .spec_bindings
            .iter()
            .find(|spec| spec.spec_key.raw().ends_with("WindowWidget"))
            .copied()
            .expect("window spec binding");
        let binding = registry.binding_by_spec_key(window_spec.spec_key);
        let keys = binding
            .props
            .iter()
            .map(|prop| prop.js_name)
            .collect::<Vec<_>>();

        assert!(!keys.contains(&"seq"));
        assert!(!keys.contains(&"elapsedMs"));
        assert!(!keys.contains(&"deltaMs"));
        assert!(!keys.contains(&"tick"));
        assert!(!keys.contains(&"nextFrameRequested"));
    }
}
