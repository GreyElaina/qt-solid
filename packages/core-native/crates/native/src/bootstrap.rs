use std::sync::OnceLock;

use qt_solid_runtime::registry::RuntimeWidgetRegistry;
pub(crate) use qt_solid_widget_core::schema::*;

static REGISTRY: OnceLock<RuntimeWidgetRegistry> = OnceLock::new();

pub(crate) fn widget_registry() -> &'static RuntimeWidgetRegistry {
    REGISTRY.get_or_init(|| {
        RuntimeWidgetRegistry::build(qt_solid_widgets_registry::assembly::all_widget_libraries())
    })
}
